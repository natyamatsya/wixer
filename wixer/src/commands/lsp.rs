use crate::analysis::{orphan_detection, publish_conflicts};
use crate::constants::{EXT_WIXLOC, EXT_WIXLOC_YAML, EXT_WXL_YAML, SCHEMA_DIR_NAME};
use crate::describe_db;
use crate::dirs;
use crate::includes;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

pub struct WixerLsp {
    client: Client,
    schemas: Arc<RwLock<HashMap<String, jsonschema::JSONSchema>>>,
    docs: Arc<RwLock<HashMap<String, String>>>,
    completions: Arc<RwLock<Vec<CompletionItem>>>,
    symbols: Arc<RwLock<HashMap<String, HashMap<String, SchemaSymbolInfo>>>>,
    documents: Arc<RwLock<HashMap<Url, String>>>,
    /// Cache for resolved YAML (with includes merged)
    resolved_cache: Arc<RwLock<HashMap<PathBuf, serde_yaml::Value>>>,
}

#[derive(Clone, Copy)]
enum SchemaSymbolKind {
    Element,
    Attribute,
    #[allow(dead_code)]
    EnumValue,
}

#[derive(Clone)]
struct SchemaSymbolInfo {
    kind: SchemaSymbolKind,
    value_kind: Option<SchemaValueType>,
    enum_values: Option<HashSet<String>>,
    deprecated: bool,
    is_id: bool,
}

#[derive(Clone, Copy)]
enum SchemaValueType {
    Boolean,
    Number,
    Integer,
}

