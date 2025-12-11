//! Condition Validation
//!
//! Detects undefined property references in condition expressions.
//! Helps catch typos like "FistInstall" instead of "FirstInstall".

use serde_yaml::Value;
use std::collections::HashSet;
use tracing::{debug, warn};

/// Warning about an undefined property in a condition
#[derive(Debug, Clone)]
pub struct UndefinedPropertyWarning {
    pub property: String,
    pub condition: String,
    pub context: String, // e.g., "CustomAction CA_MyAction"
    pub suggestion: Option<String>,
}

/// Standard MSI properties that are always available
const STANDARD_PROPERTIES: &[&str] = &[
    // Installation state
    "Installed",
    "REINSTALL",
    "REMOVE",
    "PATCH",
    "RESUME",
    "Preselected",
    "UPGRADINGPRODUCTCODE",
    // Common WiX properties
    "WIX_UPGRADE_DETECTED",
    "WIX_DOWNGRADE_DETECTED",
    "NETFRAMEWORK45",
    // System folders
    "ProgramFilesFolder",
    "ProgramFiles64Folder",
    "CommonFilesFolder",
    "SystemFolder",
    "System64Folder",
    "WindowsFolder",
    "TempFolder",
    "AppDataFolder",
    "LocalAppDataFolder",
    "DesktopFolder",
    "StartMenuFolder",
    "ProgramMenuFolder",
    // System info
    "VersionNT",
    "VersionNT64",
    "MsiNTProductType",
    "Privileged",
    "AdminUser",
    "UILevel",
    // Common properties
    "ProductName",
    "ProductVersion",
    "ProductCode",
    "UpgradeCode",
    "Manufacturer",
    "INSTALLFOLDER",
    "INSTALLDIR",
    "TARGETDIR",
];

/// Common installation stage properties (from InstallationStages.wxi.yaml pattern)
const STAGE_PROPERTIES: &[&str] = &[
    "FirstInstall",
    "Upgrading",
    "Uninstalling",
    "Maintenance",
    "RemovingForUpgrade",
];

/// Find undefined properties in conditions
pub fn find_undefined_properties(yaml: &Value) -> Vec<UndefinedPropertyWarning> {
    debug!("Checking for undefined property references in conditions");
    let mut warnings = Vec::new();

    // Collect all defined properties
    let mut defined_properties: HashSet<String> = HashSet::new();

    // Add standard properties
    for prop in STANDARD_PROPERTIES {
        defined_properties.insert(prop.to_string());
    }

    // Add stage properties (commonly used with includes)
    for prop in STAGE_PROPERTIES {
        defined_properties.insert(prop.to_string());
    }

    // Collect user-defined properties from YAML
    collect_defined_properties(yaml, &mut defined_properties);

    debug!(
        defined_count = defined_properties.len(),
        "Collected defined properties"
    );

    // Collect conditions and check for undefined properties
    let conditions = collect_conditions(yaml);

    for (condition, context) in conditions {
        let referenced = extract_property_references(&condition);
        for prop in referenced {
            if !defined_properties.contains(&prop) && !is_likely_valid(&prop) {
                let suggestion = find_similar_property(&prop, &defined_properties);
                warn!(
                    property = %prop,
                    condition = %condition,
                    context = %context,
                    "Undefined property in condition"
                );
                warnings.push(UndefinedPropertyWarning {
                    property: prop,
                    condition: condition.clone(),
                    context: context.clone(),
                    suggestion,
                });
            }
        }
    }

    if warnings.is_empty() {
        debug!("No undefined property references found");
    } else {
        debug!(
            count = warnings.len(),
            "Found undefined property references"
        );
    }

    warnings
}

/// Collect user-defined properties from YAML
fn collect_defined_properties(yaml: &Value, props: &mut HashSet<String>) {
    match yaml {
        Value::Mapping(map) => {
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("");

                // Property elements define properties
                if key_str == "Property" {
                    collect_property_ids(val, props);
                }
                // SetProperty also defines properties
                else if key_str == "SetProperty" {
                    collect_set_property_ids(val, props);
                }
                // CustomAction with Property attribute defines a property
                else if key_str == "CustomAction" {
                    collect_custom_action_properties(val, props);
                }

                collect_defined_properties(val, props);
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_defined_properties(item, props);
            }
        }
        _ => {}
    }
}

fn collect_property_ids(value: &Value, props: &mut HashSet<String>) {
    match value {
        Value::Mapping(map) => {
            if let Some(id) = map.get("Id").and_then(|v| v.as_str()) {
                props.insert(id.to_string());
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_property_ids(item, props);
            }
        }
        _ => {}
    }
}

fn collect_set_property_ids(value: &Value, props: &mut HashSet<String>) {
    match value {
        Value::Mapping(map) => {
            if let Some(id) = map.get("Id").and_then(|v| v.as_str()) {
                props.insert(id.to_string());
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_set_property_ids(item, props);
            }
        }
        _ => {}
    }
}

fn collect_custom_action_properties(value: &Value, props: &mut HashSet<String>) {
    match value {
        Value::Mapping(map) => {
            // Type 51 custom actions set a property
            if let Some(prop) = map.get("Property").and_then(|v| v.as_str()) {
                props.insert(prop.to_string());
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_custom_action_properties(item, props);
            }
        }
        _ => {}
    }
}

