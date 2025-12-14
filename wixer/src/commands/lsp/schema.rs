//! Schema loading and indexing for LSP
//!
//! Handles loading WiX JSON schemas and building completion/documentation indices.

use crate::describe_db;
use crate::embedded_assets;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tower_lsp::lsp_types::*;

use super::WixerLsp;

struct IndexContext<'a> {
    schema_name: &'a str,
    docs: &'a mut HashMap<String, String>,
    docs_urls: &'a mut HashMap<String, String>,
    msi_tables: &'a mut HashMap<String, String>,
    see_also: &'a mut HashMap<String, Vec<String>>,
    doc_paths: &'a mut HashMap<String, Vec<String>>,
    completions: &'a mut Vec<CompletionItem>,
    symbols: &'a mut HashMap<String, SchemaSymbolInfo>,
}

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
    // Load schemas from embedded assets
    let schema_names = match embedded_assets::list_json_schemas() {
        Ok(names) => names,
        Err(e) => {
            lsp.client
                .log_message(
                    MessageType::WARNING,
                    format!("Failed to list embedded schemas: {}", e),
                )
                .await;
            return;
        }
    };

    let mut schemas = lsp.schemas.write().await;
    let mut docs = lsp.docs.write().await;
    let mut docs_urls = lsp.docs_urls.write().await;
    let mut msi_tables = lsp.msi_tables.write().await;
    let mut see_also = lsp.see_also.write().await;
    let mut doc_paths = lsp.doc_paths.write().await;
    let mut completions = lsp.completions.write().await;
    let mut symbols = lsp.symbols.write().await;

    for name in schema_names {
        let Ok(content) = embedded_assets::get_json_schema(&name) else {
            continue;
        };

        let Ok(json) = serde_json::from_str::<Value>(&content) else {
            continue;
        };

        if !compile_schema(&json, &name, &mut schemas) {
            continue;
        }

        let mut schema_base = name.as_str();
        if let Some(stripped) = schema_base.strip_suffix(".json") {
            schema_base = stripped;
        }
        if let Some(stripped) = schema_base.strip_suffix(".schema") {
            schema_base = stripped;
        }

        let mut schema_symbols = HashMap::new();

        let mut ctx = IndexContext {
            schema_name: schema_base,
            docs: &mut docs,
            docs_urls: &mut docs_urls,
            msi_tables: &mut msi_tables,
            see_also: &mut see_also,
            doc_paths: &mut doc_paths,
            completions: &mut completions,
            symbols: &mut schema_symbols,
        };

        index_schema(&json, &mut ctx);
        symbols.insert(name, schema_symbols);
    }

    // Load from unified describe_db (knowledge base + schemas + extensions)
    let db_count = load_from_describe_db(
        &mut docs,
        &mut docs_urls,
        &mut msi_tables,
        &mut see_also,
        &mut doc_paths,
        &mut completions,
    );

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
    docs_urls: &mut HashMap<String, String>,
    _msi_tables: &mut HashMap<String, String>,
    _see_also: &mut HashMap<String, Vec<String>>,
    doc_paths: &mut HashMap<String, Vec<String>>,
    completions: &mut Vec<CompletionItem>,
) -> usize {
    let properties = match describe_db::load() {
        Ok(props) => props,
        Err(_) => return 0,
    };

    let mut count = 0;
    for prop in properties {
        // If already loaded from schema, still allow backfilling docs URLs.
        let already_loaded = docs.contains_key(&prop.name);

        if !already_loaded {
            docs.insert(prop.name.clone(), prop.description.clone());

            doc_paths
                .entry(prop.name.clone())
                .or_default()
                .push(prop.name.clone());
        }

        if let Some(url) = &prop.docs_url {
            docs_urls
                .entry(prop.name.clone())
                .or_insert_with(|| url.clone());
        }

        if prop.kind == "Element"
            && let Some(schema_tag) = prop.tags.iter().find(|t| *t != "Element")
        {
            let qualified = format!("{}.{}", schema_tag, prop.name);

            if !already_loaded {
                docs.entry(qualified.clone())
                    .or_insert_with(|| prop.description.clone());

                doc_paths
                    .entry(prop.name.clone())
                    .or_default()
                    .push(qualified.clone());
                doc_paths
                    .entry(qualified.clone())
                    .or_default()
                    .push(qualified.clone());
            }

            if let Some(url) = &prop.docs_url {
                docs_urls
                    .entry(qualified.clone())
                    .or_insert_with(|| url.clone());
            }
        }

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

fn index_schema(json: &Value, ctx: &mut IndexContext<'_>) {
    let Some(definitions) = json.get("definitions").and_then(|d| d.as_object()) else {
        return;
    };

    for (def_name, def_schema) in definitions {
        record_symbol(ctx, def_name, def_schema, SchemaSymbolKind::Element, None);

        let Some(props) = def_schema.get("properties").and_then(|p| p.as_object()) else {
            continue;
        };

        for (prop_name, prop_schema) in props {
            record_symbol(
                ctx,
                prop_name,
                prop_schema,
                SchemaSymbolKind::Attribute,
                Some(def_name),
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
                        ctx,
                        prop_name,
                        prop_schema,
                        SchemaSymbolKind::Attribute,
                        Some(ref_name),
                    );
                }
            }
        }
    }
}

