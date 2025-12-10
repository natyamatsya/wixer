//! Orphan Component Detection
//!
//! Detects WiX components and component groups that are defined but never
//! referenced by any Feature, meaning they won't be installed.

use serde_yaml::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct OrphanWarning {
    pub kind: OrphanKind,
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanKind {
    Component,
    ComponentGroup,
}

impl std::fmt::Display for OrphanKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrphanKind::Component => write!(f, "Component"),
            OrphanKind::ComponentGroup => write!(f, "ComponentGroup"),
        }
    }
}

#[derive(Default)]
struct ReferenceTracker {
    /// All defined Component Ids with their paths
    components: HashMap<String, String>,
    /// All defined ComponentGroup Ids with their paths
    component_groups: HashMap<String, String>,
    /// ComponentGroup -> Components it contains
    group_members: HashMap<String, HashSet<String>>,
    /// Components directly referenced by Features
    feature_component_refs: HashSet<String>,
    /// ComponentGroups directly referenced by Features
    feature_group_refs: HashSet<String>,
}

/// Analyze a WiX YAML document for orphaned components
pub fn find_orphans(yaml: &Value) -> Vec<OrphanWarning> {
    let mut tracker = ReferenceTracker::default();

    // Traverse the YAML tree to collect all definitions and references
    collect_definitions(yaml, &mut tracker, "");

    // Find orphans
    let mut warnings = Vec::new();

    // Expand group references to find all referenced components
    let mut all_referenced_components = tracker.feature_component_refs.clone();
    for group_id in &tracker.feature_group_refs {
        if let Some(members) = tracker.group_members.get(group_id) {
            all_referenced_components.extend(members.iter().cloned());
        }
    }

    // Check for orphaned ComponentGroups
    for (group_id, path) in &tracker.component_groups {
        if !tracker.feature_group_refs.contains(group_id) {
            warnings.push(OrphanWarning {
                kind: OrphanKind::ComponentGroup,
                id: group_id.clone(),
                path: path.clone(),
            });
        }
    }

    // Check for orphaned Components (not in any referenced group and not directly referenced)
    for (comp_id, path) in &tracker.components {
        if !all_referenced_components.contains(comp_id) {
            // Check if this component is in any group at all
            let in_any_group = tracker
                .group_members
                .values()
                .any(|members| members.contains(comp_id));

            // Only warn if not in a referenced group
            if !in_any_group {
                warnings.push(OrphanWarning {
                    kind: OrphanKind::Component,
                    id: comp_id.clone(),
                    path: path.clone(),
                });
            }
        }
    }

    warnings
}

fn collect_definitions(value: &Value, tracker: &mut ReferenceTracker, path: &str) {
    match value {
        Value::Mapping(map) => {
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("");
                let new_path = if path.is_empty() {
                    key_str.to_string()
                } else {
                    format!("{}.{}", path, key_str)
                };

                match key_str {
                    "Component" => collect_components(val, tracker, &new_path, None),
                    "ComponentGroup" => collect_component_groups(val, tracker, &new_path),
                    "Feature" => collect_feature_refs(val, tracker),
                    _ => collect_definitions(val, tracker, &new_path),
                }
            }
        }
        Value::Sequence(seq) => {
            for (i, item) in seq.iter().enumerate() {
                let new_path = format!("{}[{}]", path, i);
                collect_definitions(item, tracker, &new_path);
            }
        }
        _ => {}
    }
}

fn collect_components(
    value: &Value,
    tracker: &mut ReferenceTracker,
    path: &str,
    parent_group: Option<&str>,
) {
    match value {
        Value::Mapping(map) => {
            if let Some(id) = map.get("Id").and_then(|v| v.as_str()) {
                tracker.components.insert(id.to_string(), path.to_string());
                if let Some(group_id) = parent_group {
                    tracker
                        .group_members
                        .entry(group_id.to_string())
                        .or_default()
                        .insert(id.to_string());
                }
            }
            // Also recurse into nested elements
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("");
                if key_str == "Component" {
                    collect_components(
                        val,
                        tracker,
                        &format!("{}.{}", path, key_str),
                        parent_group,
                    );
                }
            }
        }
        Value::Sequence(seq) => {
            for (i, item) in seq.iter().enumerate() {
                collect_components(item, tracker, &format!("{}[{}]", path, i), parent_group);
            }
        }
        _ => {}
    }
}

