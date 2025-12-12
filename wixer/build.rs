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
    println!("cargo:rerun-if-changed=assets/generated");

    // Check if we need to rebuild the zip
    let sources = [
        assets_dir.join("wix-v4-extensions-json"),
        assets_dir.join("wixer-knowledge-base.json"),
        assets_dir.join("wix-v4-xsd-schemas"),
        assets_dir.join("ms-msix-xsd-schemas"),
        assets_dir.join("generated"),
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
                     - assets/generated/\n\
                     \n\
                     Run 'cargo run --bin prebuild-assets' to generate derived assets."
                );
            }
            false // zip exists with content, no sources to compare
        }
    };

    if !needs_rebuild {
        return;
    }

    println!("cargo:warning=Building assets.zip...");

    // Create the zip file
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");
    let file = File::create(&zip_path).expect("Failed to create assets.zip");
    let mut zip = zip::ZipWriter::new(file);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    // Add all asset directories/files (including generated/ which contains versions.json)
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

    zip.finish().expect("Failed to finalize zip");
    println!("cargo:warning=Created assets.zip ({} bytes)", zip_path.metadata().unwrap().len());
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
