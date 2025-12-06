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
        let schema_dir = self.wixer_root.join("json-schemas");
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
        // Simple heuristic: use wix.schema.json for everything for now, or try to match filename
        // Ideally we check for a magic comment or configuration

        // Try to find a schema that matches the file name
        let filename = PathBuf::from(uri.path())
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let schema = schemas.get(&filename).or_else(|| schemas.get("wix"));

        if let Some(schema) = schema {
            let diagnostics = if let Ok(yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&text)
            {
                if let Ok(json_val) = serde_json::to_value(yaml_val) {
                    match schema.validate(&json_val) {
                        Ok(_) => vec![],
                        Err(errors) => {
                            errors
                                .map(|e| {
                                    Diagnostic {
                                        range: Range {
                                            start: Position::new(0, 0), // Placeholder, jsonschema doesn't give line/col easily for YAML source
                                            end: Position::new(0, 0),
                                        },
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
                                .collect()
                        }
                    }
                } else {
                    vec![] // JSON conversion error
                }
            } else {
                vec![Diagnostic {
                    range: Range::default(),
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("wixer".to_string()),
                    message: "Invalid YAML syntax".to_string(),
                    related_information: None,
                    tags: None,
                    data: None,
                }]
            };

            self.client
                .publish_diagnostics(uri, diagnostics, version)
                .await;
        }
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
