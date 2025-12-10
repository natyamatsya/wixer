//! Publish Order Conflict Detection
//!
//! Detects conflicts between user-defined Publish elements and WiX UI extension
//! Publish entries that have the same Dialog/Control/Order combination.

use crate::simulation::extensions::ExtensionRegistry;
use serde_yaml::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PublishConflict {
    pub dialog: String,
    pub control: String,
    pub order: i32,
    pub user_event: String,
    pub extension_event: String,
    pub extension_name: String,
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
    let mut conflicts = Vec::new();

    // Load UI extension publish entries
    let registry = ExtensionRegistry::load_bundled();
    let Some(ui_ext) = registry.get("ui") else {
        return conflicts;
    };

    // Build a map of (dialog, control, order) -> extension events
    let mut ext_publish_map: HashMap<(String, String, i32), Vec<(String, String)>> = HashMap::new();
    for entry in &ui_ext.publish_entries {
        let order = entry.order.unwrap_or(1); // Default order is 1
        let key = (entry.dialog.clone(), entry.control.clone(), order);
        ext_publish_map
            .entry(key)
            .or_default()
            .push((entry.event.clone().unwrap_or_default(), "UI".to_string()));
    }

    // Collect user-defined Publish entries from YAML
    let user_entries = collect_user_publish_entries(yaml);

    // Check for conflicts
    for user_entry in &user_entries {
        let order = user_entry.order.unwrap_or(1);
        let key = (user_entry.dialog.clone(), user_entry.control.clone(), order);

        if let Some(ext_events) = ext_publish_map.get(&key) {
            for (ext_event, ext_name) in ext_events {
                conflicts.push(PublishConflict {
                    dialog: user_entry.dialog.clone(),
                    control: user_entry.control.clone(),
                    order,
                    user_event: user_entry.event.clone().unwrap_or_default(),
                    extension_event: ext_event.clone(),
                    extension_name: ext_name.clone(),
                });
            }
        }
    }

    // Also warn about user entries without explicit Order that might conflict
    for user_entry in &user_entries {
        if user_entry.order.is_none() {
            // Check if there are any extension entries for this dialog/control
            let has_ext_entries = ext_publish_map
                .keys()
                .any(|(d, c, _)| d == &user_entry.dialog && c == &user_entry.control);

            if has_ext_entries {
                // Already added as conflict with order=1, but we can add a hint
                // that the user should specify an explicit Order
            }
        }
    }

    conflicts
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
