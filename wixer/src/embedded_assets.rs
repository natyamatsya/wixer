//! Embedded assets for offline/CI operation.
//!
//! This module embeds all wixer assets as a compressed zip archive at compile
//! time so wixer can function without network access (e.g., in CI environments).
//!
//! Contents of assets.zip:
//! - `wix-v4-xsd-schemas/` - WiX v4 XSD schemas
//! - `ms-msix-xsd-schemas/` - Microsoft MSIX XSD schemas  
//! - `wix-v4-extensions-json/` - WiX v4 extension metadata (custom actions, sequences, etc.)
//! - `wixer-knowledge-base.json` - MSI property/variable documentation

use anyhow::{Context, Result};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use tracing::info;
use zip::ZipArchive;

/// Embedded assets.zip containing all wixer assets
const ASSETS_ZIP: &[u8] = include_bytes!("../assets/assets.zip");

/// Extract embedded WiX schemas (wix-v4-xsd-schemas/*) to the specified directory.
/// Returns the list of extracted file paths.
pub fn extract_wix_schemas(output_dir: &Path) -> Result<Vec<PathBuf>> {
    extract_schemas_by_prefix(output_dir, "wix-v4-xsd-schemas/")
}

/// Extract embedded MSIX schemas (ms-msix-xsd-schemas/*) to the specified directory.
/// Returns the list of extracted file paths.
pub fn extract_msix_schemas(output_dir: &Path) -> Result<Vec<PathBuf>> {
    extract_schemas_by_prefix(output_dir, "ms-msix-xsd-schemas/")
}

pub fn extract_all(output_dir: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(output_dir)?;

    let cursor = Cursor::new(ASSETS_ZIP);
    let mut archive =
        ZipArchive::new(cursor).context("Failed to read embedded assets archive")?;

    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.is_dir() {
            continue;
        }

        let name = file.name().to_string();
        let output_path = output_dir.join(&name);

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        std::fs::write(&output_path, &contents)
            .with_context(|| format!("Failed to write {}", output_path.display()))?;

        extracted.push(output_path);
    }

    info!(
        count = extracted.len(),
        "Extracted embedded assets to {}",
        output_dir.display()
    );
    Ok(extracted)
}

/// Extract files from the embedded zip matching a given prefix.
fn extract_schemas_by_prefix(output_dir: &Path, prefix: &str) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(output_dir)?;

    let cursor = Cursor::new(ASSETS_ZIP);
    let mut archive =
        ZipArchive::new(cursor).context("Failed to read embedded assets archive")?;

    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // Skip directories and files not matching our prefix
        if file.is_dir() || !name.starts_with(prefix) {
            continue;
        }

        // Strip the prefix to get the relative path
        let relative_path = name.strip_prefix(prefix).unwrap_or(&name);
        let output_path = output_dir.join(relative_path);

        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Read and write file contents
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        std::fs::write(&output_path, &contents)
            .with_context(|| format!("Failed to write {}", output_path.display()))?;

        extracted.push(output_path);
    }

    info!(
        count = extracted.len(),
        prefix = prefix,
        "Extracted embedded schemas to {}",
        output_dir.display()
    );
    Ok(extracted)
}

/// Check if embedded assets are available (non-empty zip).
pub fn has_embedded_assets() -> bool {
    !ASSETS_ZIP.is_empty()
}

/// List all files in the embedded assets archive.
pub fn list_embedded_assets() -> Result<Vec<String>> {
    let cursor = Cursor::new(ASSETS_ZIP);
    let mut archive =
        ZipArchive::new(cursor).context("Failed to read embedded assets archive")?;

    let mut files = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if !file.is_dir() {
            files.push(file.name().to_string());
        }
    }
    Ok(files)
}

/// Get the contents of a single file from the embedded assets.
pub fn get_asset(path: &str) -> Result<Vec<u8>> {
    let cursor = Cursor::new(ASSETS_ZIP);
    let mut archive =
        ZipArchive::new(cursor).context("Failed to read embedded assets archive")?;

    let mut file = archive
        .by_name(path)
        .with_context(|| format!("Asset not found: {}", path))?;

    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    Ok(contents)
}

/// Get the contents of a single file from the embedded assets as a string.
pub fn get_asset_str(path: &str) -> Result<String> {
    let bytes = get_asset(path)?;
    String::from_utf8(bytes).context("Asset is not valid UTF-8")
}

/// Get the wixer-knowledge-base.json content.
pub fn get_knowledge_base() -> Result<String> {
    get_asset_str("wixer-knowledge-base.json")
}

/// Get an extension JSON file by name (e.g., "util", "ui").
pub fn get_extension(name: &str) -> Result<String> {
    get_asset_str(&format!("wix-v4-extensions-json/{}.json", name))
}

