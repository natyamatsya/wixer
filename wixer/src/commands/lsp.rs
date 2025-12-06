use crate::utils::find_wixer_root;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

pub struct WixerLsp {
    client: Client,
    schemas: Arc<RwLock<HashMap<String, jsonschema::JSONSchema>>>,
    wixer_root: PathBuf,
}

impl WixerLsp {
    pub fn new(client: Client) -> Self {
        let wixer_root = find_wixer_root().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            client,
            schemas: Arc::new(RwLock::new(HashMap::new())),
            wixer_root,
        }
    }

    async fn load_schemas(&self) {
        let schema_dir = self.wixer_root.join("cache").join("json-schema");
        if !schema_dir.exists() {
            self.client
                .log_message(MessageType::WARNING, "Schema directory not found")
                .await;
            return;
        }

        let mut schemas = self.schemas.write().await;
        if let Ok(entries) = fs::read_dir(schema_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json")
                    && let Ok(content) = fs::read_to_string(&path)
                    && let Ok(json) = serde_json::from_str::<Value>(&content)
                    && let Ok(compiled) = jsonschema::JSONSchema::compile(&json)
                {
                    let name = path
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .replace(".schema", "");
                    schemas.insert(name, compiled);
                }
            }
        }
        self.client
            .log_message(
                MessageType::INFO,
                format!("Loaded {} schemas", schemas.len()),
            )
            .await;
    }

    async fn validate_document(&self, uri: Url, version: Option<i32>, text: String) {
        let schemas = self.schemas.read().await;
        
        let path = PathBuf::from(uri.path());
        let filename = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Schema selection strategy:
        // 1. Exact match by filename
        // 2. Match by extension
        // 3. Default to "wix"

        let extension_schema = match ext {
            "wyl" => "wixloc",
            _ => "wix",
        };

        let schema = schemas.get(&filename)
            .or_else(|| schemas.get(extension_schema))
            .or_else(|| schemas.get("wix"));

        if let Some(schema) = schema {
            let diagnostics = match serde_yaml::from_str::<serde_yaml::Value>(&text) {
                Ok(yaml_val) => {
                    if let Ok(mut json_val) = serde_json::to_value(yaml_val) {
                        // Strip xmlns
                        remove_xmlns(&mut json_val);

                        match schema.validate(&json_val) {
                            Ok(_) => vec![],
                            Err(errors) => errors
                                .map(|e| {
                                    let range = find_range(&text, &e.instance_path.to_string());
                                    Diagnostic {
                                        range,
                                        severity: Some(DiagnosticSeverity::ERROR),
                                        code: None,
                                        code_description: None,
                                        source: Some("wixer".to_string()),
                                        message: e.to_string(),
                                        related_information: None,
                                        tags: None,
                                        data: None,
                                    }
                                })
                                .collect(),
                        }
                    } else {
                        vec![] // JSON conversion error
                    }
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
}

fn find_range(text: &str, instance_path: &str) -> Range {
    let segments: Vec<&str> = instance_path.trim_start_matches('/').split('/').collect();
    if segments.is_empty() {
        return Range::default();
    }

    // Try to find the last segment that is not a number (array index)
    let target_key = segments.iter().rev().find(|s| s.parse::<usize>().is_err());

    if let Some(key) = target_key {
        // Simple heuristic: find the first occurrence of "Key:"
        // TODO: Improve this by parsing indentation or path tracking
        let pattern = format!("{}:", key);
        if let Some((line_idx, line)) = text.lines().enumerate().find(|(_, l)| l.contains(&pattern)) {
            let col_idx = line.find(&pattern).unwrap_or(0);
            return Range {
                start: Position::new(line_idx as u32, col_idx as u32),
                end: Position::new(line_idx as u32, (col_idx + pattern.len()) as u32),
            };
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

#[tower_lsp::async_trait]
impl LanguageServer for WixerLsp {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        self.load_schemas().await;
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            ..Default::default()
        })
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
        self.validate_document(
            params.text_document.uri,
            Some(params.text_document.version),
            params.text_document.text,
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.first() {
            self.validate_document(
                params.text_document.uri,
                Some(params.text_document.version),
                change.text.clone(),
            )
            .await;
        }
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
