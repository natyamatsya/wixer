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

type HoverDocCache = HashMap<String, Option<String>>;
type HoverCache = HashMap<Url, HoverDocCache>;

pub struct WixerLsp {
    client: Client,
    schemas: Arc<RwLock<HashMap<String, jsonschema::JSONSchema>>>,
    docs: Arc<RwLock<HashMap<String, String>>>,
    docs_urls: Arc<RwLock<HashMap<String, String>>>,
    doc_paths: Arc<RwLock<HashMap<String, Vec<String>>>>,
    hover_cache: Arc<RwLock<HoverCache>>,
    completions: Arc<RwLock<Vec<CompletionItem>>>,
    symbols: Arc<RwLock<HashMap<String, HashMap<String, SchemaSymbolInfo>>>>,
    documents: Arc<RwLock<HashMap<Url, String>>>,
    /// Cache for resolved YAML (with includes merged)
    resolved_cache: Arc<RwLock<HashMap<PathBuf, serde_yaml::Value>>>,
}

fn compute_yaml_path(text: &str, line_index: usize) -> Option<Vec<String>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut current_indent: Option<usize> = None;
    let mut keys: Vec<String> = Vec::new();

    for i in (0..=line_index).rev() {
        let l = lines.get(i)?;
        let trimmed = l.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = l.len().saturating_sub(trimmed.len());

        let key = if let Some(rest) = trimmed.strip_prefix("-") {
            let rest = rest.trim_start();
            extract_yaml_key(rest)
        } else {
            extract_yaml_key(trimmed)
        };

        let Some(key) = key else {
            continue;
        };

        if current_indent.is_none() {
            current_indent = Some(indent);
            keys.push(key);
            continue;
        }

        if let Some(ci) = current_indent
            && indent < ci
        {
            current_indent = Some(indent);
            keys.push(key);
        }
    }

    if keys.is_empty() {
        return None;
    }

    keys.reverse();
    Some(keys)
}

fn extract_yaml_key(s: &str) -> Option<String> {
    let mut key = String::new();
    for c in s.chars() {
        if c == ':' {
            break;
        }
        if c.is_whitespace() {
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            key.push(c);
        } else {
            return None;
        }
    }
    if key.is_empty() { None } else { Some(key) }
}

impl WixerLsp {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            schemas: Arc::new(RwLock::new(HashMap::new())),
            docs: Arc::new(RwLock::new(HashMap::new())),
            docs_urls: Arc::new(RwLock::new(HashMap::new())),
            doc_paths: Arc::new(RwLock::new(HashMap::new())),
            hover_cache: Arc::new(RwLock::new(HashMap::new())),
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

        {
            let mut hover_cache = self.hover_cache.write().await;
            hover_cache.remove(&params.text_document.uri);
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
                let mut hover_cache = self.hover_cache.write().await;
                hover_cache.remove(&params.text_document.uri);
            }

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
        let docs_urls = self.docs_urls.read().await;
        let doc_paths = self.doc_paths.read().await;
        let documents = self.documents.read().await;
        let hover_cache = self.hover_cache.read().await;

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

