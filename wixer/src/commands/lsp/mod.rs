//! WiX Language Server Protocol Implementation
//!
//! Provides IDE integration features for `.wxs.yaml` files:
//! - Schema validation
//! - Completions
//! - Hover documentation
//! - Semantic tokens
//! - Diagnostics (orphans, conflicts, scheduling warnings, etc.)

mod diagnostics;
mod schema;
mod semantic_tokens;

use crate::constants::{EXT_WIXLOC, EXT_WIXLOC_YAML, EXT_WXL_YAML};
use crate::includes;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

pub use schema::{SchemaSymbolInfo, SchemaSymbolKind, SchemaValueType};

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
}

#[tower_lsp::async_trait]
impl LanguageServer for WixerLsp {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        schema::load_schemas(self).await;
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
        semantic_tokens::semantic_tokens_full(self, params).await
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
        diagnostics::validate_document(
            self,
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
            diagnostics::validate_document(
                self,
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
