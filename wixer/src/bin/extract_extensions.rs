//! Build-time tool to extract WiX extension action definitions from .wxi source files
//!
//! Parses the WiX extension wixlib directories and extracts CustomAction and
//! InstallExecuteSequence definitions into JSON for use at runtime.
//!
//! Usage: cargo run --bin extract_extensions -- <wix_src_dir> <output_dir>

use anyhow::{Context, Result};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A custom action definition extracted from WiX source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedCustomAction {
    pub id: String,
    pub dll_entry: Option<String>,
    pub execute: Option<String>,
    pub r#return: Option<String>,
    pub impersonate: Option<String>,
    pub binary_ref: Option<String>,
    pub suppress_modularization: bool,
    pub hide_target: bool,
}

/// A sequence entry for a custom action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSequenceEntry {
    pub action: String,
    pub sequence_table: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub condition: Option<String>,
}

/// Complete extension data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedExtension {
    pub id: String,
    pub namespace: Option<String>,
    pub custom_actions: Vec<ExtractedCustomAction>,
    pub sequence_entries: Vec<ExtractedSequenceEntry>,
    pub properties: Vec<String>,
    pub binaries: Vec<String>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <wix_src_dir> <output_dir>", args[0]);
        eprintln!(
            "Example: {} submodules/wix/src wixer/assets/extensions",
            args[0]
        );
        std::process::exit(1);
    }

    let wix_src = PathBuf::from(&args[1]);
    let output_dir = PathBuf::from(&args[2]);

    fs::create_dir_all(&output_dir)?;

    let ext_dir = wix_src.join("ext");
    if !ext_dir.exists() {
        anyhow::bail!("Extension directory not found: {}", ext_dir.display());
    }

    println!("Scanning WiX extensions in: {}", ext_dir.display());

    let extensions = [
        ("Util", "http://wixtoolset.org/schemas/v4/wxs/util"),
        ("UI", "http://wixtoolset.org/schemas/v4/wxs/ui"),
        ("Firewall", "http://wixtoolset.org/schemas/v4/wxs/firewall"),
        ("NetFx", "http://wixtoolset.org/schemas/v4/wxs/netfx"),
        ("Iis", "http://wixtoolset.org/schemas/v4/wxs/iis"),
        ("Sql", "http://wixtoolset.org/schemas/v4/wxs/sql"),
        ("Bal", "http://wixtoolset.org/schemas/v4/wxs/bal"),
    ];

    for (ext_name, namespace) in extensions {
        let wixlib_dir = ext_dir.join(ext_name).join("wixlib");
        if !wixlib_dir.exists() {
            println!("  Skipping {} (no wixlib directory)", ext_name);
            continue;
        }

        println!("  Processing {}...", ext_name);
        match extract_extension(&wixlib_dir, ext_name, namespace) {
            Ok(ext_data) => {
                let output_file = output_dir.join(format!("{}.json", ext_name.to_lowercase()));
                let json = serde_json::to_string_pretty(&ext_data)?;
                fs::write(&output_file, json)?;
                println!(
                    "    -> {} actions, {} sequence entries -> {}",
                    ext_data.custom_actions.len(),
                    ext_data.sequence_entries.len(),
                    output_file.display()
                );
            }
            Err(e) => {
                eprintln!("    Error processing {}: {}", ext_name, e);
            }
        }
    }

    // Create an index file
    let index: Vec<String> = extensions
        .iter()
        .map(|(name, _)| name.to_lowercase())
        .collect();
    let index_file = output_dir.join("index.json");
    fs::write(&index_file, serde_json::to_string_pretty(&index)?)?;
    println!("\nCreated index: {}", index_file.display());

    Ok(())
}

fn extract_extension(wixlib_dir: &Path, name: &str, namespace: &str) -> Result<ExtractedExtension> {
    let mut extension = ExtractedExtension {
        id: format!("WixToolset.{}.wixext", name),
        namespace: Some(namespace.to_string()),
        custom_actions: Vec::new(),
        sequence_entries: Vec::new(),
        properties: Vec::new(),
        binaries: Vec::new(),
    };

    // Find all .wxi and .wxs files
    for entry in fs::read_dir(wixlib_dir)? {
        let entry = entry?;
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if (ext == "wxi" || ext == "wxs")
            && let Err(e) = parse_wix_file(&path, &mut extension)
        {
            eprintln!("      Warning: Failed to parse {}: {}", path.display(), e);
        }
    }

    Ok(extension)
}

