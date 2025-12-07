use crate::common::AttributeStrategy;
use crate::describe_db;
use crate::dirs;
use crate::xsd_to_json;
use anyhow::Result;
use std::collections::{BTreeSet, HashSet};
use std::fs::File;
use std::io::BufWriter;
use tracing::{error, info};

pub async fn run(fresh: bool) -> Result<()> {
    let cache_base = dirs::get_cache_dir().unwrap_or_else(|e| {
        error!("Error determining cache directory: {}", e);
        std::process::exit(1);
    });

    if !cache_base.exists() {
        std::fs::create_dir_all(&cache_base)?;
    }

    let xml_schema_dir = cache_base.join("xml-schema");
    let msix_schema_dir = cache_base.join("msix-xsd-schema");
    let json_schema_dir = cache_base.join("json-schema");
    info!(cache = %cache_base.display(), "Initializing wixer schemas");

    if fresh && cache_base.exists() {
        info!("Clearing cache...");
        if let Err(e) = std::fs::remove_dir_all(&cache_base) {
            error!("Error clearing cache: {}", e);
            std::process::exit(1);
        }
    }

    match xsd_to_json::download_wix_schemas(&xml_schema_dir, fresh).await {
        Ok(files) => {
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
                let attr_file = json_schema_dir.join("known_attributes.json");
                let file =
                    File::create(&attr_file).expect("Failed to create known_attributes.json");
                let writer = BufWriter::new(file);
                serde_json::to_writer_pretty(writer, &sorted_attributes)
                    .expect("Failed to write attributes");
                info!("✓ Generated attribute list: {}", attr_file.display());
            }

            info!("✓ Initialization complete");
        }
        Err(e) => {
            error!("Error initializing: {}", e);
            std::process::exit(1);
        }
    }

    // Download MSIX schemas
    if let Err(e) = xsd_to_json::download_msix_schemas(&msix_schema_dir, fresh).await {
        error!("Error downloading MSIX schemas: {}", e);
        std::process::exit(1);
    }

    // Generate description database
    if let Err(e) = describe_db::generate() {
        error!("Failed to generate description database: {}", e);
    } else {
        info!("Generated description database");
    }

    Ok(())
}