/// Get the versions.json manifest with asset version information.
pub fn get_versions() -> Result<String> {
    get_asset_str("generated/versions.json")
}

/// Get the pre-generated describe.json database.
pub fn get_describe_db() -> Result<String> {
    get_asset_str("generated/describe.json")
}

/// Get the pre-generated known_attributes.json.
pub fn get_known_attributes() -> Result<String> {
    get_asset_str("generated/known_attributes.json")
}

/// Get a pre-generated JSON schema by name (e.g., "wix", "util").
pub fn get_json_schema(name: &str) -> Result<String> {
    get_asset_str(&format!("generated/json-schema/{}.schema.json", name))
}

/// List all available pre-generated JSON schemas.
pub fn list_json_schemas() -> Result<Vec<String>> {
    let files = list_embedded_assets()?;
    Ok(files
        .into_iter()
        .filter(|f| f.starts_with("generated/json-schema/") && f.ends_with(".schema.json"))
        .map(|f| {
            f.strip_prefix("generated/json-schema/")
                .unwrap_or(&f)
                .strip_suffix(".schema.json")
                .unwrap_or(&f)
                .to_string()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_assets_available() {
        assert!(has_embedded_assets(), "Embedded assets.zip should not be empty");

        let files = list_embedded_assets().expect("Should list embedded assets");
        println!("Embedded schema files: {}", files.len());

        let wix_count = files.iter().filter(|f| f.starts_with("wix-v4-xsd-schemas/")).count();
        let msix_count = files.iter().filter(|f| f.starts_with("ms-msix-xsd-schemas/")).count();

        println!("  WiX schemas: {}", wix_count);
        println!("  MSIX schemas: {}", msix_count);

        assert!(wix_count > 0, "Should have WiX schemas embedded");
        assert!(msix_count > 0, "Should have MSIX schemas embedded");

        // Check extensions and knowledge_base
        let ext_count = files.iter().filter(|f| f.starts_with("wix-v4-extensions-json/")).count();
        let has_kb = files.iter().any(|f| f == "wixer-knowledge-base.json");

        println!("  Extensions: {}", ext_count);
        println!("  Knowledge base: {}", has_kb);

        assert!(ext_count > 0, "Should have extension JSON files embedded");
        assert!(has_kb, "Should have knowledge_base.json embedded");
    }

    #[test]
    fn test_extract_wix_schemas() {
        let temp_dir = std::env::temp_dir().join("wixer_test_extract_wix");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let files = extract_wix_schemas(&temp_dir).expect("Should extract WiX schemas");
        assert!(!files.is_empty(), "Should extract at least one WiX schema");

        // Verify wix.xsd exists
        assert!(
            files.iter().any(|f| f.file_name().map(|n| n == "wix.xsd").unwrap_or(false)),
            "Should have wix.xsd"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_get_knowledge_base() {
        let kb = get_knowledge_base().expect("Should get knowledge_base.json");
        assert!(kb.contains("ACTION"), "Knowledge base should contain MSI properties");
    }

    #[test]
    fn test_get_extension() {
        let util = get_extension("util").expect("Should get util.json");
        assert!(util.contains("WixToolset.Util"), "Util extension should have correct ID");
    }

    #[test]
    fn test_get_versions() {
        let versions = get_versions().expect("Should get versions.json");
        assert!(versions.contains("build_time"), "Versions should have build_time");
        assert!(versions.contains("git_commit"), "Versions should have git_commit");
        assert!(versions.contains("wix-v4-xsd-schemas"), "Versions should have WiX schemas info");
        println!("Versions manifest:\n{}", versions);
    }

    #[test]
    fn test_get_describe_db() {
        let db = get_describe_db().expect("Should get describe.json");
        assert!(db.contains("ACTION"), "Describe DB should contain MSI properties");
        assert!(db.contains("CustomAction"), "Describe DB should contain custom actions");
    }

    #[test]
    fn test_get_known_attributes() {
        let attrs = get_known_attributes().expect("Should get known_attributes.json");
        assert!(attrs.contains("Id"), "Should contain Id attribute");
        assert!(attrs.contains("Name"), "Should contain Name attribute");
    }

    #[test]
    fn test_get_json_schema() {
        let wix_schema = get_json_schema("wix").expect("Should get wix.schema.json");
        assert!(wix_schema.contains("definitions"), "Schema should have definitions");
        assert!(wix_schema.contains("Package"), "Schema should have Package element");
    }

    #[test]
    fn test_list_json_schemas() {
        let schemas = list_json_schemas().expect("Should list JSON schemas");
        println!("Available schemas: {:?}", schemas);
        assert!(schemas.contains(&"wix".to_string()), "Should have wix schema");
        assert!(schemas.contains(&"util".to_string()), "Should have util schema");
    }
}
