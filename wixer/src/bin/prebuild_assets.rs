//! Prebuild utility to curate and generate all assets for embedding.
//!
//! This binary is NOT distributed - it's only used during development to:
//! 1. Download WiX and MSIX XSD schemas (if missing)
//! 2. Generate JSON schemas from XSD files
//! 3. Generate describe.json knowledge base
//! 4. Generate known_attributes.json
//!
//! Usage:
//!   cargo run --bin prebuild-assets
//!
//! After running, `cargo build` will create assets.zip with all files.

use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let assets_dir = Path::new(&manifest_dir).join("assets");

    println!("Prebuild assets utility");
    println!("Assets directory: {}", assets_dir.display());

    // Input directories
    let wix_xsd_dir = assets_dir.join("wix-v4-xsd-schemas");
    let msix_xsd_dir = assets_dir.join("ms-msix-xsd-schemas");
    let knowledge_base_path = assets_dir.join("wixer-knowledge-base.json");

    // 1. Download XSD schemas if missing
    println!("\n=== Checking XSD schemas ===");

    if !wix_xsd_dir.exists() || fs::read_dir(&wix_xsd_dir)?.count() == 0 {
        println!("  Downloading WiX v4 XSD schemas...");
        wixer::xsd_to_json::download_wix_schemas(&wix_xsd_dir, false)
            .await
            .context("Failed to download WiX schemas")?;
        println!("  ✓ Downloaded WiX schemas to {}", wix_xsd_dir.display());
    } else {
        println!("  ✓ WiX schemas already present");
    }

    if !msix_xsd_dir.exists() || fs::read_dir(&msix_xsd_dir)?.count() == 0 {
        println!("  Downloading MSIX XSD schemas...");
        wixer::xsd_to_json::download_msix_schemas(&msix_xsd_dir, false)
            .await
            .context("Failed to download MSIX schemas")?;
        println!("  ✓ Downloaded MSIX schemas to {}", msix_xsd_dir.display());
    } else {
        println!("  ✓ MSIX schemas already present");
    }

    // Output directory for generated files
    let generated_dir = assets_dir.join("generated");
    fs::create_dir_all(&generated_dir)?;

    // 2. Generate JSON schemas from XSDs
    println!("\n=== Generating JSON schemas ===");
    let json_schema_dir = generated_dir.join("json-schema");
    fs::create_dir_all(&json_schema_dir)?;

    let mut all_attributes: HashSet<String> = HashSet::new();

    for entry in fs::read_dir(&wix_xsd_dir)? {
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

    // 3. Generate known_attributes.json
    println!("\n=== Generating known_attributes.json ===");
    let attr_path = generated_dir.join("known_attributes.json");
    let sorted_attributes: BTreeSet<_> = all_attributes.into_iter().collect();
    let file = File::create(&attr_path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &sorted_attributes)?;
    println!(
        "  Generated: {} ({} attributes)",
        attr_path.display(),
        sorted_attributes.len()
    );

    // 4. Generate describe.json
    println!("\n=== Generating describe.json ===");
    let describe_path = generated_dir.join("describe.json");

    // Load knowledge base
    let kb_content =
        fs::read_to_string(&knowledge_base_path).context("Failed to read knowledge base")?;
    let mut properties: Vec<wixer::describe_db::WixProperty> = serde_json::from_str(&kb_content)?;

    // Add entries from JSON schemas
    for entry in fs::read_dir(&json_schema_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json")
            && path.to_string_lossy().contains(".schema.json")
            && let Ok(content) = fs::read_to_string(&path)
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(definitions) = json.get("definitions").and_then(|d| d.as_object())
        {
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
    println!(
        "  Generated: {} ({} entries)",
        describe_path.display(),
        properties.len()
    );

    // 5. Generate versions.json manifest
    println!("\n=== Generating versions.json ===");
    let versions_path = generated_dir.join("versions.json");
    let versions_json = generate_versions_manifest(&assets_dir)?;
    fs::write(&versions_path, &versions_json)?;
    println!("  Generated: {}", versions_path.display());

    println!("\n=== Done ===");
    println!("Generated files are in: {}", generated_dir.display());
    println!("\nNext steps:");
    println!("  1. Review the generated files");
    println!("  2. Run 'cargo build' to rebuild assets.zip with generated files");

    Ok(())
}

/// Generate a versions.json manifest with version info for each asset.
fn generate_versions_manifest(assets_dir: &Path) -> Result<String> {
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Get current timestamp
    let build_time = {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;
        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;
        let seconds = time_of_day % 60;

        let mut year = 1970u64;
        let mut remaining_days = days_since_epoch;
        loop {
            let is_leap =
                (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
            let days_in_year = if is_leap { 366 } else { 365 };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        let is_leap =
            (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
        let days_in_months: [u64; 12] = if is_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 1u64;
        for days in days_in_months {
            if remaining_days < days {
                break;
            }
            remaining_days -= days;
            month += 1;
        }
        let day = remaining_days + 1;

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, month, day, hours, minutes, seconds
        )
    };

    // Get git commit hash
    let git_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Compute content hashes
    let wix_schemas_hash = compute_directory_hash(&assets_dir.join("wix-v4-xsd-schemas"));
    let msix_schemas_hash = compute_directory_hash(&assets_dir.join("ms-msix-xsd-schemas"));
    let extensions_hash = compute_directory_hash(&assets_dir.join("wix-v4-extensions-json"));
    let knowledge_base_hash = compute_file_hash(&assets_dir.join("wixer-knowledge-base.json"));
    let generated_hash = compute_directory_hash(&assets_dir.join("generated"));

    Ok(format!(
        r#"{{
  "build_time": "{}",
  "git_commit": "{}",
  "assets": {{
    "wix-v4-xsd-schemas": {{
      "source": "https://github.com/wixtoolset/web/tree/master/src/xsd4",
      "content_hash": "{}"
    }},
    "ms-msix-xsd-schemas": {{
      "source": "https://github.com/microsoft/msix-packaging/tree/master/resources/AppxPackaging/Manifest/Schema",
      "content_hash": "{}"
    }},
    "wix-v4-extensions-json": {{
      "source": "git-tracked",
      "git_commit": "{}",
      "content_hash": "{}"
    }},
    "wixer-knowledge-base.json": {{
      "source": "git-tracked",
      "git_commit": "{}",
      "content_hash": "{}"
    }},
    "generated": {{
      "source": "prebuild-assets",
      "content_hash": "{}"
    }}
  }}
}}"#,
        build_time,
        git_hash,
        wix_schemas_hash,
        msix_schemas_hash,
        git_hash,
        extensions_hash,
        git_hash,
        knowledge_base_hash,
        generated_hash
    ))
}

fn compute_directory_hash(dir: &Path) -> String {
    use std::collections::BTreeSet;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    if !dir.exists() {
        return "not-present".to_string();
    }

    let mut files: BTreeSet<std::path::PathBuf> = BTreeSet::new();
    collect_files_recursive(dir, &mut files);

    if files.is_empty() {
        return "empty".to_string();
    }

    let mut combined = Vec::new();
    for file_path in files {
        if let Ok(content) = fs::read(&file_path) {
            let rel_path = file_path.strip_prefix(dir).unwrap_or(&file_path);
            combined.extend_from_slice(rel_path.to_string_lossy().as_bytes());
            combined.push(0);
            combined.extend_from_slice(&content);
        }
    }

    let mut hasher = DefaultHasher::new();
    combined.hash(&mut hasher);
    let hash1 = hasher.finish();

    let mut hasher2 = DefaultHasher::new();
    hash1.hash(&mut hasher2);
    combined.len().hash(&mut hasher2);
    let hash2 = hasher2.finish();

    format!("{:016x}{:016x}", hash1, hash2)
}

fn collect_files_recursive(dir: &Path, files: &mut std::collections::BTreeSet<std::path::PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files_recursive(&path, files);
            } else {
                files.insert(path);
            }
        }
    }
}

fn compute_file_hash(path: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let Ok(content) = fs::read(path) else {
        return "not-present".to_string();
    };

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    let hash1 = hasher.finish();

    let mut hasher2 = DefaultHasher::new();
    hash1.hash(&mut hasher2);
    content.len().hash(&mut hasher2);
    let hash2 = hasher2.finish();

    format!("{:016x}{:016x}", hash1, hash2)
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
            if ca.suppress_modularization {
                ", SuppressModularization"
            } else {
                ""
            }
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
            if seq.overridable {
                " [Overridable]"
            } else {
                ""
            }
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
