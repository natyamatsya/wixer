//! Pre-build static analysis for WiX projects
//!
//! Analyzes WiX source files to detect potential issues before building:
//! - ICE80: 32-bit components in 64-bit directories
//! - ICE03: String overflow in CustomAction Target
//! - ICE74: Missing UpgradeCode
//! - XML entity escaping issues in YAML files

use crate::project::WixProject;
use anyhow::{Context, Result};
use roxmltree::Document;
use std::path::Path;
use tracing::info;

/// Analysis issue severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// An analysis issue found in the project
#[derive(Debug)]
pub struct Issue {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl Issue {
    fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            file: None,
            line: None,
        }
    }

    fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
            file: None,
            line: None,
        }
    }

    fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }
}

/// Analysis results
#[derive(Debug, Default)]
pub struct AnalysisResults {
    pub issues: Vec<Issue>,
}

impl AnalysisResults {
    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity == Severity::Warning)
    }

    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }
}

/// Run analysis on a directory
pub fn run(path: &Path) -> Result<()> {
    info!("Analyzing WiX project at: {}", path.display());

    let mut project = WixProject::new();
    project.load_from_directory(path)
        .context("Failed to load project")?;

    let results = analyze_project(&project);

    // Print results
    let error_count = results.errors().count();
    let warning_count = results.warnings().count();

    for issue in &results.issues {
        let prefix = match issue.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARNING",
            Severity::Info => "INFO",
        };

        let location = match (&issue.file, issue.line) {
            (Some(f), Some(l)) => format!("{}:{}", f, l),
            (Some(f), None) => f.clone(),
            _ => String::new(),
        };

        if location.is_empty() {
            println!("{} {}: {}", prefix, issue.code, issue.message);
        } else {
            println!("{} {} [{}]: {}", prefix, issue.code, location, issue.message);
        }
    }

    println!();
    println!("Analysis complete: {} error(s), {} warning(s)", error_count, warning_count);

    if results.has_errors() {
        anyhow::bail!("Analysis found errors");
    }

    Ok(())
}

/// Analyze a WixProject and return issues
pub fn analyze_project(project: &WixProject) -> AnalysisResults {
    let mut results = AnalysisResults::default();

    // Run all analyzers
    check_upgrade_code(project, &mut results);
    check_bitness_mismatch(project, &mut results);
    check_custom_action_length(project, &mut results);
    check_xml_entities_in_yaml(project, &mut results);
    check_condition_syntax(project, &mut results);

    results
}

/// ICE74: Check for missing UpgradeCode
fn check_upgrade_code(project: &WixProject, results: &mut AnalysisResults) {
    let mut has_upgrade_code = false;

    for (path, file) in &project.files {
        let path_str = path.display().to_string();
        
        // Check XML files
        if let Ok(doc) = Document::parse(&file.content) {
            for node in doc.descendants() {
                let name = node.tag_name().name();
                if (name == "Package" || name == "Product") && node.attribute("UpgradeCode").is_some() {
                    has_upgrade_code = true;
                    break;
                }
            }
        }
        
        // Check YAML files (simple string search for UpgradeCode key)
        if path_str.ends_with(".yaml") || path_str.ends_with(".yml") {
            // Look for UpgradeCode: followed by a GUID-like value
            if file.content.contains("UpgradeCode:") {
                has_upgrade_code = true;
            }
        }
        
        if has_upgrade_code {
            break;
        }
    }

    if !has_upgrade_code {
        results.issues.push(Issue::warning(
            "ICE74",
            "No UpgradeCode found. It is strongly recommended to specify an UpgradeCode for your application.",
        ));
    }
}

