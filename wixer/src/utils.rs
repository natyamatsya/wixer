use std::env;
use std::path::PathBuf;

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