/// Collect all conditions from YAML with their context
fn collect_conditions(yaml: &Value) -> Vec<(String, String)> {
    let mut conditions = Vec::new();
    collect_conditions_recursive(yaml, &mut conditions, "");
    conditions
}

fn collect_conditions_recursive(
    value: &Value,
    conditions: &mut Vec<(String, String)>,
    context: &str,
) {
    match value {
        Value::Mapping(map) => {
            // Track context for better error messages
            let mut new_context = context.to_string();
            if let Some(id) = map.get("Id").and_then(|v| v.as_str())
                && let Some(key) = map.keys().next().and_then(|k| k.as_str())
            {
                new_context = format!("{} {}", key, id);
            }
            if let Some(action) = map.get("Action").and_then(|v| v.as_str()) {
                new_context = format!("Custom action {}", action);
            }

            // Check for Condition attribute
            if let Some(cond) = map.get("Condition").and_then(|v| v.as_str()) {
                conditions.push((cond.to_string(), new_context.clone()));
            }

            // Recurse
            for (_key, val) in map {
                collect_conditions_recursive(val, conditions, &new_context);
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_conditions_recursive(item, conditions, context);
            }
        }
        _ => {}
    }
}

/// Extract property references from a condition expression
fn extract_property_references(condition: &str) -> Vec<String> {
    let mut props = Vec::new();

    // Simple tokenization - split on operators and whitespace
    let tokens: Vec<&str> = condition
        .split(|c: char| {
            c.is_whitespace()
                || c == '('
                || c == ')'
                || c == '='
                || c == '<'
                || c == '>'
                || c == '~'
                || c == '#'
                || c == '$'
                || c == '%'
                || c == '?'
                || c == '!'
        })
        .filter(|s| !s.is_empty())
        .collect();

    for token in tokens {
        // Skip operators and keywords
        let upper = token.to_uppercase();
        if matches!(
            upper.as_str(),
            "AND" | "OR" | "NOT" | "XOR" | "EQV" | "IMP" | "TRUE" | "FALSE"
        ) {
            continue;
        }

        // Skip quoted strings
        if token.starts_with('"') || token.starts_with('\'') {
            continue;
        }

        // Skip numbers
        if token.parse::<i64>().is_ok() {
            continue;
        }

        // This looks like a property reference
        if is_valid_property_name(token) {
            props.push(token.to_string());
        }
    }

    props
}

/// Check if a string is a valid MSI property name
fn is_valid_property_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Check if a property is likely valid even if not explicitly defined
fn is_likely_valid(prop: &str) -> bool {
    // Properties starting with _ are internal
    if prop.starts_with('_') {
        return true;
    }
    // All caps properties are often system/environment
    if prop.chars().all(|c| c.is_ascii_uppercase() || c == '_') && prop.len() <= 3 {
        return true;
    }
    // Common patterns
    if prop.starts_with("WIXUI_") || prop.starts_with("WIX_") {
        return true;
    }
    false
}

/// Find a similar property name (for suggestions)
fn find_similar_property(prop: &str, defined: &HashSet<String>) -> Option<String> {
    let prop_lower = prop.to_lowercase();

    for defined_prop in defined {
        let defined_lower = defined_prop.to_lowercase();

        // Exact match ignoring case
        if prop_lower == defined_lower && prop != defined_prop {
            return Some(format!("Did you mean '{}'?", defined_prop));
        }

        // Levenshtein distance <= 2
        if levenshtein(&prop_lower, &defined_lower) <= 2 {
            return Some(format!("Did you mean '{}'?", defined_prop));
        }
    }

    None
}

/// Simple Levenshtein distance
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut dp = vec![vec![0; n + 1]; m + 1];

    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate() {
        *val = j;
    }

    for (i, a_char) in a_chars.iter().enumerate() {
        for (j, b_char) in b_chars.iter().enumerate() {
            let cost = if a_char == b_char { 0 } else { 1 };
            dp[i + 1][j + 1] = (dp[i][j + 1] + 1)
                .min(dp[i + 1][j] + 1)
                .min(dp[i][j] + cost);
        }
    }

    dp[m][n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_typo_in_condition() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              Package:
                CustomAction:
                  Id: CA_Test
                  Execute: deferred
                InstallExecuteSequence:
                  Custom:
                    Action: CA_Test
                    Before: InstallFinalize
                    Condition: FistInstall
            "#,
        )
        .unwrap();

        let warnings = find_undefined_properties(&yaml);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].property, "FistInstall");
        assert!(warnings[0].suggestion.is_some());
        assert!(
            warnings[0]
                .suggestion
                .as_ref()
                .unwrap()
                .contains("FirstInstall")
        );
    }

    #[test]
    fn test_no_warning_for_defined_property() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              Package:
                Property:
                  Id: MY_CUSTOM_PROP
                  Value: "1"
                CustomAction:
                  Id: CA_Test
                  Execute: deferred
                InstallExecuteSequence:
                  Custom:
                    Action: CA_Test
                    Before: InstallFinalize
                    Condition: MY_CUSTOM_PROP
            "#,
        )
        .unwrap();

        let warnings = find_undefined_properties(&yaml);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_no_warning_for_standard_properties() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              Package:
                InstallExecuteSequence:
                  Custom:
                    Action: CA_Test
                    Before: InstallFinalize
                    Condition: Installed AND NOT REMOVE
            "#,
        )
        .unwrap();

        let warnings = find_undefined_properties(&yaml);
        assert!(warnings.is_empty());
    }
}