                let cache_key = format!("{}:{}:{}", pos.line, start, end);
                if let Some(doc_cache) = hover_cache.get(uri)
                    && let Some(cached) = doc_cache.get(&cache_key)
                {
                    if let Some(markdown) = cached {
                        return Ok(Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: markdown.clone(),
                            }),
                            range: None,
                        }));
                    }
                    return Ok(None);
                }

                let schema_name = self.get_schema_name(uri);
                let yaml_path = compute_yaml_path(text, pos.line as usize);

                let mut candidates = Vec::new();
                if word.contains('.') {
                    candidates.push(word.to_string());
                    if !word.starts_with(schema_name)
                        && !word.starts_with(&format!("{}.", schema_name))
                    {
                        candidates.push(format!("{}.{}", schema_name, word));
                    }
                } else {
                    if let Some(path) = yaml_path.as_ref() {
                        // Try suffixes of the YAML path so we can match schema keys even when
                        // the YAML contains container nodes like `Wix`, `Package`, etc.
                        let max_suffix = 4usize.min(path.len());
                        for n in 1..=max_suffix {
                            let suffix = path[path.len() - n..].join(".");
                            candidates.push(format!("{}.{}", schema_name, suffix));
                            candidates.push(suffix);
                        }
                    }
                    candidates.push(format!("{}.{}", schema_name, word));
                    candidates.push(word.to_string());
                }

                for key in &candidates {
                    let desc = docs_map.get(key);
                    let docs_url = docs_urls.get(key);
                    if desc.is_some() || docs_url.is_some() {
                        let yaml_path_str =
                            yaml_path.as_ref().map(|p| p.join(".")).unwrap_or_default();

                        let path_block = if yaml_path_str.is_empty() {
                            format!("**Resolved Path**: `{}`", key)
                        } else {
                            format!(
                                "**YAML Path**: `{}`\n\n**Resolved Path**: `{}`",
                                yaml_path_str, key
                            )
                        };
                        let mut markdown = format!("**{}**\n\n{}", word, path_block);
                        if let Some(desc) = desc {
                            markdown.push_str("\n\n");
                            markdown.push_str(desc);
                        }
                        if let Some(url) = docs_url {
                            markdown.push_str("\n\n**Docs**: ");
                            markdown.push_str(url);
                        }
                        drop(hover_cache);
                        let mut hover_cache = self.hover_cache.write().await;
                        hover_cache
                            .entry(uri.clone())
                            .or_default()
                            .insert(cache_key.clone(), Some(markdown.clone()));
                        return Ok(Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: markdown,
                            }),
                            range: None,
                        }));
                    }
                }

                if let Some(paths) = doc_paths.get(word)
                    && paths.len() > 1
                {
                    let mut lines = Vec::new();
                    lines.push(format!("**{}**", word));
                    lines.push("".to_string());
                    lines.push("Multiple matches. Use a qualified path:".to_string());
                    lines.push("".to_string());
                    for p in paths.iter().take(20) {
                        lines.push(format!("- **Path** `{}`", p));
                    }
                    let markdown = lines.join("\n");
                    drop(hover_cache);
                    let mut hover_cache = self.hover_cache.write().await;
                    hover_cache
                        .entry(uri.clone())
                        .or_default()
                        .insert(cache_key.clone(), Some(markdown.clone()));
                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: markdown,
                        }),
                        range: None,
                    }));
                }

                let desc = docs_map.get(word);
                let docs_url = docs_urls.get(word);
                if desc.is_some() || docs_url.is_some() {
                    let mut markdown = format!("**{}**\n\n**Resolved Path**: `{}`", word, word);
                    if let Some(desc) = desc {
                        markdown.push_str("\n\n");
                        markdown.push_str(desc);
                    }
                    if let Some(url) = docs_url {
                        markdown.push_str("\n\n**Docs**: ");
                        markdown.push_str(url);
                    }
                    drop(hover_cache);
                    let mut hover_cache = self.hover_cache.write().await;
                    hover_cache
                        .entry(uri.clone())
                        .or_default()
                        .insert(cache_key, Some(markdown.clone()));
                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: markdown,
                        }),
                        range: None,
                    }));
                }

                drop(hover_cache);
                let mut hover_cache = self.hover_cache.write().await;
                hover_cache
                    .entry(uri.clone())
                    .or_default()
                    .insert(cache_key, None);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_yaml_path_nested_mapping() {
        let text = r#"
Wix:
  Package:
    CustomAction:
      Id: CA1
"#;

        // 0-based line index: "      Id: CA1" is line 4
        let path = compute_yaml_path(text, 4).unwrap();
        assert_eq!(path, vec!["Wix", "Package", "CustomAction", "Id"]);
    }

    #[test]
    fn compute_yaml_path_sequence_item() {
        let text = r#"
Wix:
  Package:
    CustomAction:
      - Id: CA1
        Execute: immediate
"#;

        // "        Execute: immediate" is line 5
        let path = compute_yaml_path(text, 5).unwrap();
        assert_eq!(
            path,
            vec!["Wix", "Package", "CustomAction", "Id", "Execute"]
        );
    }

    #[test]
    fn compute_yaml_path_skips_comments_and_blank_lines() {
        let text = r#"
Wix:

  # comment
  Package:
    CustomAction:
      Id: CA1
"#;

        // "      Id: CA1" is line 6
        let path = compute_yaml_path(text, 6).unwrap();
        assert_eq!(path, vec!["Wix", "Package", "CustomAction", "Id"]);
    }

    #[test]
    fn extract_yaml_key_rejects_non_key_lines() {
        assert_eq!(extract_yaml_key(""), None);
        assert_eq!(extract_yaml_key("-"), None);
        assert_eq!(extract_yaml_key("123"), Some("123".to_string()));
        assert_eq!(extract_yaml_key("Key:"), Some("Key".to_string()));
        assert_eq!(extract_yaml_key("Key: value"), Some("Key".to_string()));
        assert_eq!(extract_yaml_key("Key-WithDash:"), None);
    }
}
