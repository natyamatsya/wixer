//! Scheduling Warnings
//!
//! Detects problematic action scheduling patterns like:
//! - Deferred custom actions scheduled after InstallFinalize

use serde_yaml::Value;
use tracing::{debug, warn};

/// Warning about a deferred action scheduled after InstallFinalize
#[derive(Debug, Clone)]
pub struct DeferredAfterFinalizeWarning {
    pub action_id: String,
    pub scheduling: String, // e.g., "After: InstallFinalize"
}

/// Find deferred custom actions scheduled after InstallFinalize
pub fn find_deferred_after_finalize(yaml: &Value) -> Vec<DeferredAfterFinalizeWarning> {
    debug!("Checking for deferred actions after InstallFinalize");
    let mut warnings = Vec::new();

    // Collect all custom actions with their execution type
    let deferred_actions = collect_deferred_actions(yaml);

    // Collect scheduling from InstallExecuteSequence
    let scheduled_after = collect_actions_after_finalize(yaml);

    // Check if any deferred action is scheduled after InstallFinalize
    for (action_id, scheduling) in scheduled_after {
        if deferred_actions.contains(&action_id) {
            warn!(
                action = %action_id,
                scheduling = %scheduling,
                "Deferred action scheduled after InstallFinalize - no rollback on failure"
            );
            warnings.push(DeferredAfterFinalizeWarning {
                action_id,
                scheduling,
            });
        }
    }

    if warnings.is_empty() {
        debug!("No deferred actions after InstallFinalize");
    } else {
        debug!(
            count = warnings.len(),
            "Found deferred actions after InstallFinalize"
        );
    }

    warnings
}

/// Collect IDs of custom actions with Execute: deferred
fn collect_deferred_actions(yaml: &Value) -> Vec<String> {
    let mut deferred = Vec::new();
    collect_deferred_recursive(yaml, &mut deferred);
    deferred
}

fn collect_deferred_recursive(value: &Value, deferred: &mut Vec<String>) {
    match value {
        Value::Mapping(map) => {
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("");
                if key_str == "CustomAction" {
                    collect_deferred_from_custom_action(val, deferred);
                } else {
                    collect_deferred_recursive(val, deferred);
                }
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_deferred_recursive(item, deferred);
            }
        }
        _ => {}
    }
}

fn collect_deferred_from_custom_action(value: &Value, deferred: &mut Vec<String>) {
    match value {
        Value::Mapping(map) => {
            let id = map.get("Id").and_then(|v| v.as_str());
            let execute = map.get("Execute").and_then(|v| v.as_str());

            if let Some(id) = id {
                // Check if Execute is "deferred" (case insensitive)
                if execute
                    .map(|e| e.eq_ignore_ascii_case("deferred"))
                    .unwrap_or(false)
                {
                    deferred.push(id.to_string());
                }
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_deferred_from_custom_action(item, deferred);
            }
        }
        _ => {}
    }
}

/// Collect actions scheduled after InstallFinalize
fn collect_actions_after_finalize(yaml: &Value) -> Vec<(String, String)> {
    let mut after_finalize = Vec::new();
    collect_sequence_recursive(yaml, &mut after_finalize);
    after_finalize
}

fn collect_sequence_recursive(value: &Value, after_finalize: &mut Vec<(String, String)>) {
    match value {
        Value::Mapping(map) => {
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("");
                if key_str == "InstallExecuteSequence" {
                    collect_from_sequence(val, after_finalize);
                } else {
                    collect_sequence_recursive(val, after_finalize);
                }
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_sequence_recursive(item, after_finalize);
            }
        }
        _ => {}
    }
}

fn collect_from_sequence(value: &Value, after_finalize: &mut Vec<(String, String)>) {
    match value {
        Value::Mapping(map) => {
            // Check for Custom entries
            if let Some(custom) = map.get("Custom") {
                collect_from_custom_entries(custom, after_finalize);
            }
            // Recurse into other entries
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("");
                if key_str != "Custom" {
                    collect_from_sequence(val, after_finalize);
                }
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_from_sequence(item, after_finalize);
            }
        }
        _ => {}
    }
}

fn collect_from_custom_entries(value: &Value, after_finalize: &mut Vec<(String, String)>) {
    match value {
        Value::Mapping(map) => {
            let action = map.get("Action").and_then(|v| v.as_str());
            let after = map.get("After").and_then(|v| v.as_str());

            if let (Some(action), Some(after)) = (action, after) {
                // Check if scheduled after InstallFinalize
                if after.eq_ignore_ascii_case("InstallFinalize") {
                    after_finalize.push((action.to_string(), format!("After: {}", after)));
                }
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_from_custom_entries(item, after_finalize);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_deferred_after_finalize() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              Package:
                CustomAction:
                  Id: CA_TooLate
                  Execute: deferred
                  Directory: INSTALLFOLDER
                  ExeCommand: echo test
                InstallExecuteSequence:
                  Custom:
                    Action: CA_TooLate
                    After: InstallFinalize
            "#,
        )
        .unwrap();

        let warnings = find_deferred_after_finalize(&yaml);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].action_id, "CA_TooLate");
    }

    #[test]
    fn test_no_warning_for_before_finalize() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              Package:
                CustomAction:
                  Id: CA_OK
                  Execute: deferred
                  Directory: INSTALLFOLDER
                  ExeCommand: echo test
                InstallExecuteSequence:
                  Custom:
                    Action: CA_OK
                    Before: InstallFinalize
            "#,
        )
        .unwrap();

        let warnings = find_deferred_after_finalize(&yaml);
        assert!(warnings.is_empty());
    }
}
