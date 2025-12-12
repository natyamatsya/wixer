use crate::dirs;
use crate::embedded_assets;
use crate::simulation::extensions::{ExtensionRegistry, ExtractedExtension};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use tracing::info;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WixProperty {
    pub name: String,
    pub kind: String, // "Property", "Directory", "Preprocessor", "CPack", "Element", "Attribute", "CustomAction"
    pub description: String,
    pub docs_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

pub fn generate() -> Result<()> {
    info!("Generating description database...");

    // 1. Load embedded properties from assets.zip
    let embedded_json = embedded_assets::get_knowledge_base()
        .context("Failed to load embedded knowledge base")?;
    let mut properties: Vec<WixProperty> =
        serde_json::from_str(&embedded_json).context("Failed to parse embedded knowledge base")?;

    // 2. Load properties from JSON schemas
    let cache_dir = dirs::get_cache_dir()?;
    let schema_dir = cache_dir.join("json-schema");
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

    // 3. Load extension data from bundled JSON
    let registry = ExtensionRegistry::load_bundled();
    for ext_name in registry.available() {
        if let Some(ext) = registry.get(ext_name) {
            add_extension_entries(ext, &mut properties);
        }
    }

    // 4. Save to cache
    let output_path = cache_dir.join("describe.json");
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(&output_path)?;
    serde_json::to_writer(file, &properties)?;

    info!(count = properties.len(), file = %output_path.display(), "Description database generated");
    Ok(())
}

fn add_extension_entries(ext: &ExtractedExtension, properties: &mut Vec<WixProperty>) {
    // Extract short extension name (e.g., "Util" from "WixToolset.Util.wixext")
    let ext_short = ext
        .id
        .strip_prefix("WixToolset.")
        .and_then(|s: &str| s.strip_suffix(".wixext"))
        .unwrap_or(&ext.id);

    // Add custom actions
    for ca in &ext.custom_actions {
        let execute_type = ca.execute.as_deref().unwrap_or("immediate");
        let return_type = ca.r#return.as_deref().unwrap_or("check");

        let description = format!(
            "Custom action from {} extension. Execute: {}, Return: {}{}{}",
            ext_short,
            execute_type,
            return_type,
            ca.dll_entry
                .as_ref()
                .map(|e| format!(", DLL Entry: {}", e))
                .unwrap_or_default(),
            if ca.suppress_modularization {
                ", SuppressModularization"
            } else {
                ""
            }
        );

        properties.push(WixProperty {
            name: ca.id.clone(),
            kind: "CustomAction".to_string(),
            description,
            docs_url: None,
            tags: vec![ext_short.to_string(), "Extension".to_string()],
        });
    }

    // Add sequence entries with scheduling info
    for seq in &ext.sequence_entries {
        let scheduling = match (&seq.before, &seq.after) {
            (Some(before), _) => format!("Before {}", before),
            (_, Some(after)) => format!("After {}", after),
            _ => "Default sequence".to_string(),
        };

        let description = format!(
            "Scheduled in {} ({}){}{}",
            seq.sequence_table,
            scheduling,
            seq.condition
                .as_ref()
                .map(|c| format!(", Condition: {}", c))
                .unwrap_or_default(),
            if seq.overridable {
                " [Overridable]"
            } else {
                ""
            }
        );

        // Only add if not already present as CustomAction
        if !properties.iter().any(|p| p.name == seq.action) {
            properties.push(WixProperty {
                name: seq.action.clone(),
                kind: "SequenceEntry".to_string(),
                description,
                docs_url: None,
                tags: vec![ext_short.to_string(), "Extension".to_string()],
            });
        }
    }

    // Add properties defined by extensions
    for prop in &ext.properties {
        if !properties.iter().any(|p| p.name == *prop) {
            properties.push(WixProperty {
                name: prop.clone(),
                kind: "Property".to_string(),
                description: format!("Property defined by {} extension.", ext_short),
                docs_url: None,
                tags: vec![ext_short.to_string(), "Extension".to_string()],
            });
        }
    }
}

pub fn load() -> Result<Vec<WixProperty>> {
    let cache_dir = dirs::get_cache_dir()?;
    let cache_file = cache_dir.join("describe.json");
    if !cache_file.exists() {
        // If it doesn't exist, try to generate it (implicit init) or fall back to embedded
        // For safety, let's return just the embedded ones + warning?
        // Or just error saying "run init"
        // Better: generate it now.
        generate()?;
    }

    let file = File::open(cache_file)?;
    let properties: Vec<WixProperty> = serde_json::from_reader(file)?;
    Ok(properties)
}
