use crate::utils::find_wixer_root;
use anyhow::Result;
use roxmltree::Document;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use walkdir::WalkDir;

pub fn run() -> Result<()> {
    let wixer_root = find_wixer_root()?;
    let cache = wixer_root.join("cache");
    let wix_json_path = cache.join("json-schema/wix.schema.json");

    println!("Loading WiX schema from {}", wix_json_path.display());
    let wix_content = fs::read_to_string(&wix_json_path)?;
    let wix_schema: Value = serde_json::from_str(&wix_content)?;

    let wix_defs = match wix_schema.get("definitions") {
        Some(Value::Object(defs)) => defs,
        _ => {
            eprintln!("Error: No definitions found in WiX schema");
            return Ok(());
        }
    };

    // Load MSIX Schemas (Recursively)
    let msix_root = cache.join("msix-xsd-schema");
    let mut msix_elements = HashMap::new();

    println!("Loading MSIX schemas from {}", msix_root.display());
    if msix_root.exists() {
        for entry in WalkDir::new(&msix_root) {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "xsd") {
                let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                let content = match fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue, // Skip unreadable
                };

                // Simple parsing to get element names
                if let Ok(doc) = Document::parse(&content) {
                    for node in doc.descendants() {
                        if node.tag_name().name() == "element" {
                            if let Some(name) = node.attribute("name") {
                                msix_elements.insert(name.to_string(), file_name.clone());
                            }
                        }
                        // Also map complexTypes as they often back elements
                        if node.tag_name().name() == "complexType" {
                            if let Some(name) = node.attribute("name") {
                                // Strip CT_ prefix common in MSIX
                                let clean_name = if let Some(stripped) = name.strip_prefix("CT_") {
                                    stripped
                                } else {
                                    name
                                };
                                msix_elements
                                    .entry(clean_name.to_string())
                                    .or_insert_with(|| file_name.clone());
                            }
                        }
                    }
                }
            }
        }
    } else {
        eprintln!("MSIX schemas not found. Run 'wixer init' first.");
        return Ok(());
    }

    println!("\nMapping Analysis:");
    println!(
        "{:<30} | {:<40} | {:<20}",
        "WiX Element", "Potential MSIX Match", "MSIX Source"
    );
    println!("{:-<30}-+-{:-<40}-+-{:-<20}", "", "", "");

    let mut matches = Vec::new();

    for (wix_name, _) in wix_defs {
        // 1. Exact Match
        if let Some(source) = msix_elements.get(wix_name) {
            matches.push((wix_name, wix_name, source));
        } else {
            // 2. Case-insensitive match
            let lower_wix = wix_name.to_lowercase();
            for (msix_name, source) in &msix_elements {
                if msix_name.to_lowercase() == lower_wix {
                    matches.push((wix_name, msix_name, source));
                    break;
                }
            }
        }
    }

    matches.sort_by_key(|k| k.0);

    for (wix_name, match_name, source) in &matches {
        println!("{:<30} | {:<40} | {:<20}", wix_name, match_name, source);
    }

    println!(
        "\nFound {} potential matches out of {} WiX definitions.",
        matches.len(),
        wix_defs.len()
    );

    Ok(())
}