fn collect_component_groups(value: &Value, tracker: &mut ReferenceTracker, path: &str) {
    match value {
        Value::Mapping(map) => {
            let group_id = map.get("Id").and_then(|v| v.as_str());
            if let Some(id) = group_id {
                tracker
                    .component_groups
                    .insert(id.to_string(), path.to_string());
            }
            // Collect components within this group
            if let Some(components) = map.get("Component") {
                collect_components(
                    components,
                    tracker,
                    &format!("{}.Component", path),
                    group_id,
                );
            }
            // Handle ComponentRef within groups
            if let Some(refs) = map.get("ComponentRef") {
                collect_component_refs_into_group(refs, tracker, group_id);
            }
        }
        Value::Sequence(seq) => {
            for (i, item) in seq.iter().enumerate() {
                collect_component_groups(item, tracker, &format!("{}[{}]", path, i));
            }
        }
        _ => {}
    }
}

fn collect_component_refs_into_group(
    value: &Value,
    tracker: &mut ReferenceTracker,
    group_id: Option<&str>,
) {
    match value {
        Value::Mapping(map) => {
            if let Some(id) = map.get("Id").and_then(|v| v.as_str())
                && let Some(gid) = group_id
            {
                tracker
                    .group_members
                    .entry(gid.to_string())
                    .or_default()
                    .insert(id.to_string());
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_component_refs_into_group(item, tracker, group_id);
            }
        }
        _ => {}
    }
}

fn collect_feature_refs(value: &Value, tracker: &mut ReferenceTracker) {
    match value {
        Value::Mapping(map) => {
            // Direct ComponentRef
            if let Some(refs) = map.get("ComponentRef") {
                collect_ids_into_set(refs, &mut tracker.feature_component_refs);
            }
            // ComponentGroupRef
            if let Some(refs) = map.get("ComponentGroupRef") {
                collect_ids_into_set(refs, &mut tracker.feature_group_refs);
            }
            // Nested Features
            if let Some(nested) = map.get("Feature") {
                collect_feature_refs(nested, tracker);
            }
            // Recurse into all values to find nested refs
            for val in map.values() {
                if let Value::Mapping(_) | Value::Sequence(_) = val {
                    collect_feature_refs(val, tracker);
                }
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_feature_refs(item, tracker);
            }
        }
        _ => {}
    }
}

fn collect_ids_into_set(value: &Value, set: &mut HashSet<String>) {
    match value {
        Value::Mapping(map) => {
            if let Some(id) = map.get("Id").and_then(|v| v.as_str()) {
                set.insert(id.to_string());
            }
        }
        Value::String(s) => {
            set.insert(s.clone());
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_ids_into_set(item, set);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_orphans() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              Package:
                ComponentGroup:
                  Id: MyGroup
                  Component:
                    - Id: Comp1
                    - Id: Comp2
                Feature:
                  Id: MainFeature
                  ComponentGroupRef:
                    Id: MyGroup
            "#,
        )
        .unwrap();

        let orphans = find_orphans(&yaml);
        assert!(orphans.is_empty(), "Expected no orphans: {:?}", orphans);
    }

    #[test]
    fn test_orphan_component_group() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              Package:
                ComponentGroup:
                  Id: OrphanGroup
                  Component:
                    Id: Comp1
                Feature:
                  Id: MainFeature
            "#,
        )
        .unwrap();

        let orphans = find_orphans(&yaml);
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].kind, OrphanKind::ComponentGroup);
        assert_eq!(orphans[0].id, "OrphanGroup");
    }

    #[test]
    fn test_orphan_standalone_component() {
        let yaml: Value = serde_yaml::from_str(
            r#"
            Wix:
              Package:
                Directory:
                  Component:
                    Id: StandaloneComp
                Feature:
                  Id: MainFeature
            "#,
        )
        .unwrap();

        let orphans = find_orphans(&yaml);
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].kind, OrphanKind::Component);
        assert_eq!(orphans[0].id, "StandaloneComp");
    }
}
