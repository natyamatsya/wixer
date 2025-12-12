//! Prebuild utility to generate derived assets for embedding.
//!
//! This binary is NOT distributed - it's only used during development to generate:
//! - JSON schemas from XSD files
//! - describe.json knowledge base
//! - known_attributes.json
//!
//! Usage:
//!   cargo run --bin prebuild-assets
//!
//! This should be run after downloading new XSD schemas to assets/wix-v4-xsd-schemas/

use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| ".".to_string());
    let assets_dir = Path::new(&manifest_dir).join("assets");

    println!("Prebuild assets utility");
    println!("Assets directory: {}", assets_dir.display());

    // Input directories
    let xsd_dir = assets_dir.join("wix-v4-xsd-schemas");
    let _extensions_dir = assets_dir.join("wix-v4-extensions-json");
    let knowledge_base_path = assets_dir.join("wixer-knowledge-base.json");

    // Output directory for generated files
    let generated_dir = assets_dir.join("generated");
    fs::create_dir_all(&generated_dir)?;

    // 1. Generate JSON schemas from XSDs
    println!("\n=== Generating JSON schemas ===");
    let json_schema_dir = generated_dir.join("json-schema");
    fs::create_dir_all(&json_schema_dir)?;

    let mut all_attributes: HashSet<String> = HashSet::new();

    if xsd_dir.exists() {
        for entry in fs::read_dir(&xsd_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "xsd") {
                let xsd_name = path.file_stem().unwrap().to_string_lossy();
                println!("  Converting: {}.xsd", xsd_name);

                let attributes = wixer::xsd_to_json::convert_xsd(
                    path.to_str().unwrap(),
                    None,
                    &json_schema_dir,
                    wixer::common::AttributeStrategy::Inline,
                    true,  // include_descriptions
                    false, // strict_mode
                    None,  // root_element
                )?;

                all_attributes.extend(attributes);
            }
        }
    } else {
        println!("  WARNING: XSD directory not found: {}", xsd_dir.display());
    }

    // 2. Generate known_attributes.json
    println!("\n=== Generating known_attributes.json ===");
    let attr_path = generated_dir.join("known_attributes.json");
    let sorted_attributes: BTreeSet<_> = all_attributes.into_iter().collect();
    let file = File::create(&attr_path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &sorted_attributes)?;
    println!("  Generated: {} ({} attributes)", attr_path.display(), sorted_attributes.len());

    // 3. Generate describe.json
    println!("\n=== Generating describe.json ===");
    let describe_path = generated_dir.join("describe.json");
    
    // Load knowledge base
    let kb_content = fs::read_to_string(&knowledge_base_path)
        .context("Failed to read knowledge base")?;
    let mut properties: Vec<wixer::describe_db::WixProperty> = 
        serde_json::from_str(&kb_content)?;

    // Add entries from JSON schemas
    for entry in fs::read_dir(&json_schema_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") 
            && path.to_string_lossy().contains(".schema.json")
        {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(definitions) = json.get("definitions").and_then(|d| d.as_object()) {
                        let schema_name = path.file_stem().unwrap().to_string_lossy();
                        for (name, def) in definitions {
                            let description = def
                                .get("description")
                                .and_then(|d| d.as_str())
                                .unwrap_or("No description available.")
                                .to_string();

                            let tags = vec!["Element".to_string(), schema_name.replace(".schema", "")];

                            properties.push(wixer::describe_db::WixProperty {
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
        }
    }

    // Add extension entries
    let registry = wixer::simulation::extensions::ExtensionRegistry::load_bundled();
    for ext_name in registry.available() {
        if let Some(ext) = registry.get(ext_name) {
            add_extension_entries(ext, &mut properties);
        }
    }

    // Write describe.json
    let file = File::create(&describe_path)?;
    serde_json::to_writer_pretty(file, &properties)?;
    println!("  Generated: {} ({} entries)", describe_path.display(), properties.len());

    println!("\n=== Done ===");
    println!("Generated files are in: {}", generated_dir.display());
    println!("\nNext steps:");
    println!("  1. Review the generated files");
    println!("  2. Run 'cargo build' to rebuild assets.zip with generated files");

    Ok(())
}

fn add_extension_entries(
    ext: &wixer::simulation::extensions::ExtractedExtension,
    properties: &mut Vec<wixer::describe_db::WixProperty>,
) {
    let ext_short = ext
        .id
        .strip_prefix("WixToolset.")
        .and_then(|s: &str| s.strip_suffix(".wixext"))
        .unwrap_or(&ext.id);

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
            if ca.suppress_modularization { ", SuppressModularization" } else { "" }
        );

        properties.push(wixer::describe_db::WixProperty {
            name: ca.id.clone(),
            kind: "CustomAction".to_string(),
            description,
            docs_url: None,
            tags: vec![ext_short.to_string(), "Extension".to_string()],
        });
    }

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
            if seq.overridable { " [Overridable]" } else { "" }
        );

        if !properties.iter().any(|p| p.name == seq.action) {
            properties.push(wixer::describe_db::WixProperty {
                name: seq.action.clone(),
                kind: "SequenceEntry".to_string(),
                description,
                docs_url: None,
                tags: vec![ext_short.to_string(), "Extension".to_string()],
            });
        }
    }

    for prop in &ext.properties {
        if !properties.iter().any(|p| p.name == *prop) {
            properties.push(wixer::describe_db::WixProperty {
                name: prop.clone(),
                kind: "Property".to_string(),
                description: format!("Property defined by {} extension.", ext_short),
                docs_url: None,
                tags: vec![ext_short.to_string(), "Extension".to_string()],
            });
        }
    }
}
