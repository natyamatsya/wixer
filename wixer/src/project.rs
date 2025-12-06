use anyhow::{Context, Result};
use regex::Regex;
use roxmltree::Document;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
    pub files: HashMap<PathBuf, WixFile>,
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
            files: HashMap::new(),
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
            let is_template = ext == "in";

            if is_wix || (is_template && path.file_stem().and_then(|s| Path::new(s).extension()).is_some()) {
                self.add_file(path)?;
            }
        }
        Ok(())
    }

    pub fn add_file(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
        
        // Pre-process content (CPack variables)
        let re = Regex::new(r"(@[A-Z_]+@)").unwrap();
        let processed_content = re.replace_all(&content, |caps: &regex::Captures| {
            let var = &caps[0];
            if var == "@CPACK_WIX_CUSTOM_XMLNS_EXPANDED@" || var == "@CPACK_WIX_PATCH_XMLNS_EXPANDED@" {
                 format!(" xmlns:ui=\"http://dummy\" xmlns:util=\"http://dummy\" {}=\"1\"", &var[1..var.len()-1])
            } else {
                var.to_string()
            }
        }).to_string();

        self.files.insert(path.to_path_buf(), WixFile {
            path: path.to_path_buf(),
            content: processed_content.clone(), // Store processed content for parsing
        });

        // Parse and index symbols immediately
        if let Ok(doc) = Document::parse(&processed_content) {
            self.index_document(&doc, path);
        }

        Ok(())
    }

    fn index_document(&mut self, doc: &Document, path: &Path) {
        for node in doc.descendants() {
            if let Some(id) = node.attribute("Id") {
                self.symbols.insert(id.to_string(), Symbol {
                    id: id.to_string(),
                    kind: node.tag_name().name().to_string(),
                    file_path: path.to_path_buf(),
                });
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
                                && let Some(filename) = Path::new(source).file_name() {
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
                        && let Some(target) = node.attribute("Target") {
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

    pub fn find_product_info(&self) -> (Option<String>, Option<String>, Option<String>) {
        let mut name = None;
        let mut version = None;
        let mut manufacturer = None;

        for file in self.files.values() {
            if let Ok(doc) = Document::parse(&file.content)
                && let Some(node) = doc.descendants().find(|n| n.tag_name().name() == "Product" || n.tag_name().name() == "Package") {
                    if name.is_none() { name = node.attribute("Name").map(|s| s.to_string()); }
                    if version.is_none() { version = node.attribute("Version").map(|s| s.to_string()); }
                    if manufacturer.is_none() { manufacturer = node.attribute("Manufacturer").map(|s| s.to_string()); }
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
                                && let Some(ext) = child.attribute("Id") {
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
                    if node.tag_name().name() == "File" && node.attribute("Id") == Some(id)
                        && let Some(src) = node.attribute("Source").or_else(|| node.attribute("SourceFile")) {
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
                        && let Some(src) = node.attribute("SourceFile").or_else(|| node.attribute("Source")) {
                            return Some(src.to_string());
                        }
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct FileTypeAssociation {
    pub name: String,
    pub description: Option<String>,
    pub extensions: Vec<String>,
    pub icon: Option<String>,
}
