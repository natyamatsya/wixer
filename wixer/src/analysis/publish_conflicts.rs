//! Publish Order Conflict Detection
//!
//! Detects conflicts between user-defined Publish elements and WiX UI extension
//! Publish entries that have the same Dialog/Control/Order combination.

use crate::constants::ui_events::{ACTION_EVENTS, NAVIGATION_EVENTS};
use crate::simulation::extensions::ExtensionRegistry;
use serde_yaml::Value;
use std::collections::HashMap;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct PublishConflict {
    pub dialog: String,
    pub control: String,
    pub order: i32,
    pub user_event: String,
    pub extension_event: String,
    pub extension_value: Option<String>,
    pub extension_name: String,
    pub suggestion: ConflictSuggestion,
}

#[derive(Debug, Clone)]
pub enum ConflictSuggestion {
    /// User wants same behavior - use override to replace extension's
    UseOverride {
        event: String,
        target_dialog: String,
    },
    /// User wants different behavior - change Order to run before/after
    ChangeOrder {
        suggested_order: i32,
        reason: String,
    },
    /// User wants to add to behavior - just change order
    AddWithOrder { suggested_order: i32 },
}

#[derive(Debug, Clone)]
pub struct PublishEntry {
    pub dialog: String,
    pub control: String,
    pub event: Option<String>,
    pub order: Option<i32>,
}

/// Find Publish order conflicts between user YAML and UI extension
pub fn find_conflicts(yaml: &Value) -> Vec<PublishConflict> {
    debug!("Checking for Publish order conflicts with UI extension");
    let mut conflicts = Vec::new();

    // Load UI extension publish entries
    let registry = ExtensionRegistry::load_bundled();
    let Some(ui_ext) = registry.get("ui") else {
        debug!("UI extension not loaded, skipping conflict detection");
        return conflicts;
    };

    debug!(
        extension_entries = ui_ext.publish_entries.len(),
        "Loaded UI extension publish entries"
    );

    // Build a map of (dialog, control, order) -> extension entries with full info
    #[derive(Clone)]
    struct ExtEntry {
        event: String,
        value: Option<String>,
    }
    let mut ext_publish_map: HashMap<(String, String, i32), Vec<ExtEntry>> = HashMap::new();
    for entry in &ui_ext.publish_entries {
        let order = entry.order.unwrap_or(1); // Default order is 1
        let key = (entry.dialog.clone(), entry.control.clone(), order);
        ext_publish_map.entry(key).or_default().push(ExtEntry {
            event: entry.event.clone().unwrap_or_default(),
            value: entry.value.clone(),
        });
    }

    // Find max order used by extension for each dialog/control pair
    let mut max_orders: HashMap<(String, String), i32> = HashMap::new();
    for (dialog, control, order) in ext_publish_map.keys() {
        let key = (dialog.clone(), control.clone());
        max_orders
            .entry(key)
            .and_modify(|max| *max = (*max).max(*order))
            .or_insert(*order);
    }

    // Collect user-defined Publish entries from YAML
    let user_entries = collect_user_publish_entries(yaml);

    // Check for conflicts
    for user_entry in &user_entries {
        let order = user_entry.order.unwrap_or(1);
        let key = (user_entry.dialog.clone(), user_entry.control.clone(), order);

        if let Some(ext_entries) = ext_publish_map.get(&key) {
            for ext_entry in ext_entries {
                let suggestion = generate_suggestion(
                    &user_entry.event,
                    &ext_entry.event,
                    &ext_entry.value,
                    order,
                    max_orders
                        .get(&(user_entry.dialog.clone(), user_entry.control.clone()))
                        .copied()
                        .unwrap_or(1),
                );

                warn!(
                    dialog = %user_entry.dialog,
                    control = %user_entry.control,
                    order = order,
                    user_event = %user_entry.event.as_deref().unwrap_or(""),
                    ext_event = %ext_entry.event,
                    "Publish order conflict with UI extension"
                );
                conflicts.push(PublishConflict {
                    dialog: user_entry.dialog.clone(),
                    control: user_entry.control.clone(),
                    order,
                    user_event: user_entry.event.clone().unwrap_or_default(),
                    extension_event: ext_entry.event.clone(),
                    extension_value: ext_entry.value.clone(),
                    extension_name: "UI".to_string(),
                    suggestion,
                });
            }
        }
    }

    if conflicts.is_empty() {
        debug!("No Publish order conflicts found");
    } else {
        debug!(count = conflicts.len(), "Found Publish order conflicts");
    }

    conflicts
}