fn parse_wix_file(path: &Path, extension: &mut ExtractedExtension) -> Result<()> {
    let content = fs::read_to_string(path).context("Failed to read file")?;

    // Skip files with preprocessor variables that would break parsing
    // We'll handle the template expansion manually
    let content = preprocess_wix_content(&content);

    let doc = Document::parse(&content).context("Failed to parse XML")?;

    for node in doc.descendants() {
        match node.tag_name().name() {
            "CustomAction" => {
                if let Some(ca) = extract_custom_action(&node) {
                    extension.custom_actions.push(ca);
                }
            }
            "Custom" => {
                if let Some(seq) = extract_sequence_entry(&node) {
                    extension.sequence_entries.push(seq);
                }
            }
            "Property" => {
                if let Some(id) = node.attribute("Id") {
                    let id = expand_vars(id);
                    if !id.contains("$(") && !extension.properties.contains(&id) {
                        extension.properties.push(id);
                    }
                }
            }
            "Binary" => {
                if let Some(id) = node.attribute("Id") {
                    let id = expand_vars(id);
                    if !id.contains("$(") && !extension.binaries.contains(&id) {
                        extension.binaries.push(id);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn preprocess_wix_content(content: &str) -> String {
    // Remove preprocessor directives that would break XML parsing
    let mut result = String::new();
    let mut skip_until_endforeach: i32 = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("<?foreach") {
            skip_until_endforeach += 1;
            continue;
        }
        if trimmed.starts_with("<?endforeach") {
            skip_until_endforeach = skip_until_endforeach.saturating_sub(1);
            continue;
        }
        if trimmed.starts_with("<?include") || trimmed.starts_with("<?define") {
            continue;
        }

        if skip_until_endforeach == 0 {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

fn extract_custom_action(node: &Node) -> Option<ExtractedCustomAction> {
    let id = node.attribute("Id")?;
    let id = expand_vars(id);

    // Skip virtual/template actions
    if id.contains("$(") {
        return None;
    }

    Some(ExtractedCustomAction {
        id,
        dll_entry: node.attribute("DllEntry").map(|s| s.to_string()),
        execute: node.attribute("Execute").map(|s| s.to_string()),
        r#return: node.attribute("Return").map(|s| s.to_string()),
        impersonate: node.attribute("Impersonate").map(|s| s.to_string()),
        binary_ref: node.attribute("BinaryRef").map(expand_vars),
        suppress_modularization: node.attribute("SuppressModularization") == Some("yes"),
        hide_target: node.attribute("HideTarget") == Some("yes"),
    })
}

fn extract_sequence_entry(node: &Node) -> Option<ExtractedSequenceEntry> {
    let action = node.attribute("Action")?;
    let action = expand_vars(action);

    // Skip virtual/template actions
    if action.contains("$(") {
        return None;
    }

    // Determine sequence table from parent
    let sequence_table = node
        .ancestors()
        .find(|n| {
            matches!(
                n.tag_name().name(),
                "InstallExecuteSequence"
                    | "InstallUISequence"
                    | "AdminExecuteSequence"
                    | "AdminUISequence"
                    | "AdvertiseExecuteSequence"
            )
        })
        .map(|n| n.tag_name().name().to_string())
        .unwrap_or_else(|| "InstallExecuteSequence".to_string());

    // Get condition from text content
    let condition = node
        .text()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Some(ExtractedSequenceEntry {
        action,
        sequence_table,
        before: node.attribute("Before").map(expand_vars),
        after: node.attribute("After").map(expand_vars),
        condition,
    })
}

/// Expand common WiX variables to their typical values
fn expand_vars(s: &str) -> String {
    s.replace("$(var.Prefix)", "Wix4")
        .replace("$(var.Prefix6)", "Wix6")
        .replace("$(var.Suffix)", "_X64") // Default to x64
        .replace("$(WIXUIARCH)", "X64")
        .replace("virtual ", "")
        .trim()
        .to_string()
}
