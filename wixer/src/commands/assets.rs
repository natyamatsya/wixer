use std::path::PathBuf;

use anyhow::Result;
use tracing::info;

use crate::embedded_assets;

pub fn run(info_flag: bool, extract: Option<PathBuf>) -> Result<()> {
    let info_flag = info_flag || extract.is_none();

    if info_flag {
        if let Ok(versions) = embedded_assets::get_versions() {
            info!("Embedded asset versions:\n{}", versions);
        }

        let files = embedded_assets::list_embedded_assets()?;
        let wix_xsd = files.iter().filter(|f| f.starts_with("wix-v4-xsd-schemas/")).count();
        let msix_xsd = files
            .iter()
            .filter(|f| f.starts_with("ms-msix-xsd-schemas/") && f.ends_with(".xsd"))
            .count();
        let json_schemas = files
            .iter()
            .filter(|f| f.starts_with("generated/json-schema/") && f.ends_with(".schema.json"))
            .count();
        let extensions = files
            .iter()
            .filter(|f| f.starts_with("wix-v4-extensions-json/") && f.ends_with(".json"))
            .count();

        info!(
            total = files.len(),
            wix_xsd,
            msix_xsd,
            json_schemas,
            extensions,
            "Embedded assets summary"
        );

        if let Ok(schemas) = embedded_assets::list_json_schemas() {
            info!("Available JSON schemas: {:?}", schemas);
        }
    }

    if let Some(output_dir) = extract {
        let extracted = embedded_assets::extract_all(&output_dir)?;
        info!(count = extracted.len(), dir = %output_dir.display(), "Extracted embedded assets");
    }

    Ok(())
}
