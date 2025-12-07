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
    pub profiles: HashMap<String, BuildConfig>,
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

impl WixerConfig {
    pub fn get_build_config(&self, profile: Option<&str>) -> BuildConfig {
        if let Some(p) = profile
            && let Some(profile_config) = self.profiles.get(p) {
                // Merge profile config with default build config
                // Profile takes precedence for defined values (conceptually)
                // But for simplicity, we'll return the profile config with fallbacks from default if fields were Option
                // Since our fields are mostly flags, we'll just return the profile config
                // but we should probably copy over the real_wix_path if missing
                let mut merged = profile_config.clone();
                if merged.real_wix_path.is_none() {
                    merged.real_wix_path = self.build.real_wix_path.clone();
                }
                return merged;
            }
        self.build.clone()
    }

    pub fn load() -> Self {
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

    fn load_from_path(path: &Path) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let config: WixerConfig = serde_yaml::from_reader(reader)?;
        Ok(config)
    }
}
