use serde::Deserialize;
use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use crate::utils::find_wixer_root;
use tracing::info;

#[derive(Debug, Deserialize, Default)]
pub struct WixerConfig {
    #[serde(default)]
    pub build: BuildConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct BuildConfig {
    #[serde(default)]
    pub inspect_pdb: bool,
    #[serde(default)]
    pub inspect_symbols: Vec<String>,
    pub real_wix_path: Option<String>,
}

impl WixerConfig {
    pub fn load() -> Self {
        // Try current dir first
        if let Ok(cfg) = Self::load_from_path(Path::new("wixer.yaml")) {
            info!("Loaded configuration from wixer.yaml");
            return cfg;
        }
        
        // Try wixer root
        if let Ok(root) = find_wixer_root() {
             // find_wixer_root returns the .wixer folder. The config should be in the project root (parent)
             if let Some(project_root) = root.parent() {
                 let path = project_root.join("wixer.yaml");
                 if let Ok(cfg) = Self::load_from_path(&path) {
                     info!("Loaded configuration from {}", path.display());
                     return cfg;
                 }
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