impl SchemaSymbolInfo {
    fn from_schema(kind: SchemaSymbolKind, name: &str, schema: &Value) -> Self {
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

    fn update_from_schema(&mut self, kind: SchemaSymbolKind, name: &str, schema: &Value) {
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

impl WixerLsp {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            schemas: Arc::new(RwLock::new(HashMap::new())),
            docs: Arc::new(RwLock::new(HashMap::new())),
            completions: Arc::new(RwLock::new(Vec::new())),
            symbols: Arc::new(RwLock::new(HashMap::new())),
            documents: Arc::new(RwLock::new(HashMap::new())),
            resolved_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get resolved YAML with includes, using cache if available
    async fn get_resolved_yaml(&self, path: &Path) -> Option<serde_yaml::Value> {
        // Check cache first
        {
            let cache = self.resolved_cache.read().await;
            if let Some(cached) = cache.get(path) {
                return Some(cached.clone());
            }
        }

        // Not in cache, resolve includes
        if path.exists()
            && let Ok(resolved) = includes::load_and_process_yaml(path)
        {
            // Store in cache
            let mut cache = self.resolved_cache.write().await;
            cache.insert(path.to_path_buf(), resolved.clone());
            return Some(resolved);
        }

        None
    }

    /// Invalidate cache for a file and any files that might include it
    async fn invalidate_cache(&self, path: &Path) {
        let mut cache = self.resolved_cache.write().await;
        // Remove the changed file
        cache.remove(path);
        // Also clear any parent files that might include this one
        // (conservative approach: clear all for now)
        if path.extension().is_some_and(|e| e == "yaml") {
            cache.clear();
        }
    }

    async fn load_schemas(&self) {
        let Some(cache_dir) = self.resolve_cache_dir().await else {
            return;
        };

        let schema_dir = cache_dir.join(SCHEMA_DIR_NAME);
        if !schema_dir.exists() {
            self.client
                .log_message(MessageType::WARNING, "Schema directory not found")
                .await;
            return;
        }

        let Ok(entries) = fs::read_dir(&schema_dir) else {
            self.client
                .log_message(MessageType::WARNING, "Failed to read schema directory")
                .await;
            return;
        };

        let mut schemas = self.schemas.write().await;
        let mut docs = self.docs.write().await;
        let mut completions = self.completions.write().await;
        let mut symbols = self.symbols.write().await;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let Some(json) = Self::read_schema_json(&path) else {
                continue;
            };

            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let name = stem.replace(".schema", "");

            if !Self::compile_schema(&json, &name, &mut schemas) {
                continue;
            }

            let mut schema_symbols = HashMap::new();
            Self::index_schema(&json, &mut docs, &mut completions, &mut schema_symbols);
            symbols.insert(name, schema_symbols);
        }

        // Load from unified describe_db (knowledge base + schemas + extensions)
        let db_count = Self::load_from_describe_db(&mut docs, &mut completions);

        completions.sort_by(|a, b| a.label.cmp(&b.label));
        completions.dedup_by(|a, b| a.label == b.label);

        self.client
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

    async fn resolve_cache_dir(&self) -> Option<PathBuf> {
        match dirs::get_cache_dir() {
            Ok(dir) => Some(dir),
            Err(_) => {
                self.client
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
            Self::record_symbol(
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
                Self::record_symbol(
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
                        Self::record_symbol(
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
            SchemaSymbolKind::Attribute | SchemaSymbolKind::EnumValue => {
                CompletionItemKind::PROPERTY
            }
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

    fn get_schema_name(&self, uri: &Url) -> &str {
        let path = PathBuf::from(uri.path());
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if filename.ends_with(EXT_WIXLOC_YAML) || filename.ends_with(EXT_WXL_YAML) {
            EXT_WIXLOC
        } else {
            "wix"
        }
    }

    async fn validate_document(&self, uri: Url, version: Option<i32>, text: String) {
        let schemas = self.schemas.read().await;

        let path = PathBuf::from(uri.path());
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let extension_schema = self.get_schema_name(&uri);

        let schema = schemas
            .get(&filename)
            .or_else(|| schemas.get(extension_schema))
            .or_else(|| schemas.get("wix"));

        let Some(schema) = schema else {
            return;
        };

        let diagnostics = match serde_yaml::from_str::<serde_yaml::Value>(&text) {
            Ok(yaml_val) => {
                let mut diags = Vec::new();

                // Schema validation
                if let Ok(mut json_val) = serde_json::to_value(&yaml_val) {
                    remove_xmlns(&mut json_val);

                    if let Err(errors) = schema.validate(&json_val) {
                        diags.extend(errors.map(|error| {
                            let range = find_range(&text, &error.instance_path.to_string());
                            Diagnostic {
                                range,
                                severity: Some(DiagnosticSeverity::ERROR),
                                code: None,
                                code_description: None,
                                source: Some("wixer".to_string()),
                                message: error.to_string(),
                                related_information: None,
                                tags: None,
                                data: None,
                            }
                        }));
                    }
                }

                // Orphan component detection (with include resolution, cached)
                let resolved_yaml = self
                    .get_resolved_yaml(&path)
                    .await
                    .unwrap_or_else(|| yaml_val.clone());

                let orphans = orphan_detection::find_orphans(&resolved_yaml);
                for orphan in orphans {
                    // Try to find the orphan in original text for accurate range
                    let range = find_orphan_range(&text, &orphan.id);
                    diags.push(Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(NumberOrString::String("orphan-component".to_string())),
                        code_description: None,
                        source: Some("wixer".to_string()),
                        message: format!(
                            "{} '{}' is not referenced by any Feature and will not be installed",
                            orphan.kind, orphan.id
                        ),
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }

                // Publish order conflict detection (UI extension)
                let conflicts = publish_conflicts::find_conflicts(&resolved_yaml);
                for conflict in conflicts {
                    let range = find_publish_range(&text, &conflict.dialog, &conflict.control);
                    diags.push(Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(NumberOrString::String("publish-order-conflict".to_string())),
                        code_description: None,
                        source: Some("wixer".to_string()),
                        message: format!(
                            "Publish Order conflict: Dialog '{}' Control '{}' Order {} conflicts with {} extension event '{}'. Consider using a different Order value.",
                            conflict.dialog, conflict.control, conflict.order,
                            conflict.extension_name, conflict.extension_event
                        ),
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }

                diags
            }
            Err(e) => {
                let range = if let Some(loc) = e.location() {
                    Range {
                        start: Position::new(loc.line() as u32 - 1, loc.column() as u32 - 1),
                        end: Position::new(loc.line() as u32 - 1, loc.column() as u32),
                    }
                } else {
                    Range::default()
                };

                vec![Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("wixer".to_string()),
                    message: e.to_string(),
                    related_information: None,
                    tags: None,
                    data: None,
                }]
            }
        };

        self.client
            .publish_diagnostics(uri, diagnostics, version)
            .await;
    }
}

fn determine_value_type(schema: &Value) -> Option<SchemaValueType> {
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

fn detect_indent_width(text: &str) -> usize {
    let mut indent = None;

    for line in text.lines() {
        let trimmed = line.trim_start_matches([' ', '\t']);
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let leading = line.len() - trimmed.len();
        if leading > 0 {
            match indent {
                Some(current) if leading >= current => {}
                _ => indent = Some(leading),
            }
        }
    }

    indent.unwrap_or(2).max(1)
}

fn find_range(text: &str, instance_path: &str) -> Range {
    let segments: Vec<&str> = instance_path.trim_start_matches('/').split('/').collect();
    if segments.is_empty() {
        return Range::default();
    }

    // Try to find the last segment that is not a number (array index)
    let target_key = segments.iter().rev().find(|s| s.parse::<usize>().is_err());

    if let Some(key) = target_key {
        let non_numeric: Vec<&str> = segments
            .iter()
            .filter(|s| s.parse::<usize>().is_err())
            .cloned()
            .collect();
        let expected_depth = non_numeric.len().saturating_sub(1);
        let indent_width = detect_indent_width(text);

        for (line_idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let indent = line.len().saturating_sub(trimmed.len());
            let indent_depth = indent / indent_width;

            if indent_depth > expected_depth + 1 {
                continue;
            }

            if let Some(col_idx) = line.find(*key) {
                let after_key = &line[col_idx + key.len()..];
                if (trimmed.starts_with('-') && after_key.starts_with(':')
                    || after_key.starts_with(' '))
                    || (trimmed.starts_with(*key)
                        && (after_key.starts_with(':') || after_key.starts_with(' '))
                        && indent_depth >= expected_depth.saturating_sub(1))
                {
                    let col_idx = line.find(*key).unwrap_or(0);
                    return Range {
                        start: Position::new(line_idx as u32, col_idx as u32),
                        end: Position::new(line_idx as u32, (col_idx + key.len()) as u32),
                    };
                }
            }
        }
    }

    Range::default()
}

/// Find the range in the text where an orphan component/group Id is defined
fn find_orphan_range(text: &str, id: &str) -> Range {
    // Look for "Id: <id>" pattern in the text
    for (line_idx, line) in text.lines().enumerate() {
        // Check for Id: value pattern
        if let Some(id_pos) = line.find("Id:") {
            let after_id = &line[id_pos + 3..];
            let trimmed = after_id.trim();
            if trimmed == id || trimmed.trim_matches(|c| c == '"' || c == '\'') == id {
                return Range {
                    start: Position::new(line_idx as u32, id_pos as u32),
                    end: Position::new(line_idx as u32, line.len() as u32),
                };
            }
        }
        // Check for - Id: value pattern (in sequences)
        if let Some(dash_pos) = line.find("- Id:") {
            let after_id = &line[dash_pos + 5..];
            let trimmed = after_id.trim();
            if trimmed == id || trimmed.trim_matches(|c| c == '"' || c == '\'') == id {
                return Range {
                    start: Position::new(line_idx as u32, dash_pos as u32),
                    end: Position::new(line_idx as u32, line.len() as u32),
                };
            }
        }
    }
    Range::default()
}

/// Find the range in text where a Publish element with given Dialog/Control is defined
fn find_publish_range(text: &str, dialog: &str, control: &str) -> Range {
    let mut in_publish_block = false;
    let mut found_dialog = false;
    let mut publish_line: Option<usize> = None;

    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        // Look for Publish: key
        if trimmed.starts_with("Publish:") || trimmed.starts_with("- Publish:") {
            in_publish_block = true;
            publish_line = Some(line_idx);
            found_dialog = false;
            continue;
        }

        if in_publish_block {
            // Check for Dialog match
            if trimmed.starts_with("Dialog:") && trimmed.contains(dialog) {
                found_dialog = true;
            }
            // Check for Control match after finding Dialog
            if found_dialog && trimmed.starts_with("Control:") && trimmed.contains(control) {
                // Found the matching Publish element
                if let Some(pub_line) = publish_line {
                    return Range {
                        start: Position::new(pub_line as u32, 0),
                        end: Position::new(line_idx as u32, line.len() as u32),
                    };
                }
            }
            // Reset if we hit a new top-level key
            if !trimmed.is_empty()
                && !trimmed.starts_with('-')
                && !trimmed.starts_with('#')
                && trimmed.chars().next().is_some_and(|c| c.is_alphabetic())
                && !trimmed.starts_with("Dialog:")
                && !trimmed.starts_with("Control:")
                && !trimmed.starts_with("Event:")
                && !trimmed.starts_with("Value:")
                && !trimmed.starts_with("Order:")
                && !trimmed.starts_with("Condition:")
            {
                in_publish_block = false;
                found_dialog = false;
            }
        }
    }
    Range::default()
}

fn remove_xmlns(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|k, _| !k.starts_with("xmlns"));
            for (_, v) in map.iter_mut() {
                remove_xmlns(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                remove_xmlns(v);
            }
        }
        _ => {}
    }
}

fn extract_value_segment(val_part: &str) -> &str {
    let trimmed = val_part.trim_start();
    let without_comment = trimmed.split('#').next().unwrap_or(trimmed);
    without_comment.trim()
}

fn determine_value_token(value: &str, symbol: Option<&SchemaSymbolInfo>) -> (u32, u32) {
    let stripped = strip_quotes(value);

    if let Some(info) = symbol {
        if let Some(enum_values) = &info.enum_values
            && enum_values.contains(stripped)
        {
            return (3, if info.deprecated { 1 } else { 0 });
        }

        if let Some(kind) = info.value_kind {
            match kind {
                SchemaValueType::Boolean => {
                    if is_boolean_literal(stripped) {
                        return (5, if info.deprecated { 1 } else { 0 });
                    }
                }
                SchemaValueType::Number | SchemaValueType::Integer => {
                    if is_numeric_literal(stripped) {
                        return (4, if info.deprecated { 1 } else { 0 });
                    }
                }
            }
        }

        if info.is_id {
            return (6, if info.deprecated { 1 } else { 0 });
        }
    }

    if is_boolean_literal(stripped) {
        return (5, 0);
    }
    if is_numeric_literal(stripped) {
        return (4, 0);
    }

    (2, 0)
}

fn strip_quotes(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        if (bytes[0] == b'"' && bytes[trimmed.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[trimmed.len() - 1] == b'\'')
        {
            return &trimmed[1..trimmed.len() - 1];
        }
    }
    trimmed
}

fn is_boolean_literal(value: &str) -> bool {
    matches!(value, "true" | "false" | "yes" | "no" | "on" | "off")
}

fn is_numeric_literal(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    if value.starts_with("0x") || value.starts_with("0X") {
        return value[2..].chars().all(|c| c.is_ascii_hexdigit());
    }
    value
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+')
}

fn schema_indicates_id(name: &str, schema: &Value) -> bool {
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

#[tower_lsp::async_trait]
impl LanguageServer for WixerLsp {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        self.load_schemas().await;
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions::default()),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    SemanticTokenType::CLASS,       // 0: Elements
                                    SemanticTokenType::PROPERTY,    // 1: Attributes
                                    SemanticTokenType::STRING,      // 2: Strings
                                    SemanticTokenType::ENUM_MEMBER, // 3: Enum values
                                    SemanticTokenType::NUMBER,      // 4: Numbers
                                    SemanticTokenType::KEYWORD,     // 5: Booleans
                                    SemanticTokenType::VARIABLE,    // 6: Identifiers/IDs
                                ],
                                token_modifiers: vec![SemanticTokenModifier::DEPRECATED],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                        },
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        let completions = self.completions.read().await;
        let symbols = self.symbols.read().await;

        let schema_name = self.get_schema_name(&uri);
        let empty_map = HashMap::new();
        let symbol_map = symbols.get(schema_name).unwrap_or(&empty_map);

        if let Some(text) = documents.get(&uri) {
            let mut data = Vec::new();
            let mut prev_line = 0;
            let mut prev_start = 0;

            for (line_idx, line) in text.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                // Naive YAML key detection: "Key:"
                if let Some(colon_idx) = line.find(':') {
                    let key_part = &line[0..colon_idx];
                    let key = key_part.trim();
                    let start_col = line.find(key).unwrap_or(0);

                    // Check if key is known Element (Class) or Property
                    let is_known = completions.iter().any(|c| c.label == key);

                    // Determine token type
                    let (token_type, modifier_bits) = if let Some(info) = symbol_map.get(key) {
                        let token_type = match info.kind {
                            SchemaSymbolKind::Element => 0,
                            SchemaSymbolKind::Attribute => 1,
                            SchemaSymbolKind::EnumValue => 3,
                        };
                        (token_type, if info.deprecated { 1 } else { 0 })
                    } else if is_known {
                        if key.chars().next().is_some_and(|c| c.is_uppercase()) {
                            (0, 0)
                        } else {
                            (1, 0)
                        }
                    } else {
                        (1, 0)
                    };

                    // Relative calculations for Delta encoding
                    let delta_line = line_idx as u32 - prev_line;
                    let delta_start = if delta_line == 0 {
                        start_col as u32 - prev_start
                    } else {
                        start_col as u32
                    };

                    data.push(SemanticToken {
                        delta_line,
                        delta_start,
                        length: key.len() as u32,
                        token_type,
                        token_modifiers_bitset: modifier_bits,
                    });

                    prev_line = line_idx as u32;
                    prev_start = start_col as u32;

                    // Handle Value part if present on same line
                    // "Key: Value"
                    if colon_idx + 1 < line.len() {
                        let val_part = &line[colon_idx + 1..];
                        if !val_part.trim().is_empty() {
                            let value_segment = extract_value_segment(val_part);
                            if !value_segment.is_empty() {
                                let val_start_col = colon_idx
                                    + 1
                                    + val_part.find(|c: char| !c.is_whitespace()).unwrap_or(0);
                                let (value_token_type, value_modifier_bits) =
                                    determine_value_token(value_segment, symbol_map.get(key));
                                let delta_line_val = 0; // Same line
                                let delta_start_val = val_start_col as u32 - prev_start;

                                data.push(SemanticToken {
                                    delta_line: delta_line_val,
                                    delta_start: delta_start_val,
                                    length: value_segment.len() as u32,
                                    token_type: value_token_type,
                                    token_modifiers_bitset: value_modifier_bits,
                                });
                                prev_start = val_start_col as u32;
                            }
                        }
                    }
                }
            }

            return Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
                result_id: None,
                data,
            })));
        }

        Ok(None)
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Wixer LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        {
            let mut docs = self.documents.write().await;
            docs.insert(
                params.text_document.uri.clone(),
                params.text_document.text.clone(),
            );
        }
        self.validate_document(
            params.text_document.uri,
            Some(params.text_document.version),
            params.text_document.text,
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.first() {
            // Invalidate resolved include cache for this file
            let path = PathBuf::from(params.text_document.uri.path());
            self.invalidate_cache(&path).await;

            {
                let mut docs = self.documents.write().await;
                docs.insert(params.text_document.uri.clone(), change.text.clone());
            }
            self.validate_document(
                params.text_document.uri,
                Some(params.text_document.version),
                change.text.clone(),
            )
            .await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let docs_map = self.docs.read().await;
        let documents = self.documents.read().await;

        let uri = &params.text_document_position_params.text_document.uri;
        if let Some(text) = documents.get(uri) {
            let pos = params.text_document_position_params.position;
            if let Some(line) = text.lines().nth(pos.line as usize) {
                // Simple word extraction
                let col = pos.character as usize;
                if col >= line.len() {
                    return Ok(None);
                }

                // Find word boundaries
                let start = line[..col]
                    .rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let end = line[col..]
                    .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
                    .map(|i| col + i)
                    .unwrap_or(line.len());

                let word = &line[start..end];

                if let Some(desc) = docs_map.get(word) {
                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!("**{}**\n\n{}", word, desc),
                        }),
                        range: None,
                    }));
                }
            }
        }
        Ok(None)
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        let completions = self.completions.read().await;
        self.client
            .log_message(
                MessageType::LOG,
                format!("Providing {} completions", completions.len()),
            )
            .await;
        Ok(Some(CompletionResponse::Array(completions.clone())))
    }
}

pub async fn run() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = tower_lsp::LspService::new(WixerLsp::new);
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
    Ok(())
}