/// ICE80: Check for 32-bit components in 64-bit directories
fn check_bitness_mismatch(project: &WixProject, results: &mut AnalysisResults) {
    // Known 64-bit standard directories
    let dir_64bit = [
        "ProgramFiles64Folder",
        "System64Folder",
        "CommonFiles64Folder",
    ];

    for (path, file) in &project.files {
        if let Ok(doc) = Document::parse(&file.content) {
            // Find directories that are 64-bit
            let mut is_64bit_dir = std::collections::HashSet::new();
            for dir in &dir_64bit {
                is_64bit_dir.insert(*dir);
            }

            // Also check for directories under 64-bit standard directories
            for node in doc.descendants() {
                if node.tag_name().name() == "Directory" || node.tag_name().name() == "StandardDirectory" {
                    if let Some(id) = node.attribute("Id") {
                        if dir_64bit.contains(&id) {
                            // Mark all child directories as 64-bit
                            mark_child_dirs_64bit(&node, &mut is_64bit_dir);
                        }
                    }
                }
            }

            // Check components
            for node in doc.descendants() {
                if node.tag_name().name() == "Component" {
                    let bitness = node.attribute("Bitness").unwrap_or("default");
                    let is_32bit = bitness == "always32" || bitness == "default";

                    // Find parent directory
                    if let Some(dir_ref) = node.attribute("Directory") {
                        if is_32bit && is_64bit_dir.contains(dir_ref) {
                            let comp_id = node.attribute("Id").unwrap_or("?");
                            results.issues.push(
                                Issue::error(
                                    "ICE80",
                                    format!(
                                        "32-bit Component '{}' uses 64-bit Directory '{}'. Add Bitness=\"always64\" to the Component.",
                                        comp_id, dir_ref
                                    ),
                                )
                                .with_file(path.display().to_string()),
                            );
                        }
                    }
                }
            }
        }
    }
}

fn mark_child_dirs_64bit(node: &roxmltree::Node, _set: &mut std::collections::HashSet<&str>) {
    for child in node.children() {
        if child.tag_name().name() == "Directory" {
            // Note: We can't insert &str from attribute into HashSet<&'static str>
            // This is a limitation - we'd need owned strings for dynamic IDs
            // For now, we only check against known 64-bit standard directories
            mark_child_dirs_64bit(&child, _set);
        }
    }
}

