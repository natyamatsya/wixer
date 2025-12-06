use crate::common::AttributeStrategy;
use crate::utils::find_wixer_root;
use crate::xsd_to_json;
use anyhow::Result;
use std::collections::{BTreeSet, HashSet};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

pub fn run(fresh: bool) -> Result<()> {
    let wixer_root = find_wixer_root().unwrap_or_else(|e| {
        eprintln!("Error determining project root: {}", e);
        std::process::exit(1);
    });

    let cache_base = wixer_root.join("cache");
    let xml_schema_dir = cache_base.join("xml-schema");
    let json_schema_dir = cache_base.join("json-schema");
    println!("Initializing wixer in {}", wixer_root.display());

    if fresh {
        if cache_base.exists() {
            println!("Clearing cache...");
            if let Err(e) = std::fs::remove_dir_all(&cache_base) {
                eprintln!("Error clearing cache: {}", e);
                std::process::exit(1);
            }
        }
    }

    match xsd_to_json::download_wix_schemas(&xml_schema_dir, fresh) {
        Ok(files) => {
            let mut all_attributes = HashSet::new();
            println!("Generating JSON schemas...");

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
                    Err(e) => eprintln!("Error converting {}: {}", file.display(), e),
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
                println!("✓ Generated attribute list: {}", attr_file.display());
            }

            println!("✓ Initialization complete");
        }
        Err(e) => {
            eprintln!("Error initializing: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
