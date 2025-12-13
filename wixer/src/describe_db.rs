use crate::embedded_assets;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WixProperty {
    pub name: String,
    pub kind: String, // "Property", "Directory", "Preprocessor", "CPack", "Element", "Attribute", "CustomAction"
    pub description: String,
    pub docs_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Load the pre-generated describe database from embedded assets.
pub fn load() -> Result<Vec<WixProperty>> {
    let json =
        embedded_assets::get_describe_db().context("Failed to load embedded describe database")?;
    let properties: Vec<WixProperty> =
        serde_json::from_str(&json).context("Failed to parse describe database")?;
    Ok(properties)
}
