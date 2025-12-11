//! Schema loading and indexing for LSP
//!
//! Handles loading WiX JSON schemas and building completion/documentation indices.

use crate::constants::SCHEMA_DIR_NAME;
use crate::describe_db;
use crate::dirs;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tower_lsp::lsp_types::*;

use super::WixerLsp;

#[derive(Clone, Copy)]
pub enum SchemaSymbolKind {
    Element,
    Attribute,
    #[allow(dead_code)]
    EnumValue,
}

#[derive(Clone)]
pub struct SchemaSymbolInfo {
    pub kind: SchemaSymbolKind,
    pub value_kind: Option<SchemaValueType>,
    pub enum_values: Option<HashSet<String>>,
    pub deprecated: bool,
    pub is_id: bool,
}

#[derive(Clone, Copy)]
pub enum SchemaValueType {
    Boolean,
    Number,
    Integer,
}

impl SchemaSymbolInfo {
    pub fn from_schema(kind: SchemaSymbolKind, name: &str, schema: &Value) -> Self {
        let mut info = SchemaSymbolInfo {
            kind,
            value_kind: None,
            enum_values: None,
            deprecated: false,
            is_id: false,
        };
        info.update_from_schema(kind, name, schema);
        info
    }

    pub fn update_from_schema(&mut self, kind: SchemaSymbolKind, name: &str, schema: &Value) {
        self.kind = kind;

        if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
            let mut set = self.enum_values.take().unwrap_or_default();
            for value in enum_values {
                match value {
                    Value::String(s) => {
                        set.insert(s.clone());
                    }
                    Value::Number(n) => {
                        set.insert(n.to_string());
                    }
                    Value::Bool(b) => {
                        set.insert(b.to_string());
                    }
                    _ => {}
                }
            }
            if !set.is_empty() {
                self.enum_values = Some(set);
            }
        }

        if let Some(value_type) = determine_value_type(schema) {
            self.value_kind = Some(value_type);
        }

        if schema
            .get("deprecated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            self.deprecated = true;
        }

        if schema_indicates_id(name, schema) {
            self.is_id = true;
        }
    }
}

pub async fn load_schemas(lsp: &WixerLsp) {
    let Some(cache_dir) = resolve_cache_dir(lsp).await else {
        return;
    };

    let schema_dir = cache_dir.join(SCHEMA_DIR_NAME);
    if !schema_dir.exists() {
        lsp.client
            .log_message(MessageType::WARNING, "Schema directory not found")
            .await;
        return;
    }

    let Ok(entries) = fs::read_dir(&schema_dir) else {
        lsp.client
            .log_message(MessageType::WARNING, "Failed to read schema directory")
            .await;
        return;
    };

    let mut schemas = lsp.schemas.write().await;
    let mut docs = lsp.docs.write().await;
    let mut completions = lsp.completions.write().await;
    let mut symbols = lsp.symbols.write().await;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let Some(json) = read_schema_json(&path) else {
            continue;
        };

        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let name = stem.replace(".schema", "");

        if !compile_schema(&json, &name, &mut schemas) {
            continue;
        }

        let mut schema_symbols = HashMap::new();
        index_schema(&json, &mut docs, &mut completions, &mut schema_symbols);
        symbols.insert(name, schema_symbols);
    }

    // Load from unified describe_db (knowledge base + schemas + extensions)
    let db_count = load_from_describe_db(&mut docs, &mut completions);

    completions.sort_by(|a, b| a.label.cmp(&b.label));
    completions.dedup_by(|a, b| a.label == b.label);

    lsp.client
        .log_message(
            MessageType::INFO,
            format!(
                "Loaded {} schemas, {} describe_db entries, {} total completions",
                schemas.len(),
                db_count,
                completions.len()
            ),
        )
        .await;
}

fn load_from_describe_db(
    docs: &mut HashMap<String, String>,
    completions: &mut Vec<CompletionItem>,
) -> usize {
    let properties = match describe_db::load() {
        Ok(props) => props,
        Err(_) => return 0,
    };

    let mut count = 0;
    for prop in properties {
        // Skip if already loaded from schema
        if docs.contains_key(&prop.name) {
            continue;
        }

        docs.insert(prop.name.clone(), prop.description.clone());

        let kind = match prop.kind.as_str() {
            "CustomAction" | "SequenceEntry" => CompletionItemKind::FUNCTION,
            "Property" => CompletionItemKind::VARIABLE,
            "Element" => CompletionItemKind::CLASS,
            "Attribute" => CompletionItemKind::PROPERTY,
            _ => CompletionItemKind::TEXT,
        };

        let detail = if prop.tags.is_empty() {
            None
        } else {
            Some(prop.tags.join(", "))
        };

        completions.push(CompletionItem {
            label: prop.name.clone(),
            kind: Some(kind),
            detail,
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: prop.description,
            })),
            ..Default::default()
        });
        count += 1;
    }

    count
}

