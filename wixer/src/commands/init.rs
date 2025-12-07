use crate::common::AttributeStrategy;
use crate::describe_db;
use crate::dirs;
use crate::xsd_to_json;
use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashSet};
use std::fs::File;
use std::io::BufWriter;
use tracing::{error, info};

pub async fn run(fresh: bool) -> Result<()> {
    let cache_base = dirs::get_cache_dir()
        .context("Error determining cache directory")?;

    if !cache_base.exists() {
        std::fs::create_dir_all(&cache_base)?;
    }

    let xml_schema_dir = cache_base.join("xml-schema");
    let msix_schema_dir = cache_base.join("msix-xsd-schema");
    let json_schema_dir = cache_base.join("json-schema");
    info!(cache = %cache_base.display(), "Initializing wixer schemas");

    if fresh && cache_base.exists() {
        info!("Clearing cache...");
        std::fs::remove_dir_all(&cache_base)
            .with_context(|| "Error clearing cache")?;
        std::fs::create_dir_all(&cache_base)?;
    }

    let files = xsd_to_json::download_wix_schemas(&xml_schema_dir, fresh)
        .await
        .context("Error downloading WiX schemas")?;

    let mut all_attributes = HashSet::new();
    info!("Generating JSON schemas...");

    for file in files {
        match xsd_to_json::convert_xsd(
            file.to_str().unwrap(),
            None,
            &json_schema_dir,
            AttributeStrategy::Inline,
            true,  // include_descriptions
            false, // strict_mode
            None,  // root_element
        ) {
            Ok(attrs) => all_attributes.extend(attrs),
            Err(e) => {
                error!(file = %file.display(), "Error converting {}: {}", file.display(), e)
            }
        }
    }

    // Save known attributes
    if !all_attributes.is_empty() {
        let sorted_attributes: BTreeSet<_> = all_attributes.into_iter().collect();
        let attr_file = cache_base.join("known_attributes.json");
        let file = File::create(&attr_file)
            .with_context(|| format!("Failed to create {}", attr_file.display()))?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &sorted_attributes)
            .context("Failed to write attributes")?;
        info!("✓ Generated attribute list: {}", attr_file.display());
    }

    info!("✓ Initialization complete");

    // Download MSIX schemas
    xsd_to_json::download_msix_schemas(&msix_schema_dir, fresh)
        .await
        .context("Error downloading MSIX schemas")?;

    // Generate description database
    match describe_db::generate() {
        Ok(_) => info!("Generated description database"),
        Err(e) => {
            error!("Failed to generate description database: {}", e);
        }
    }

    Ok(())
}
