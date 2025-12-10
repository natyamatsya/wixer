//! Cross-file upgrade context
//!
//! Collects information across multiple files to enable cross-file transformations.
//! For example, WIXUI_INSTALLDIR property might be in a different file than UIRef.

use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::path::Path;

/// Context collected from analyzing multiple WiX files
#[derive(Debug, Default)]
pub struct UpgradeContext {
    /// Map of property ID to its value (e.g., WIXUI_INSTALLDIR -> MY_INSTALL_LOCATION)
    pub properties: HashMap<String, String>,

    /// Set of UIRef IDs found (e.g., WixUI_InstallDir)
    pub ui_refs: Vec<String>,

    /// Whether any file needs the UI namespace
    pub needs_ui_namespace: bool,
}

impl UpgradeContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze a single file and collect context
    pub fn analyze_content(&mut self, content: &str) -> anyhow::Result<()> {
        // Try to parse - if it fails (e.g., due to preprocessor directives),
        // we'll do a best-effort regex scan
        match Document::parse(content) {
            Ok(doc) => {
                self.analyze_node(&doc.root_element());
            }
            Err(_) => {
                // Fallback: regex-based scanning for preprocessor-heavy files
                self.scan_content_fallback(content);
            }
        }
        Ok(())
    }

    /// Recursively analyze a node and its children
    fn analyze_node(&mut self, node: &Node) {
        let tag = node.tag_name().name();

        // Collect Property values
        if tag == "Property"
            && let (Some(id), Some(value)) = (node.attribute("Id"), node.attribute("Value"))
        {
            self.properties.insert(id.to_string(), value.to_string());
        }

        // Collect UIRef IDs
        if tag == "UIRef"
            && let Some(id) = node.attribute("Id")
            && id.starts_with("WixUI_")
        {
            self.ui_refs.push(id.to_string());
            self.needs_ui_namespace = true;
        }

        // Recurse into children
        for child in node.children() {
            if child.is_element() {
                self.analyze_node(&child);
            }
        }
    }

    /// Fallback scanning using string matching for files that can't be parsed
    fn scan_content_fallback(&mut self, content: &str) {
        // Look for Property elements with WIXUI_INSTALLDIR
        // Pattern: <Property Id="WIXUI_INSTALLDIR" Value="..."/>
        if let Some(start) = content.find("Id=\"WIXUI_INSTALLDIR\"")
            && let Some(value_start) = content[start..].find("Value=\"")
        {
            let value_begin = start + value_start + 7;
            if let Some(value_end) = content[value_begin..].find('"') {
                let value = &content[value_begin..value_begin + value_end];
                self.properties
                    .insert("WIXUI_INSTALLDIR".to_string(), value.to_string());
            }
        }

        // Look for UIRef elements with WixUI_*
        for ui_type in &[
            "WixUI_InstallDir",
            "WixUI_Minimal",
            "WixUI_Advanced",
            "WixUI_FeatureTree",
            "WixUI_Mondo",
        ] {
            let pattern = format!("UIRef Id=\"{}\"", ui_type);
            if content.contains(&pattern) {
                self.ui_refs.push(ui_type.to_string());
                self.needs_ui_namespace = true;
            }
        }
    }

    /// Get the install directory for WixUI_InstallDir
    pub fn get_wixui_install_directory(&self) -> Option<&str> {
        self.properties.get("WIXUI_INSTALLDIR").map(|s| s.as_str())
    }

    /// Check if we have a specific UIRef
    pub fn has_ui_ref(&self, id: &str) -> bool {
        self.ui_refs.iter().any(|r| r == id)
    }
}

/// Analyze multiple files and build a combined context
pub fn analyze_files(paths: &[&Path]) -> anyhow::Result<UpgradeContext> {
    let mut context = UpgradeContext::new();

    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            context.analyze_content(&content)?;
        }
    }

    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_property() {
        let mut ctx = UpgradeContext::new();
        let content = r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product>
    <Property Id="WIXUI_INSTALLDIR" Value="MY_INSTALL_LOCATION"/>
  </Product>
</Wix>"#;
        ctx.analyze_content(content).unwrap();
        assert_eq!(
            ctx.get_wixui_install_directory(),
            Some("MY_INSTALL_LOCATION")
        );
    }

    #[test]
    fn test_analyze_uiref() {
        let mut ctx = UpgradeContext::new();
        let content = r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product>
    <UIRef Id="WixUI_InstallDir"/>
  </Product>
</Wix>"#;
        ctx.analyze_content(content).unwrap();
        assert!(ctx.has_ui_ref("WixUI_InstallDir"));
        assert!(ctx.needs_ui_namespace);
    }

    #[test]
    fn test_fallback_scanning() {
        let mut ctx = UpgradeContext::new();
        // Content with preprocessor that would fail parsing
        let content = r#"<?xml version="1.0"?>
<?include Variables.wxi?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product>
    <Property Id="WIXUI_INSTALLDIR" Value="INSTALLFOLDER"/>
    <UIRef Id="WixUI_InstallDir"/>
  </Product>
</Wix>"#;
        ctx.analyze_content(content).unwrap();
        assert_eq!(ctx.get_wixui_install_directory(), Some("INSTALLFOLDER"));
        assert!(ctx.has_ui_ref("WixUI_InstallDir"));
    }
}
