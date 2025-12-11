//! Diagnostic generation for LSP
//!
//! Handles validation and diagnostic reporting for WiX YAML files.

use crate::analysis::{
    condition_validation, orphan_detection,
    publish_conflicts::{self, ConflictSuggestion},
    scheduling_warnings,
};
use std::path::PathBuf;
use tower_lsp::lsp_types::*;

use super::WixerLsp;

/// Validate document and publish diagnostics
pub async fn validate_document(lsp: &WixerLsp, uri: Url, version: Option<i32>, text: String) {
    let schemas = lsp.schemas.read().await;

    let path = PathBuf::from(uri.path());
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let extension_schema = lsp.get_schema_name(&uri);

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
            let resolved_yaml = lsp
                .get_resolved_yaml(&path)
                .await
                .unwrap_or_else(|| yaml_val.clone());

            let orphans = orphan_detection::find_orphans(&resolved_yaml);
            for orphan in orphans {
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
                let suggestion = format_suggestion(&conflict);
                diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("publish-order-conflict".to_string())),
                    code_description: None,
                    source: Some("wixer".to_string()),
                    message: format!(
                        "Publish Order conflict: Dialog '{}' Control '{}' Order {} conflicts with {} extension event '{}'. {}",
                        conflict.dialog, conflict.control, conflict.order,
                        conflict.extension_name, conflict.extension_event, suggestion
                    ),
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }

            // Invalid override target detection
            let invalid_overrides = publish_conflicts::find_invalid_overrides(&resolved_yaml);
            for invalid in invalid_overrides {
                let range = find_publish_range(&text, &invalid.dialog, &invalid.control);
                let suggestion = if invalid.available_targets.is_empty() {
                    format!(
                        "No override targets available for Dialog '{}' Control '{}'",
                        invalid.dialog, invalid.control
                    )
                } else {
                    format!(
                        "Available targets: {}",
                        invalid.available_targets.join(", ")
                    )
                };
                diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("invalid-override".to_string())),
                    code_description: None,
                    source: Some("wixer".to_string()),
                    message: format!(
                        "Override target '{}' not found in UI extension for Dialog '{}' Control '{}'. {}",
                        invalid.override_target, invalid.dialog, invalid.control, suggestion
                    ),
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }

            // Deferred actions after InstallFinalize warning
            let deferred_warnings =
                scheduling_warnings::find_deferred_after_finalize(&resolved_yaml);
            for warning in deferred_warnings {
                let range = find_action_range(&text, &warning.action_id);
                diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String(
                        "deferred-after-finalize".to_string(),
                    )),
                    code_description: None,
                    source: Some("wixer".to_string()),
                    message: format!(
                        "Deferred action '{}' scheduled {} runs outside the installation transaction. Failures will not trigger rollback.",
                        warning.action_id, warning.scheduling
                    ),
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }

            // Undefined property references in conditions
            let undefined_props = condition_validation::find_undefined_properties(&resolved_yaml);
            for warning in undefined_props {
                let range = find_condition_range(&text, &warning.condition);
                let suggestion = warning
                    .suggestion
                    .map(|s| format!(" {}", s))
                    .unwrap_or_default();
                diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("undefined-property".to_string())),
                    code_description: None,
                    source: Some("wixer".to_string()),
                    message: format!(
                        "Condition references undefined property '{}' in {}.{}",
                        warning.property, warning.context, suggestion
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

    lsp.client
        .publish_diagnostics(uri, diagnostics, version)
        .await;
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

fn find_condition_range(text: &str, condition: &str) -> Range {
    for (line_idx, line) in text.lines().enumerate() {
        if let Some(cond_pos) = line.find("Condition:") {
            let after_cond = &line[cond_pos + 10..];
            let trimmed = after_cond.trim();
            if trimmed.contains(condition) || condition.contains(trimmed) {
                return Range {
                    start: Position::new(line_idx as u32, cond_pos as u32),
                    end: Position::new(line_idx as u32, line.len() as u32),
                };
            }
        }
    }
    Range::default()
}

fn find_action_range(text: &str, action_id: &str) -> Range {
    for (line_idx, line) in text.lines().enumerate() {
        if let Some(id_pos) = line.find("Id:") {
            let after_id = &line[id_pos + 3..];
            let trimmed = after_id.trim();
            if trimmed == action_id || trimmed.trim_matches(|c| c == '"' || c == '\'') == action_id
            {
                return Range {
                    start: Position::new(line_idx as u32, id_pos as u32),
                    end: Position::new(line_idx as u32, line.len() as u32),
                };
            }
        }
        if let Some(action_pos) = line.find("Action:") {
            let after_action = &line[action_pos + 7..];
            let trimmed = after_action.trim();
            if trimmed == action_id || trimmed.trim_matches(|c| c == '"' || c == '\'') == action_id
            {
                return Range {
                    start: Position::new(line_idx as u32, action_pos as u32),
                    end: Position::new(line_idx as u32, line.len() as u32),
                };
            }
        }
    }
    Range::default()
}

fn find_orphan_range(text: &str, id: &str) -> Range {
    for (line_idx, line) in text.lines().enumerate() {
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

fn format_suggestion(conflict: &publish_conflicts::PublishConflict) -> String {
    match &conflict.suggestion {
        ConflictSuggestion::UseOverride {
            event,
            target_dialog,
        } => {
            format!(
                "Since you're replacing the '{}' to '{}', use `Event: override {}` in your Publish element.",
                event, target_dialog, target_dialog
            )
        }
        ConflictSuggestion::ChangeOrder {
            suggested_order,
            reason,
        } => {
            format!("{}. Try `Order: {}`.", reason, suggested_order)
        }
        ConflictSuggestion::AddWithOrder { suggested_order } => {
            format!(
                "To add your event after the extension's, use `Order: {}`.",
                suggested_order
            )
        }
    }
}

fn find_publish_range(text: &str, dialog: &str, control: &str) -> Range {
    let mut in_publish_block = false;
    let mut found_dialog = false;
    let mut publish_line: Option<usize> = None;

    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with("Publish:") || trimmed.starts_with("- Publish:") {
            in_publish_block = true;
            publish_line = Some(line_idx);
            found_dialog = false;
            continue;
        }

        if in_publish_block {
            if trimmed.starts_with("Dialog:") && trimmed.contains(dialog) {
                found_dialog = true;
            }
            if found_dialog
                && trimmed.starts_with("Control:")
                && trimmed.contains(control)
                && let Some(pub_line) = publish_line
            {
                return Range {
                    start: Position::new(pub_line as u32, 0),
                    end: Position::new(line_idx as u32, line.len() as u32),
                };
            }
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
