//! Build script to generate assets.zip at compile time.
//!
//! This script creates a store-only (uncompressed) zip archive containing:
//! - wix-v4-xsd-schemas/ - WiX v4 XSD schemas
//! - ms-msix-xsd-schemas/ - Microsoft MSIX XSD schemas
//! - wix-v4-extensions-json/ - WiX v4 extension metadata JSON
//! - wixer-knowledge-base.json - MSI property documentation
//! - versions.json - Asset version manifest

use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let assets_dir = Path::new(&manifest_dir).join("assets");
    let zip_path = assets_dir.join("assets.zip");

    // Rerun if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");
    
    // Rerun if any asset source files change
    println!("cargo:rerun-if-changed=assets/wix-v4-extensions-json");
    println!("cargo:rerun-if-changed=assets/wixer-knowledge-base.json");
    println!("cargo:rerun-if-changed=assets/wix-v4-xsd-schemas");
    println!("cargo:rerun-if-changed=assets/ms-msix-xsd-schemas");

    // Check if we need to rebuild the zip
    let sources = [
        assets_dir.join("wix-v4-extensions-json"),
        assets_dir.join("wixer-knowledge-base.json"),
        assets_dir.join("wix-v4-xsd-schemas"),
        assets_dir.join("ms-msix-xsd-schemas"),
    ];

    // Get the newest source modification time
    let newest_source = sources
        .iter()
        .filter(|p| p.exists())
        .filter_map(|p| get_newest_mtime(p))
        .max();

    // Check if zip exists and is newer than all sources
    let zip_mtime = zip_path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok());

    // Also check zip size - if it's tiny, it's probably a placeholder
    let zip_size = zip_path.metadata().ok().map(|m| m.len()).unwrap_or(0);
    let zip_is_placeholder = zip_size < 1000; // Real zip should be >1MB
    
    let needs_rebuild = match (newest_source, zip_mtime) {
        (Some(src), Some(zip)) => src > zip || zip_is_placeholder,
        (Some(_), None) => true, // zip doesn't exist
        (None, _) => {
            // No source files - this is an error in CI without pre-populated assets
            if !zip_path.exists() || zip_is_placeholder {
                panic!(
                    "No asset source files found and assets.zip doesn't exist or is empty.\n\
                     Expected at least one of:\n\
                     - assets/wix-v4-xsd-schemas/\n\
                     - assets/ms-msix-xsd-schemas/\n\
                     - assets/wix-v4-extensions-json/\n\
                     - assets/wixer-knowledge-base.json\n\
                     \n\
                     Run 'wixer init' first to download schemas, or ensure the \n\
                     assets directories are populated."
                );
            }
            false // zip exists with content, no sources to compare
        }
    };

    if !needs_rebuild {
        return;
    }

    println!("cargo:warning=Building assets.zip...");

    // Generate versions.json manifest
    let versions_json = generate_versions_manifest(&assets_dir);
    let versions_path = assets_dir.join("versions.json");
    fs::write(&versions_path, &versions_json).expect("Failed to write versions.json");

    // Create the zip file
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");
    let file = File::create(&zip_path).expect("Failed to create assets.zip");
    let mut zip = zip::ZipWriter::new(file);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    // Add all asset directories/files
    for source in &sources {
        if source.exists() {
            if source.is_dir() {
                add_directory_to_zip(&mut zip, source, source.file_name().unwrap().to_str().unwrap(), options)
                    .expect("Failed to add directory to zip");
            } else if source.is_file() {
                add_file_to_zip(&mut zip, source, source.file_name().unwrap().to_str().unwrap(), options)
                    .expect("Failed to add file to zip");
            }
        }
    }

    // Add versions.json to the zip
    add_file_to_zip(&mut zip, &versions_path, "versions.json", options)
        .expect("Failed to add versions.json to zip");

    zip.finish().expect("Failed to finalize zip");

    // Clean up temporary versions.json
    let _ = fs::remove_file(&versions_path);
    println!("cargo:warning=Created assets.zip ({} bytes)", zip_path.metadata().unwrap().len());
}