fn record_symbol(
    ctx: &mut IndexContext<'_>,
    name: &str,
    schema: &Value,
    kind: SchemaSymbolKind,
    parent: Option<&str>,
) {
    let desc = schema.get("description").and_then(|d| d.as_str());
    let docs_url = schema
        .get("x-wixer-docs-url")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    let msi_table = schema
        .get("x-wixer-msi-table")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    let see_also = schema
        .get("x-wixer-see-also")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    let should_index =
        desc.is_some() || docs_url.is_some() || msi_table.is_some() || !see_also.is_empty();

    let base_key = if let Some(parent) = parent {
        format!("{}.{}", parent, name)
    } else {
        name.to_string()
    };
    let schema_key = format!("{}.{}", ctx.schema_name, base_key);

    if let Some(desc) = desc {
        ctx.docs
            .entry(name.to_string())
            .or_insert_with(|| desc.to_string());
        ctx.docs
            .entry(base_key.clone())
            .or_insert_with(|| desc.to_string());
        ctx.docs
            .entry(schema_key.clone())
            .or_insert_with(|| desc.to_string());
    }

    if let Some(url) = docs_url {
        ctx.docs_urls
            .entry(name.to_string())
            .or_insert_with(|| url.clone());
        ctx.docs_urls
            .entry(base_key.clone())
            .or_insert_with(|| url.clone());
        ctx.docs_urls
            .entry(schema_key.clone())
            .or_insert_with(|| url);
    }

    if let Some(table) = msi_table {
        ctx.msi_tables
            .entry(name.to_string())
            .or_insert_with(|| table.clone());
        ctx.msi_tables
            .entry(base_key.clone())
            .or_insert_with(|| table.clone());
        ctx.msi_tables
            .entry(schema_key.clone())
            .or_insert_with(|| table);
    }

    if !see_also.is_empty() {
        ctx.see_also
            .entry(name.to_string())
            .or_insert_with(|| see_also.clone());
        ctx.see_also
            .entry(base_key.clone())
            .or_insert_with(|| see_also.clone());
        ctx.see_also
            .entry(schema_key.clone())
            .or_insert_with(|| see_also);
    }

    if should_index {
        ctx.doc_paths
            .entry(name.to_string())
            .or_default()
            .push(schema_key.clone());
        ctx.doc_paths
            .entry(base_key.clone())
            .or_default()
            .push(schema_key.clone());
        ctx.doc_paths
            .entry(schema_key.clone())
            .or_default()
            .push(schema_key);
    }

    ctx.symbols
        .entry(name.to_string())
        .and_modify(|info| info.update_from_schema(kind, name, schema))
        .or_insert_with(|| SchemaSymbolInfo::from_schema(kind, name, schema));

    let completion_kind = match kind {
        SchemaSymbolKind::Element => CompletionItemKind::CLASS,
        SchemaSymbolKind::Attribute | SchemaSymbolKind::EnumValue => CompletionItemKind::PROPERTY,
    };

    ctx.completions.push(CompletionItem {
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
