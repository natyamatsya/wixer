use crate::dirs;
use anyhow::Result;
use roxmltree::Document;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use walkdir::WalkDir;

pub struct SchemaMapper {
    wix_defs: HashMap<String, Value>, // Element Name -> Schema definition
    msix_elements: HashMap<String, String>, // Element Name -> Source XSD file
}

impl SchemaMapper {
    pub fn new() -> Result<Self> {
        let cache = dirs::get_cache_dir()?;
        
        // Load WiX definitions
        let wix_json_path = cache.join("json-schema/wix.schema.json");
        let wix_content = fs::read_to_string(wix_json_path)?;
        let wix_schema: Value = serde_json::from_str(&wix_content)?;
        
        let mut wix_defs = HashMap::new();
        if let Some(Value::Object(defs)) = wix_schema.get("definitions") {
            for (k, v) in defs {
                wix_defs.insert(k.clone(), v.clone());
            }
        }

        // Load MSIX definitions
        let mut msix_elements = HashMap::new();
        let msix_root = cache.join("msix-xsd-schema");
        
        if msix_root.exists() {
            for entry in WalkDir::new(&msix_root).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "xsd") {
                    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                    if let Ok(content) = fs::read_to_string(path)
                        && let Ok(doc) = Document::parse(&content) {
                            for node in doc.descendants() {
                                if node.tag_name().name() == "element"
                                    && let Some(name) = node.attribute("name") {
                                        msix_elements.insert(name.to_string(), file_name.clone());
                                    }
                                if node.tag_name().name() == "complexType"
                                    && let Some(name) = node.attribute("name") {
                                        let clean_name = name.strip_prefix("CT_").unwrap_or(name);
                                        msix_elements.entry(clean_name.to_string()).or_insert_with(|| file_name.clone());
                                    }
                            }
                        }
                }
            }
        }

        Ok(Self {
            wix_defs,
            msix_elements,
        })
    }

    pub fn get_mapping(&self) -> Vec<(String, String, String)> {
        let mut matches = Vec::new();
        for wix_name in self.wix_defs.keys() {
            if let Some(source) = self.msix_elements.get(wix_name) {
                matches.push((wix_name.clone(), wix_name.clone(), source.clone()));
            } else {
                let lower_wix = wix_name.to_lowercase();
                for (msix_name, source) in &self.msix_elements {
                    if msix_name.to_lowercase() == lower_wix {
                        matches.push((wix_name.clone(), msix_name.clone(), source.clone()));
                        break;
                    }
                }
            }
        }
        matches.sort_by(|a, b| a.0.cmp(&b.0));
        matches
    }
}
