use anyhow::{Context, Result};
use regex::Regex;
use roxmltree::Document;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::upgrade::{UpgradeContext, upgrade_content_with_context};

#[derive(Debug, Clone)]
pub struct WixFile {
    pub path: PathBuf,
    pub content: String,
    // We can't store Document here because it references content.
    // We parse on demand or find a way to store it.
    // For now, let's just store content and have helper methods.
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub id: String,
    pub kind: String,
    pub file_path: PathBuf,
}

pub struct WixProject {
    /// Ordered by path so that iteration (and everything derived from it) is deterministic
    pub files: BTreeMap<PathBuf, WixFile>,
    pub symbols: HashMap<String, Symbol>,
}

impl Default for WixProject {
    fn default() -> Self {
        Self::new()
    }
}

impl WixProject {
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            symbols: HashMap::new(),
        }
    }

    pub fn load_from_directory(&mut self, root: &Path) -> Result<()> {
        let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip wixer source/build dirs and hidden dirs
            !name.eq_ignore_ascii_case("wixer")
                && !name.eq_ignore_ascii_case("target")
                && !name.starts_with('.')
        });

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            let is_wix = matches!(ext, "wxs" | "wxi" | "wsi" | "wxl");
            let is_yaml = ext == "yaml" || ext == "yml";
            let is_template = ext == "in";

            // Load WiX XML files
            if is_wix
                || (is_template
                    && path
                        .file_stem()
                        .and_then(|s| Path::new(s).extension())
                        .is_some())
            {
                self.add_file(path)?;
            }
            // Load YAML files that look like WiX YAML (e.g., Product.wxs.yaml)
            else if is_yaml {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if stem.ends_with(".wxs") || stem.ends_with(".wxi") || stem.ends_with(".wxl") {
                    self.add_file(path)?;
                }
            }
        }
        Ok(())
    }

    pub fn add_file(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        // Pre-process content (CPack variables)
        let re = Regex::new(r"(@[A-Za-z0-9_]+@)").unwrap();
        let processed_content = re
            .replace_all(&content, |caps: &regex::Captures| {
                let var = &caps[0];
                if var == "@CPACK_WIX_CUSTOM_XMLNS_EXPANDED@"
                    || var == "@CPACK_WIX_PATCH_XMLNS_EXPANDED@"
                {
                    format!(
                        " xmlns:ui=\"http://dummy\" xmlns:util=\"http://dummy\" {}=\"1\"",
                        &var[1..var.len() - 1]
                    )
                } else {
                    var.to_string()
                }
            })
            .to_string();

        self.files.insert(
            path.to_path_buf(),
            WixFile {
                path: path.to_path_buf(),
                content: processed_content.clone(), // Store processed content for parsing
            },
        );

        // Parse and index symbols immediately
        if let Ok(doc) = Document::parse(&processed_content) {
            self.index_document(&doc, path);
        }

        Ok(())
    }

    fn index_document(&mut self, doc: &Document, path: &Path) {
        for node in doc.descendants() {
            if let Some(id) = node.attribute("Id") {
                self.symbols.insert(
                    id.to_string(),
                    Symbol {
                        id: id.to_string(),
                        kind: node.tag_name().name().to_string(),
                        file_path: path.to_path_buf(),
                    },
                );
            }
        }
    }

    pub fn get_executables(&self) -> Vec<String> {
        let mut exes = Vec::new();
        for file in self.files.values() {
            if let Ok(doc) = Document::parse(&file.content) {
                for node in doc.descendants() {
                    if node.tag_name().name() == "File"
                        && let Some(source) = node.attribute("Source")
                        && source.to_lowercase().ends_with(".exe")
                        && let Some(filename) = Path::new(source).file_name()
                    {
                        exes.push(filename.to_string_lossy().to_string());
                    }
                }
            }
        }
        exes
    }

    pub fn get_shortcut_targets(&self) -> Vec<String> {
        let mut targets = Vec::new();
        for file in self.files.values() {
            if let Ok(doc) = Document::parse(&file.content) {
                for node in doc.descendants() {
                    if node.tag_name().name() == "Shortcut"
                        && let Some(target) = node.attribute("Target")
                    {
                        // Heuristic: clean up [INSTALLFOLDER] or [INSTALL_ROOT] prefix
                        let clean_target = target
                            .replace("[INSTALL_ROOT]", "")
                            .replace("[INSTALLFOLDER]", "");

                        if clean_target.to_lowercase().ends_with(".exe") {
                            targets.push(clean_target);
                        }
                    }
                }
            }
        }
        targets
    }

    /// Collects Name/Version/Manufacturer from the first Product/Package found,
    /// looking at `primary` first (if loaded) and then at the other files in path order.
    pub fn find_product_info(
        &self,
        primary: Option<&Path>,
    ) -> (Option<String>, Option<String>, Option<String>) {
        let mut name = None;
        let mut version = None;
        let mut manufacturer = None;

        let primary_file = primary.and_then(|p| self.files.get(p));
        let other_files = self
            .files
            .iter()
            .filter(|(path, _)| Some(path.as_path()) != primary)
            .map(|(_, file)| file);

        for file in primary_file.into_iter().chain(other_files) {
            if let Ok(doc) = Document::parse(&file.content)
                && let Some(node) = doc
                    .descendants()
                    .find(|n| n.tag_name().name() == "Product" || n.tag_name().name() == "Package")
            {
                if name.is_none() {
                    name = node.attribute("Name").map(|s| s.to_string());
                }
                if version.is_none() {
                    version = node.attribute("Version").map(|s| s.to_string());
                }
                if manufacturer.is_none() {
                    manufacturer = node.attribute("Manufacturer").map(|s| s.to_string());
                }
            }
        }
        (name, version, manufacturer)
    }

    pub fn get_file_associations(&self) -> Vec<FileTypeAssociation> {
        let mut associations = Vec::new();
        for file in self.files.values() {
            if let Ok(doc) = Document::parse(&file.content) {
                for node in doc.descendants() {
                    if node.tag_name().name() == "ProgId" {
                        let name = node.attribute("Id").unwrap_or("").to_string();
                        let description = node.attribute("Description").map(|s| s.to_string());
                        let icon = node.attribute("Icon").map(|s| s.to_string());

                        let mut extensions = Vec::new();
                        for child in node.children() {
                            if child.tag_name().name() == "Extension"
                                && let Some(ext) = child.attribute("Id")
                            {
                                extensions.push(ext.to_string());
                            }
                        }

                        if !extensions.is_empty() {
                            associations.push(FileTypeAssociation {
                                name,
                                description,
                                extensions,
                                icon,
                            });
                        }
                    }
                }
            }
        }
        associations
    }

    pub fn get_file_source(&self, id: &str) -> Option<String> {
        for file in self.files.values() {
            if let Ok(doc) = Document::parse(&file.content) {
                for node in doc.descendants() {
                    if node.tag_name().name() == "File"
                        && node.attribute("Id") == Some(id)
                        && let Some(src) = node
                            .attribute("Source")
                            .or_else(|| node.attribute("SourceFile"))
                    {
                        return Some(src.to_string());
                    }
                }
            }
        }
        None
    }

    pub fn get_app_icon(&self) -> Option<String> {
        // Look for <Icon> element. Prefer one that matches Product Icon pattern or just first one.
        for file in self.files.values() {
            if let Ok(doc) = Document::parse(&file.content) {
                for node in doc.descendants() {
                    if node.tag_name().name() == "Icon"
                        && let Some(src) = node
                            .attribute("SourceFile")
                            .or_else(|| node.attribute("Source"))
                    {
                        return Some(src.to_string());
                    }
                }
            }
        }
        None
    }

    // ========================================================================
    // Upgrade Support
    // ========================================================================

    /// Build an UpgradeContext from all files in the project
    ///
    /// This enables cross-file transformations during v3→v4 upgrade,
    /// such as finding WIXUI_INSTALLDIR property in one file when
    /// UIRef is in another.
    pub fn build_upgrade_context(&self) -> UpgradeContext {
        let mut ctx = UpgradeContext::new();
        for file in self.files.values() {
            // Ignore errors - context building is best-effort
            let _ = ctx.analyze_content(&file.content);
        }
        ctx
    }

    /// Upgrade all files in the project from WiX v3 to v4
    ///
    /// Uses shared cross-file context for transformations that
    /// depend on information from other files.
    ///
    /// Returns a map of original path → upgraded content
    pub fn upgrade_to_v4(&self) -> Result<HashMap<PathBuf, String>> {
        let ctx = self.build_upgrade_context();
        let mut results = HashMap::new();

        for (path, file) in &self.files {
            let upgraded = upgrade_content_with_context(&file.content, &ctx)
                .with_context(|| format!("Failed to upgrade {}", path.display()))?;
            results.insert(path.clone(), upgraded);
        }

        Ok(results)
    }

    /// Upgrade all files and write to an output directory
    ///
    /// Preserves the relative directory structure from the project root.
    pub fn upgrade_to_v4_dir(
        &self,
        project_root: &Path,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>> {
        let upgraded = self.upgrade_to_v4()?;
        let mut output_paths = Vec::new();

        for (input_path, content) in upgraded {
            // Compute relative path from project root
            let relative = input_path.strip_prefix(project_root).unwrap_or(&input_path);
            let output_path = output_dir.join(relative);

            // Create parent directories
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&output_path, content)
                .with_context(|| format!("Failed to write {}", output_path.display()))?;

            output_paths.push(output_path);
        }

        Ok(output_paths)
    }

    /// Check if the project appears to be WiX v3 (needs upgrade)
    pub fn is_v3(&self) -> bool {
        for file in self.files.values() {
            // Check for v3 namespace
            if file
                .content
                .contains("http://schemas.microsoft.com/wix/2006/wi")
            {
                return true;
            }
            // Check for Product element (v3) vs Package (v4)
            if file.content.contains("<Product ") || file.content.contains("<Product>") {
                return true;
            }
        }
        false
    }

    /// Check if the project appears to be WiX v4
    pub fn is_v4(&self) -> bool {
        for file in self.files.values() {
            if file
                .content
                .contains("http://wixtoolset.org/schemas/v4/wxs")
            {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct FileTypeAssociation {
    pub name: String,
    pub description: Option<String>,
    pub extensions: Vec<String>,
    pub icon: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_with_packages(names: &[&str]) -> WixProject {
        let mut project = WixProject::new();
        for name in names {
            let path = PathBuf::from(format!("{name}.wxs"));
            project.files.insert(
                path.clone(),
                WixFile {
                    path,
                    content: format!(
                        r#"<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs"><Package Name="{name}" Version="1.0.0.0" Manufacturer="M"/></Wix>"#
                    ),
                },
            );
        }
        project
    }

    #[test]
    fn find_product_info_prefers_primary_file() {
        let project = project_with_packages(&["a", "b", "c"]);
        let (name, _, _) = project.find_product_info(Some(Path::new("b.wxs")));
        assert_eq!(name.as_deref(), Some("b"));
    }

    #[test]
    fn find_product_info_without_primary_uses_path_order() {
        let project = project_with_packages(&["c", "a", "b"]);
        let (name, _, _) = project.find_product_info(None);
        assert_eq!(name.as_deref(), Some("a"));
        let (name, _, _) = project.find_product_info(Some(Path::new("missing.wxs")));
        assert_eq!(name.as_deref(), Some("a"));
    }

    #[test]
    fn test_is_v3_detection() {
        let mut project = WixProject::new();
        project.files.insert(
            PathBuf::from("test.wxs"),
            WixFile {
                path: PathBuf::from("test.wxs"),
                content: r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Test" Version="1.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package/>
  </Product>
</Wix>"#.to_string(),
            },
        );
        assert!(project.is_v3());
        assert!(!project.is_v4());
    }

    #[test]
    fn test_is_v4_detection() {
        let mut project = WixProject::new();
        project.files.insert(
            PathBuf::from("test.wxs"),
            WixFile {
                path: PathBuf::from("test.wxs"),
                content: r#"<?xml version="1.0"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package ProductCode="*" Name="Test" Version="1.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}"/>
</Wix>"#.to_string(),
            },
        );
        assert!(!project.is_v3());
        assert!(project.is_v4());
    }

    #[test]
    fn test_build_upgrade_context() {
        let mut project = WixProject::new();
        project.files.insert(
            PathBuf::from("Product.wxs"),
            WixFile {
                path: PathBuf::from("Product.wxs"),
                content: r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product>
    <Property Id="WIXUI_INSTALLDIR" Value="MY_INSTALL_LOCATION"/>
  </Product>
</Wix>"#
                    .to_string(),
            },
        );
        project.files.insert(
            PathBuf::from("UI.wxs"),
            WixFile {
                path: PathBuf::from("UI.wxs"),
                content: r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <UIRef Id="WixUI_InstallDir"/>
  </Fragment>
</Wix>"#
                    .to_string(),
            },
        );

        let ctx = project.build_upgrade_context();

        // Context should have property from Product.wxs
        assert_eq!(
            ctx.get_wixui_install_directory(),
            Some("MY_INSTALL_LOCATION")
        );
        // Context should have UIRef from UI.wxs
        assert!(ctx.has_ui_ref("WixUI_InstallDir"));
        assert!(ctx.needs_ui_namespace);
    }

    #[test]
    fn test_upgrade_to_v4() {
        let mut project = WixProject::new();
        project.files.insert(
            PathBuf::from("test.wxs"),
            WixFile {
                path: PathBuf::from("test.wxs"),
                content: r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Test" Version="1.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package Compressed="yes"/>
  </Product>
</Wix>"#.to_string(),
            },
        );

        let upgraded = project.upgrade_to_v4().unwrap();
        let content = upgraded.get(&PathBuf::from("test.wxs")).unwrap();

        // Should have v4 namespace
        assert!(content.contains("http://wixtoolset.org/schemas/v4/wxs"));
        // Should have Package instead of Product
        assert!(content.contains("<Package"));
        assert!(!content.contains("<Product"));
    }
}
