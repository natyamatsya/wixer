//! Semantic token generation for LSP
//!
//! Provides syntax highlighting information for WiX YAML files.

use std::collections::HashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use super::WixerLsp;
use super::schema::{SchemaSymbolInfo, SchemaSymbolKind, SchemaValueType};

pub async fn semantic_tokens_full(
    lsp: &WixerLsp,
    params: SemanticTokensParams,
) -> Result<Option<SemanticTokensResult>> {
    let uri = params.text_document.uri;
    let documents = lsp.documents.read().await;
    let completions = lsp.completions.read().await;
    let symbols = lsp.symbols.read().await;

    let schema_name = lsp.get_schema_name(&uri);
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