async fn resolve_cache_dir(lsp: &WixerLsp) -> Option<std::path::PathBuf> {
    match dirs::get_cache_dir() {
        Ok(dir) => Some(dir),
        Err(_) => {
            lsp.client
                .log_message(MessageType::WARNING, "Could not determine cache directory")
                .await;
            None
        }
    }
}

fn read_schema_json(path: &Path) -> Option<Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn compile_schema(
    json: &Value,
    name: &str,
    schemas: &mut HashMap<String, jsonschema::JSONSchema>,
) -> bool {
    let compiled = match jsonschema::JSONSchema::compile(json) {
        Ok(compiled) => compiled,
        Err(_) => return false,
    };

    schemas.insert(name.to_string(), compiled);
    true
}

fn index_schema(
    json: &Value,
    docs: &mut HashMap<String, String>,
    completions: &mut Vec<CompletionItem>,
    symbols: &mut HashMap<String, SchemaSymbolInfo>,
) {
    let Some(definitions) = json.get("definitions").and_then(|d| d.as_object()) else {
        return;
    };

    for (def_name, def_schema) in definitions {
        record_symbol(
            def_name,
            def_schema,
            SchemaSymbolKind::Element,
            docs,
            completions,
            symbols,
        );

        let Some(props) = def_schema.get("properties").and_then(|p| p.as_object()) else {
            continue;
        };

        for (prop_name, prop_schema) in props {
            record_symbol(
                prop_name,
                prop_schema,
                SchemaSymbolKind::Attribute,
                docs,
                completions,
                symbols,
            );
        }
    }

    if let Some(all_of) = json.get("allOf").and_then(|arr| arr.as_array()) {
        for entry in all_of {
            let Some(ref_name) = entry
                .get("$ref")
                .and_then(|r| r.as_str())
                .and_then(|r| r.strip_prefix("#/definitions/"))
            else {
                continue;
            };

            let Some(ref_schema) = definitions.get(ref_name) else {
                continue;
            };

            if let Some(props) = ref_schema.get("properties").and_then(|p| p.as_object()) {
                for (prop_name, prop_schema) in props {
                    record_symbol(
                        prop_name,
                        prop_schema,
                        SchemaSymbolKind::Attribute,
                        docs,
                        completions,
                        symbols,
                    );
                }
            }
        }
    }
}

fn record_symbol(
    name: &str,
    schema: &Value,
    kind: SchemaSymbolKind,
    docs: &mut HashMap<String, String>,
    completions: &mut Vec<CompletionItem>,
    symbols: &mut HashMap<String, SchemaSymbolInfo>,
) {
    if let Some(desc) = schema.get("description").and_then(|d| d.as_str()) {
        docs.entry(name.to_string())
            .or_insert_with(|| desc.to_string());
    }

    symbols
        .entry(name.to_string())
        .and_modify(|info| info.update_from_schema(kind, name, schema))
        .or_insert_with(|| SchemaSymbolInfo::from_schema(kind, name, schema));

    let completion_kind = match kind {
        SchemaSymbolKind::Element => CompletionItemKind::CLASS,
        SchemaSymbolKind::Attribute | SchemaSymbolKind::EnumValue => CompletionItemKind::PROPERTY,
    };

    completions.push(CompletionItem {
        label: name.to_string(),
        kind: Some(completion_kind),
        documentation: schema.get("description").and_then(|d| d.as_str()).map(|d| {
            Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: d.to_string(),
            })
        }),
        ..Default::default()
    });
}

pub fn determine_value_type(schema: &Value) -> Option<SchemaValueType> {
    let type_node = schema.get("type")?;
    if let Some(type_str) = type_node.as_str() {
        return map_type_str(type_str);
    }
    if let Some(arr) = type_node.as_array() {
        for entry in arr {
            if let Some(type_str) = entry.as_str()
                && let Some(mapped) = map_type_str(type_str)
            {
                return Some(mapped);
            }
        }
    }
    None
}

fn map_type_str(type_str: &str) -> Option<SchemaValueType> {
    match type_str {
        "boolean" => Some(SchemaValueType::Boolean),
        "number" => Some(SchemaValueType::Number),
        "integer" => Some(SchemaValueType::Integer),
        _ => None,
    }
}

pub fn schema_indicates_id(name: &str, schema: &Value) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower == "id" || lower.ends_with("id") {
        return true;
    }
    if schema
        .get("format")
        .and_then(|v| v.as_str())
        .map(|s| matches!(s, "uuid" | "guid" | "wix-id"))
        .unwrap_or(false)
    {
        return true;
    }
    if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
        let pattern_lower = pattern.to_ascii_lowercase();
        if pattern_lower.contains("id") {
            return true;
        }
    }
    false
}