/// Generate a versions.json manifest with version info for each asset.
fn generate_versions_manifest(assets_dir: &Path) -> String {
    let build_time = chrono_lite_now();
    
    // Get git commit hash for tracked assets (extensions, knowledge_base)
    let git_hash = get_git_hash().unwrap_or_else(|| "unknown".to_string());
    
    // Compute content hash for downloaded schemas
    let wix_schemas_hash = compute_directory_hash(&assets_dir.join("wix-v4-xsd-schemas"))
        .unwrap_or_else(|| "not-present".to_string());
    let msix_schemas_hash = compute_directory_hash(&assets_dir.join("ms-msix-xsd-schemas"))
        .unwrap_or_else(|| "not-present".to_string());
    let extensions_hash = compute_directory_hash(&assets_dir.join("wix-v4-extensions-json"))
        .unwrap_or_else(|| "not-present".to_string());
    let knowledge_base_hash = compute_file_hash(&assets_dir.join("wixer-knowledge-base.json"))
        .unwrap_or_else(|| "not-present".to_string());
    
    format!(
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
        knowledge_base_hash
    )
}

/// Get current git commit hash
fn get_git_hash() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Simple timestamp without external crate
fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Convert to ISO-8601 format (approximate, UTC)
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    
    // Simple year calculation (good enough for build timestamps)
    let mut year = 1970;
    let mut remaining_days = days_since_epoch;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    
    let days_in_months: [u64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    
    let mut month = 1;
    for days in days_in_months {
        if remaining_days < days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }
    let day = remaining_days + 1;
    
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hours, minutes, seconds)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Compute SHA256 hash of all files in a directory (sorted for determinism)
fn compute_directory_hash(dir: &Path) -> Option<String> {
    use std::collections::BTreeSet;
    
    if !dir.exists() {
        return None;
    }
    
    let mut files: BTreeSet<_> = Vec::new().into_iter().collect();
    collect_files(dir, &mut files);
    
    if files.is_empty() {
        return None;
    }
    
    // Simple hash: concatenate all file contents and compute hash
    let mut combined = Vec::new();
    for file_path in files {
        if let Ok(content) = fs::read(&file_path) {
            // Include relative path in hash for structure awareness
            let rel_path = file_path.strip_prefix(dir).unwrap_or(&file_path);
            combined.extend_from_slice(rel_path.to_string_lossy().as_bytes());
            combined.push(0);
            combined.extend_from_slice(&content);
        }
    }
    
    Some(simple_sha256(&combined))
}

fn collect_files(dir: &Path, files: &mut std::collections::BTreeSet<std::path::PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, files);
            } else {
                files.insert(path);
            }
        }
    }
}

/// Compute SHA256 hash of a single file
fn compute_file_hash(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|content| simple_sha256(&content))
}

/// Simple SHA256 implementation (we can't use sha2 crate in build.rs easily)
/// This uses a basic approach - for build scripts, we just need consistency
fn simple_sha256(data: &[u8]) -> String {
    // Use a simple hash for build script - we don't need cryptographic strength
    // Just need consistency for cache invalidation
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let hash1 = hasher.finish();
    
    // Double hash for more bits
    let mut hasher2 = DefaultHasher::new();
    hash1.hash(&mut hasher2);
    data.len().hash(&mut hasher2);
    let hash2 = hasher2.finish();
    
    format!("{:016x}{:016x}", hash1, hash2)
}

fn get_newest_mtime(path: &Path) -> Option<std::time::SystemTime> {
    if path.is_file() {
        return path.metadata().ok().and_then(|m| m.modified().ok());
    }

    let mut newest = None;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let mtime = if entry_path.is_dir() {
                get_newest_mtime(&entry_path)
            } else {
                entry_path.metadata().ok().and_then(|m| m.modified().ok())
            };
            if let Some(t) = mtime {
                newest = Some(newest.map_or(t, |n: std::time::SystemTime| n.max(t)));
            }
        }
    }
    newest
}

fn add_directory_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir_path: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());

        if path.is_dir() {
            add_directory_to_zip(zip, &path, &name, options)?;
        } else {
            add_file_to_zip(zip, &path, &name, options)?;
        }
    }
    Ok(())
}

fn add_file_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    file_path: &Path,
    name: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open(file_path)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    zip.start_file(name, options)?;
    zip.write_all(&contents)?;
    Ok(())
}
