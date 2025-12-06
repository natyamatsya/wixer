use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::path::Path;
use tracing::info;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WixProperty {
    pub name: String,
    pub kind: String, // "Property", "Directory", "Preprocessor", "CPack", "Element", "Attribute"
    pub description: String,
    pub docs_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

pub fn generate(root: &Path) -> Result<()> {
    info!("Generating description database...");

    // 1. Load embedded properties
    let embedded_json = include_str!("../assets/knowledge_base.json");
    let mut properties: Vec<WixProperty> =
        serde_json::from_str(embedded_json).context("Failed to parse embedded knowledge base")?;

    // 2. Load properties from JSON schemas
    let schema_dir = root.join("cache").join("json-schema");
    if schema_dir.exists() {
        for entry in fs::read_dir(schema_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(json) = serde_json::from_str::<Value>(&content)
                && let Some(definitions) = json.get("definitions").and_then(|d| d.as_object())
            {
                for (name, def) in definitions {
                    // Extract description
                    let description = def
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("No description available.")
                        .to_string();

                    // Determine file source for tags
                    let schema_name = path.file_stem().unwrap().to_string_lossy();
                    let tags = vec!["Element".to_string(), schema_name.replace(".schema", "")];

                    properties.push(WixProperty {
                        name: name.clone(),
                        kind: "Element".to_string(),
                        description,
                        docs_url: None,
                        tags,
                    });
                }
            }
        }
    }

    // 3. Save to cache
    let output_path = root.join("cache").join("describe.json");
    let file = File::create(&output_path)?;
    serde_json::to_writer(file, &properties)?;

    info!(count = properties.len(), file = %output_path.display(), "Description database generated");
    Ok(())
}

pub fn load(root: &Path) -> Result<Vec<WixProperty>> {
    let cache_file = root.join("cache").join("describe.json");
    if !cache_file.exists() {
        // If it doesn't exist, try to generate it (implicit init) or fall back to embedded
        // For safety, let's return just the embedded ones + warning?
        // Or just error saying "run init"
        // Better: generate it now.
        generate(root)?;
    }

    let file = File::open(cache_file)?;
    let properties: Vec<WixProperty> = serde_json::from_reader(file)?;
    Ok(properties)
}
