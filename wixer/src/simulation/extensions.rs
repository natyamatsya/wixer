//! WiX Extension loading for simulation
//!
//! Loads extension definitions from bundled JSON files extracted from WiX source.

use super::model::{Action, CustomActionType, SequencePosition, SequenceTable};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Extracted custom action from WiX extension source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedCustomAction {
    pub id: String,
    pub dll_entry: Option<String>,
    pub execute: Option<String>,
    pub r#return: Option<String>,
    pub impersonate: Option<String>,
    pub binary_ref: Option<String>,
    #[serde(default)]
    pub suppress_modularization: bool,
    #[serde(default)]
    pub hide_target: bool,
}

/// Extracted sequence entry from WiX extension source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSequenceEntry {
    pub action: String,
    pub sequence_table: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub condition: Option<String>,
}

/// Complete extension data loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedExtension {
    pub id: String,
    pub namespace: Option<String>,
    pub custom_actions: Vec<ExtractedCustomAction>,
    pub sequence_entries: Vec<ExtractedSequenceEntry>,
    #[serde(default)]
    pub properties: Vec<String>,
    #[serde(default)]
    pub binaries: Vec<String>,
}

impl ExtractedExtension {
    /// Convert to simulation Actions
    pub fn to_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();

        // Build a map of action ID -> sequence entry for scheduling
        let mut scheduling: HashMap<&str, &ExtractedSequenceEntry> = HashMap::new();
        for entry in &self.sequence_entries {
            scheduling.insert(&entry.action, entry);
        }

        for ca in &self.custom_actions {
            let mut action = Action::new_custom(
                &ca.id,
                CustomActionType::DllBinary {
                    dll_entry: ca.dll_entry.clone().unwrap_or_default(),
                    binary_ref: ca.binary_ref.clone().unwrap_or_default(),
                },
            );

            // Set execution options based on execute type
            if let Some(exec) = &ca.execute {
                match exec.as_str() {
                    "deferred" => action.execution.deferred = true,
                    "rollback" => action.execution.rollback = true,
                    "commit" => action.execution.commit = true,
                    _ => {}
                }
            }

            if ca.impersonate == Some("no".to_string()) {
                action.execution.no_impersonate = true;
            }

            if ca.r#return == Some("ignore".to_string()) {
                action.execution.continue_on_error = true;
            }

            // Apply scheduling if available
            if let Some(sched) = scheduling.get(ca.id.as_str()) {
                action.condition = sched.condition.clone();

                if let Some(before) = &sched.before {
                    action.position = Some(SequencePosition::Before(before.clone()));
                } else if let Some(after) = &sched.after {
                    action.position = Some(SequencePosition::After(after.clone()));
                }
            }

            actions.push(action);
        }

        actions
    }

    /// Get actions for a specific sequence table
    pub fn actions_for_table(&self, table: SequenceTable) -> Vec<Action> {
        let table_name = table.name();

        // Filter sequence entries for this table
        let relevant_actions: std::collections::HashSet<&str> = self
            .sequence_entries
            .iter()
            .filter(|e| e.sequence_table == table_name)
            .map(|e| e.action.as_str())
            .collect();

        self.to_actions()
            .into_iter()
            .filter(|a| relevant_actions.contains(a.id.as_str()))
            .collect()
    }
}

/// Registry of loaded extensions
#[derive(Debug, Default)]
pub struct ExtensionRegistry {
    extensions: HashMap<String, ExtractedExtension>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load bundled extensions from embedded JSON
    pub fn load_bundled() -> Self {
        let mut registry = Self::new();

        // Load from embedded assets
        if let Ok(ext) = load_bundled_extension("util") {
            registry.register(ext);
        }
        if let Ok(ext) = load_bundled_extension("ui") {
            registry.register(ext);
        }
        if let Ok(ext) = load_bundled_extension("netfx") {
            registry.register(ext);
        }
        if let Ok(ext) = load_bundled_extension("iis") {
            registry.register(ext);
        }
        if let Ok(ext) = load_bundled_extension("sql") {
            registry.register(ext);
        }
        if let Ok(ext) = load_bundled_extension("firewall") {
            registry.register(ext);
        }

        registry
    }

    /// Register an extension
    pub fn register(&mut self, ext: ExtractedExtension) {
        // Register by multiple names for flexible lookup
        let id_lower = ext.id.to_lowercase();
        self.extensions.insert(ext.id.clone(), ext.clone());
        self.extensions.insert(id_lower, ext.clone());

        // Also register by short name (e.g., "util" for "WixToolset.Util.wixext")
        if let Some(short) = ext.id.strip_prefix("WixToolset.")
            && let Some(short) = short.strip_suffix(".wixext")
        {
            self.extensions.insert(short.to_lowercase(), ext);
        }
    }

    /// Get an extension by name
    pub fn get(&self, name: &str) -> Option<&ExtractedExtension> {
        // Try exact match first
        if let Some(ext) = self.extensions.get(name) {
            return Some(ext);
        }

        // Try lowercase
        if let Some(ext) = self.extensions.get(&name.to_lowercase()) {
            return Some(ext);
        }

        // Try stripping common suffixes/prefixes
        let normalized = name
            .to_lowercase()
            .replace("wixtoolset.", "")
            .replace(".wixext", "");
        self.extensions.get(&normalized)
    }

    /// List available extension names
    pub fn available(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.extensions.values().map(|e| e.id.as_str()).collect();
        names.sort();
        names.dedup();
        names
    }
}

/// Load a bundled extension from embedded JSON
fn load_bundled_extension(name: &str) -> Result<ExtractedExtension> {
    let json = match name {
        "util" => include_str!("../../assets/extensions/util.json"),
        "ui" => include_str!("../../assets/extensions/ui.json"),
        "netfx" => include_str!("../../assets/extensions/netfx.json"),
        "iis" => include_str!("../../assets/extensions/iis.json"),
        "sql" => include_str!("../../assets/extensions/sql.json"),
        "firewall" => include_str!("../../assets/extensions/firewall.json"),
        "bal" => include_str!("../../assets/extensions/bal.json"),
        _ => anyhow::bail!("Unknown bundled extension: {}", name),
    };

    serde_json::from_str(json).context("Failed to parse extension JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_util_extension() {
        let ext = load_bundled_extension("util").unwrap();
        assert_eq!(ext.id, "WixToolset.Util.wixext");
        assert!(!ext.custom_actions.is_empty());
        assert!(!ext.sequence_entries.is_empty());
    }

    #[test]
    fn test_registry_lookup() {
        let registry = ExtensionRegistry::load_bundled();

        // Various ways to look up Util extension
        assert!(registry.get("WixToolset.Util.wixext").is_some());
        assert!(registry.get("util").is_some());
        assert!(registry.get("Util").is_some());
    }

    #[test]
    fn test_to_actions() {
        let ext = load_bundled_extension("util").unwrap();
        let actions = ext.to_actions();

        assert!(!actions.is_empty());

        // Check that CloseApplications action has correct scheduling
        let close_app = actions.iter().find(|a| a.id.contains("CloseApplications"));
        assert!(close_app.is_some());
    }
}