/// ICE03: Check for CustomAction Target string overflow (max 255 chars for Type 51)
fn check_custom_action_length(project: &WixProject, results: &mut AnalysisResults) {
    const MAX_TARGET_LENGTH: usize = 255;

    for (path, file) in &project.files {
        if let Ok(doc) = Document::parse(&file.content) {
            for node in doc.descendants() {
                if node.tag_name().name() == "CustomAction" {
                    if let Some(value) = node.attribute("Value") {
                        if value.len() > MAX_TARGET_LENGTH {
                            let id = node.attribute("Id").unwrap_or("?");
                            results.issues.push(
                                Issue::warning(
                                    "ICE03",
                                    format!(
                                        "CustomAction '{}' Value is {} chars (max {}). This may cause ICE03 string overflow.",
                                        id, value.len(), MAX_TARGET_LENGTH
                                    ),
                                )
                                .with_file(path.display().to_string()),
                            );
                        }
                    }
                    // Also check SetProperty pattern (has Property attribute)
                    if node.attribute("Property").is_some() {
                        if let Some(value) = node.attribute("Value") {
                            if value.len() > MAX_TARGET_LENGTH {
                                let id = node.attribute("Id").unwrap_or("?");
                                results.issues.push(
                                    Issue::warning(
                                        "ICE03",
                                        format!(
                                            "SetProperty '{}' Value is {} chars (max {}). Consider using a shorter value or external file.",
                                            id, value.len(), MAX_TARGET_LENGTH
                                        ),
                                    )
                                    .with_file(path.display().to_string()),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Check for XML entities in YAML files that should be raw characters
fn check_xml_entities_in_yaml(project: &WixProject, results: &mut AnalysisResults) {
    let entities = [
        ("&gt;", ">"),
        ("&lt;", "<"),
        ("&amp;", "&"),
        ("&quot;", "\""),
        ("&apos;", "'"),
    ];

    for (path, file) in &project.files {
        let path_str = path.display().to_string();
        if !path_str.ends_with(".yaml") && !path_str.ends_with(".yml") {
            continue;
        }

        for (entity, replacement) in &entities {
            if file.content.contains(entity) {
                results.issues.push(
                    Issue::warning(
                        "YAML001",
                        format!(
                            "YAML file contains XML entity '{}' which should be '{}'. This may cause double-escaping.",
                            entity, replacement
                        ),
                    )
                    .with_file(path_str.clone()),
                );
            }
        }
    }
}

/// Check condition syntax for common issues
fn check_condition_syntax(project: &WixProject, results: &mut AnalysisResults) {
    for (path, file) in &project.files {
        if let Ok(doc) = Document::parse(&file.content) {
            for node in doc.descendants() {
                // Check Condition attributes
                for attr in node.attributes() {
                    if attr.name().contains("Condition") || attr.name() == "Condition" {
                        let value = attr.value();
                        
                        // Check for unbalanced quotes
                        let quote_count = value.chars().filter(|c| *c == '"').count();
                        if quote_count % 2 != 0 {
                            results.issues.push(
                                Issue::error(
                                    "COND001",
                                    format!(
                                        "Condition has unbalanced quotes: {}",
                                        if value.len() > 50 { &value[..50] } else { value }
                                    ),
                                )
                                .with_file(path.display().to_string()),
                            );
                        }

                        // Check for common typos
                        if value.contains("&&") {
                            results.issues.push(
                                Issue::warning(
                                    "COND002",
                                    "Condition contains '&&' - MSI conditions use 'AND' not '&&'",
                                )
                                .with_file(path.display().to_string()),
                            );
                        }
                        if value.contains("||") {
                            results.issues.push(
                                Issue::warning(
                                    "COND003",
                                    "Condition contains '||' - MSI conditions use 'OR' not '||'",
                                )
                                .with_file(path.display().to_string()),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::WixFile;
    use std::path::PathBuf;

    #[test]
    fn test_check_upgrade_code_missing() {
        let mut project = WixProject::new();
        project.files.insert(
            PathBuf::from("test.wxs"),
            WixFile {
                path: PathBuf::from("test.wxs"),
                content: r#"<?xml version="1.0"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="Test" Version="1.0" Manufacturer="Test"/>
</Wix>"#.to_string(),
            },
        );

        let results = analyze_project(&project);
        assert!(results.issues.iter().any(|i| i.code == "ICE74"));
    }

    #[test]
    fn test_check_upgrade_code_present() {
        let mut project = WixProject::new();
        project.files.insert(
            PathBuf::from("test.wxs"),
            WixFile {
                path: PathBuf::from("test.wxs"),
                content: r#"<?xml version="1.0"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="Test" Version="1.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}"/>
</Wix>"#.to_string(),
            },
        );

        let results = analyze_project(&project);
        assert!(!results.issues.iter().any(|i| i.code == "ICE74"));
    }

    #[test]
    fn test_check_xml_entities_in_yaml() {
        let mut project = WixProject::new();
        project.files.insert(
            PathBuf::from("test.wxs.yaml"),
            WixFile {
                path: PathBuf::from("test.wxs.yaml"),
                content: r#"Package:
  Condition: VERSION &gt;= "3.0""#.to_string(),
            },
        );

        let results = analyze_project(&project);
        assert!(results.issues.iter().any(|i| i.code == "YAML001"));
    }

    #[test]
    fn test_check_condition_syntax() {
        let mut project = WixProject::new();
        // Note: && must be escaped as &amp;&amp; in XML attributes
        project.files.insert(
            PathBuf::from("test.wxs"),
            WixFile {
                path: PathBuf::from("test.wxs"),
                content: r#"<?xml version="1.0"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package>
    <Launch Condition="A &amp;&amp; B" Message="Test"/>
  </Package>
</Wix>"#.to_string(),
            },
        );

        let results = analyze_project(&project);
        // The parser will decode &amp;&amp; to &&, which we should catch
        assert!(results.issues.iter().any(|i| i.code == "COND002"));
    }
}
