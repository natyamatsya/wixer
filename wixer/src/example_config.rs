//! Example configuration schema
//!
//! Defines the `wixer.example.yaml` format used to configure example builds
//! across different build systems (gallery, cargo-make, CMake).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Configuration for a WiX example project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleConfig {
    /// Display name of the example
    pub name: String,

    /// Description of what this example demonstrates
    #[serde(default)]
    pub description: String,

    /// Target architecture (x86, x64, arm64)
    #[serde(default)]
    pub arch: Option<String>,

    /// Preprocessor variable definitions (-d flags)
    #[serde(default)]
    pub defines: HashMap<String, String>,

    /// Source files to build (auto-detected if not specified)
    #[serde(default)]
    pub sources: Vec<String>,

    /// WiX extensions required
    #[serde(default)]
    pub extensions: Vec<String>,

    /// External dependencies that must exist before build
    #[serde(default)]
    pub requires: Vec<Requirement>,

    /// Optional prebuild steps to generate required artifacts (e.g., custom action DLLs)
    #[serde(default)]
    pub prebuild: Vec<PrebuildStep>,

    /// Whether this example can be built standalone (without external deps)
    #[serde(default = "default_buildable")]
    pub buildable: bool,

    /// If true, gallery builds are expected to fail and should be treated as success.
    #[serde(default)]
    pub expect_build_failure: bool,

    /// Reason why the example is not buildable (if buildable=false)
    #[serde(default)]
    pub skip_reason: Option<String>,
}

/// A prebuild step to run before building the MSI (only if required artifacts are missing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrebuildStep {
    /// The tool/executable to run (e.g., dotnet, cmake, cargo)
    pub tool: String,

    /// Arguments for the tool
    #[serde(default)]
    pub args: Vec<String>,

    /// Working directory for the command (relative to example dir)
    #[serde(default)]
    pub cwd: Option<String>,

    /// Files that should exist after the command runs (relative to example dir)
    #[serde(default)]
    pub produces: Vec<String>,
}

fn default_buildable() -> bool {
    true
}

/// External requirement for building
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    /// Path to required file or directory (relative to example dir)
    pub path: String,

    /// Description of what this requirement is
    #[serde(default)]
    pub description: String,
}

impl ExampleConfig {
    /// Load configuration from a wixer.example.yaml file
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: ExampleConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Try to load configuration from an example directory
    /// Returns None if no config file exists
    pub fn try_load(example_dir: &Path) -> Option<Self> {
        let config_path = example_dir.join("wixer.example.yaml");
        if config_path.exists() {
            Self::load(&config_path).ok()
        } else {
            None
        }
    }

    /// Generate wixer build arguments from this config
    pub fn build_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Add -arch flag if specified
        if let Some(arch) = &self.arch {
            args.push("-arch".to_string());
            args.push(arch.clone());
        }

        // Add -d flags for defines
        for (key, value) in &self.defines {
            args.push("-d".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Add -ext flags for extensions
        for ext in &self.extensions {
            args.push("-ext".to_string());
            args.push(ext.clone());
        }

        args
    }

    /// Check if all requirements are satisfied
    pub fn check_requirements(&self, example_dir: &Path) -> Vec<String> {
        let mut missing = Vec::new();
        for req in &self.requires {
            let req_path = example_dir.join(&req.path);
            if !req_path.exists() {
                missing.push(format!("{}: {}", req.path, req.description));
            }
        }
        missing
    }
}

impl Default for ExampleConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            arch: None,
            defines: HashMap::new(),
            sources: Vec::new(),
            extensions: Vec::new(),
            requires: Vec::new(),
            prebuild: Vec::new(),
            buildable: true,
            expect_build_failure: false,
            skip_reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let yaml = r#"
name: "02_x86_x64_Installer"
description: "Platform-dependent installer"
defines:
  Platform: x64
extensions:
  - WixToolset.Util.wixext
"#;
        let config: ExampleConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "02_x86_x64_Installer");
        assert_eq!(config.defines.get("Platform"), Some(&"x64".to_string()));
        assert!(config.buildable);
    }

    #[test]
    fn test_build_args() {
        let mut config = ExampleConfig::default();
        config
            .defines
            .insert("Platform".to_string(), "x64".to_string());
        config.extensions.push("WixToolset.Util.wixext".to_string());

        let args = config.build_args();
        assert!(args.contains(&"-d".to_string()));
        assert!(args.contains(&"Platform=x64".to_string()));
        assert!(args.contains(&"-ext".to_string()));
        assert!(args.contains(&"WixToolset.Util.wixext".to_string()));
    }
}