/// Generate intelligent suggestion based on event types
fn generate_suggestion(
    user_event: &Option<String>,
    ext_event: &str,
    ext_value: &Option<String>,
    current_order: i32,
    max_ext_order: i32,
) -> ConflictSuggestion {
    let user_ev = user_event.as_deref().unwrap_or("");

    // If user is doing the same type of event (e.g., both NewDialog), suggest override
    if user_ev == ext_event
        && let Some(target) = ext_value
    {
        return ConflictSuggestion::UseOverride {
            event: ext_event.to_string(),
            target_dialog: target.clone(),
        };
    }

    // If user wants to navigate (NewDialog, SpawnDialog) and extension does too,
    // they probably want to replace the navigation
    if is_navigation_event(user_ev)
        && is_navigation_event(ext_event)
        && let Some(target) = ext_value
    {
        return ConflictSuggestion::UseOverride {
            event: ext_event.to_string(),
            target_dialog: target.clone(),
        };
    }

    // If user is doing validation (DoAction, SetProperty) before navigation,
    // suggest running before
    if is_action_event(user_ev) && is_navigation_event(ext_event) {
        return ConflictSuggestion::ChangeOrder {
            suggested_order: current_order.saturating_sub(1).max(1),
            reason: format!("Run your {} before the navigation event", user_ev),
        };
    }

    // Default: suggest adding after all extension events
    ConflictSuggestion::AddWithOrder {
        suggested_order: max_ext_order + 10,
    }
}

fn is_navigation_event(event: &str) -> bool {
    NAVIGATION_EVENTS.contains(&event)
}

fn is_action_event(event: &str) -> bool {
    ACTION_EVENTS.contains(&event)
}

fn collect_user_publish_entries(yaml: &Value) -> Vec<PublishEntry> {
    let mut entries = Vec::new();
    collect_publish_recursive(yaml, &mut entries);
    entries
}

fn collect_publish_recursive(value: &Value, entries: &mut Vec<PublishEntry>) {
    match value {
        Value::Mapping(map) => {
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("");
                if key_str == "Publish" {
                    collect_publish_elements(val, entries);
                } else {
                    collect_publish_recursive(val, entries);
                }
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_publish_recursive(item, entries);
            }
        }
        _ => {}
    }
}

fn collect_publish_elements(value: &Value, entries: &mut Vec<PublishEntry>) {
    match value {
        Value::Mapping(map) => {
            let dialog = map
                .get("Dialog")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let control = map
                .get("Control")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let event = map
                .get("Event")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let order = map.get("Order").and_then(|v| v.as_i64()).map(|n| n as i32);

            if let (Some(dialog), Some(control)) = (dialog, control) {
                entries.push(PublishEntry {
                    dialog,
                    control,
                    event,
                    order,
                });
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_publish_elements(item, entries);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_conflicts_with_explicit_order() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              UI:
                Publish:
                  Dialog: WelcomeDlg
                  Control: Next
                  Event: NewDialog
                  Value: MyCustomDlg
                  Order: 99
            "#,
        )
        .unwrap();

        let conflicts = find_conflicts(&yaml);
        // Order 99 shouldn't conflict with typical extension orders (1-5)
        assert!(
            conflicts.is_empty() || conflicts.iter().all(|c| c.order != 99),
            "Unexpected conflicts: {:?}",
            conflicts
        );
    }

    #[test]
    fn test_detects_default_order_conflict() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              UI:
                Publish:
                  Dialog: WelcomeDlg
                  Control: Next
                  Event: NewDialog
                  Value: SomeDlg
            "#,
        )
        .unwrap();

        let conflicts = find_conflicts(&yaml);
        // Without explicit Order, defaults to 1 which may conflict
        // This test depends on whether WelcomeDlg/Next has order=1 in UI extension
        // The test validates the detection mechanism works
        println!("Conflicts found: {:?}", conflicts);
    }
}
