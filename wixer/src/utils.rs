use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use tracing::warn;

use crate::dirs;

/// Finds the project root by looking for markers like .git, wixer.yaml, or .wixer
pub fn find_project_root() -> std::io::Result<PathBuf> {
    let current_dir = env::current_dir()?;
    let mut dir = current_dir.as_path();

    loop {
        // Check for indicators
        if dir.join(".git").exists() || dir.join("wixer.yaml").exists() || dir.join(".wixer").exists() {
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

/// Returns candidate locations for the known_attributes.json file in priority order.
pub fn known_attributes_candidates(explicit: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = explicit {
        candidates.push(path.to_path_buf());
    }

    candidates.push(PathBuf::from("json-schema/known_attributes.json"));
    candidates.push(PathBuf::from("json-schemas/known_attributes.json"));

    if let Ok(cache_dir) = dirs::get_cache_dir() {
        candidates.push(
            cache_dir
                .join("json-schema")
                .join("known_attributes.json"),
        );
    }

    if let Ok(root) = find_project_root() {
        candidates.push(
            root
                .join(".wixer")
                .join("cache")
                .join("json-schema")
                .join("known_attributes.json"),
        );
    }

    candidates
}

pub fn load_known_attributes(explicit: Option<&Path>, warn_on_missing_inline: bool) -> HashSet<String> {
    for candidate in known_attributes_candidates(explicit) {
        if !candidate.exists() {
            continue;
        }

        match File::open(&candidate) {
            Ok(file) => {
                let reader = BufReader::new(file);
                match serde_json::from_reader(reader) {
                    Ok(attrs) => return attrs,
                    Err(e) => warn!(file = %candidate.display(), "Failed to parse attributes file: {}", e),
                }
            }
            Err(e) => warn!(file = %candidate.display(), "Failed to open attributes file: {}", e),
        }
    }

    if warn_on_missing_inline {
        warn!(
            "No known_attributes.json found and strategy is Inline. Attribute detection may fail."
        );
    }

    HashSet::new()
}
