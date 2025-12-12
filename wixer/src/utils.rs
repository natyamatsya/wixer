use std::collections::HashSet;
use std::env;
use std::path::PathBuf;

use tracing::warn;

use crate::embedded_assets;

/// Finds the project root by looking for markers like .git, wixer.yaml, or .wixer
pub fn find_project_root() -> std::io::Result<PathBuf> {
    let current_dir = env::current_dir()?;
    let mut dir = current_dir.as_path();

    loop {
        // Check for indicators
        if dir.join(".git").exists()
            || dir.join("wixer.yaml").exists()
            || dir.join(".wixer").exists()
        {
            return Ok(dir.to_path_buf());
        }

        if let Some(parent) = dir.parent() {
            dir = parent;
        } else {
            break;
        }
    }

    Ok(current_dir)
}

/// Finds the configuration file by walking up the directory tree
pub fn find_configuration_file() -> Option<PathBuf> {
    let current_dir = env::current_dir().ok()?;
    let mut dir = current_dir.as_path();

    loop {
        let config_path = dir.join("wixer.yaml");
        if config_path.exists() {
            return Some(config_path);
        }

        if let Some(parent) = dir.parent() {
            dir = parent;
        } else {
            break;
        }
    }

    None
}

/// Load known attributes from embedded assets.
pub fn load_known_attributes() -> HashSet<String> {
    match embedded_assets::get_known_attributes() {
        Ok(json) => match serde_json::from_str(&json) {
            Ok(attrs) => return attrs,
            Err(e) => {
                warn!("Failed to parse embedded known_attributes.json: {}", e);
            }
        },
        Err(e) => {
            warn!("Failed to load embedded known_attributes.json: {}", e);
        }
    }

    // Fallback to defaults if embedded assets fail
    let defaults = [
        "Id",
        "Name",
        "Version",
        "Manufacturer",
        "UpgradeCode",
        "Language",
        "Codepage",
        "Title",
        "Description",
        "Level",
        "Source",
        "KeyPath",
        "Compressed",
        "Scope",
        "InstallerVersion",
        "EmbedCab",
        "DowngradeErrorMessage",
        "Guid",
        "Directory",
        "Target",
        "DiskId",
        "Vital",
        "Hidden",
        "System",
        "ReadOnly",
        "Checksum",
        "PatchGroup",
        "Part",
        "File",
        "Component",
        "Feature",
        "Property",
        "Condition",
        "Value",
        "Type",
        "Action",
        "Sequence",
        "Before",
        "After",
        "Cost",
        "Key",
    ];

    defaults.iter().map(|s| s.to_string()).collect()
}
