use serde::Deserialize;
use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use crate::utils::find_configuration_file;
use crate::dirs;
use tracing::info;

use std::collections::HashMap;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct WixerConfig {
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileBuildConfig>,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct BuildConfig {
    #[serde(default)]
    pub inspect_pdb: bool,
    #[serde(default)]
    pub inspect_symbols: Vec<String>,
    #[serde(default)]
    pub verbose: bool,
    pub real_wix_path: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct LoggingConfig {
    pub file: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ProfileBuildConfig {
    pub inspect_pdb: Option<bool>,
    pub inspect_symbols: Option<Vec<String>>,
    pub verbose: Option<bool>,
    pub real_wix_path: Option<String>,
}

impl ProfileBuildConfig {
    fn apply(&self, target: &mut BuildConfig) {
        if let Some(value) = self.inspect_pdb {
            target.inspect_pdb = value;
        }

        if let Some(values) = &self.inspect_symbols {
            target.inspect_symbols = values.clone();
        }

        if let Some(value) = self.verbose {
            target.verbose = value;
        }

        if let Some(path) = &self.real_wix_path {
            target.real_wix_path = Some(path.clone());
        }
    }
}

impl WixerConfig {
    pub fn get_build_config(&self, profile: Option<&str>) -> BuildConfig {
        let mut merged = self.build.clone();

        if let Some(p) = profile
            && let Some(profile_config) = self.profiles.get(p)
        {
            profile_config.apply(&mut merged);
        }

        merged
    }

    /// Load config from explicit path, or search for it.
    pub fn load_from(explicit_path: Option<&Path>) -> Self {
        // If explicit path provided, use it
        if let Some(path) = explicit_path {
            if let Ok(cfg) = Self::load_from_path(path) {
                info!("Loaded configuration from {}", path.display());
                return cfg;
            }
        }

        Self::load()
    }

    pub fn load() -> Self {
        // Check WIXER_CONFIG environment variable first
        if let Ok(config_path) = std::env::var("WIXER_CONFIG") {
            let path = Path::new(&config_path);
            if let Ok(cfg) = Self::load_from_path(path) {
                info!("Loaded configuration from {} (WIXER_CONFIG)", path.display());
                return cfg;
            }
        }

        // Try to find configuration file by walking up
        if let Some(path) = find_configuration_file() 
            && let Ok(cfg) = Self::load_from_path(&path) {
                info!("Loaded configuration from {}", path.display());
                return cfg;
        }

        // Try global config
        if let Ok(config_dir) = dirs::get_config_dir() {
            let path = config_dir.join("wixer.yaml");
            if let Ok(cfg) = Self::load_from_path(&path) {
                info!("Loaded configuration from {}", path.display());
                return cfg;
            }
        }

        Self::default()
    }

    pub fn load_from_path(path: &Path) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let config: WixerConfig = serde_yaml::from_reader(reader)?;
        Ok(config)
    }
}
