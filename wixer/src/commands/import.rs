use crate::common::AttributeStrategy;
use crate::wix_to_yaml;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tracing::{error, info};
use walkdir::WalkDir;

pub fn run(
    input_dir: &Path,
    output_dir: &Path,
    attribute_strategy: AttributeStrategy,
) -> Result<()> {
    info!(
        input_dir = %input_dir.display(),
        output_dir = %output_dir.display(),
        "Importing WiX files recursively"
    );

    fs::create_dir_all(output_dir)?;

    let known_attributes = HashSet::new(); // Placeholder for now

    let mut count = 0;
    let mut errors = 0;

    for entry in WalkDir::new(input_dir) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                error!("Error accessing entry: {}", e);
                continue;
            }
        };

        let path = entry.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                // Determine if it is a WiX file or a WiX template file
                let is_wix = file_name.ends_with(".wxs");
                let is_wix_in = file_name.ends_with(".wxs.in");

                if is_wix || is_wix_in {
                    // Calculate relative path to preserve structure
                    let relative_path = path.strip_prefix(input_dir).unwrap_or(path);
                    let output_path = output_dir.join(relative_path);

                    // Determine new extension
                    let final_output_path = if is_wix_in {
                        // replace .wxs.in with .yaml.in
                        // But careful: path.with_extension replaces the *last* extension.
                        // "file.wxs.in" -> "file.wxs.yaml" if we just set yaml.
                        // We want "file.yaml.in".
                        // Let's construct the name manually.
                        let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
                        let stem = file_name.trim_end_matches(".wxs.in");
                        parent.join(format!("{}.yaml.in", stem))
                    } else {
                        // replace .wxs with .yaml
                        output_path.with_extension("yaml")
                    };

                    // Create parent directories
                    if let Some(parent) = final_output_path.parent() {
                        fs::create_dir_all(parent).with_context(|| {
                            format!("Failed to create directory: {}", parent.display())
                        })?;
                    }

                    info!(input = %path.display(), output = %final_output_path.display(), "Importing");

                    if let Err(e) = wix_to_yaml::convert_file(
                        path,
                        &final_output_path,
                        attribute_strategy,
                        &known_attributes,
                    ) {
                        error!(file = %path.display(), "Failed to convert: {}", e);
                        errors += 1;
                    } else {
                        count += 1;
                    }
                }
            }
        }
    }

    info!(imported = count, errors = errors, "Import complete");

    if errors > 0 {
        anyhow::bail!("Completed with {} errors", errors);
    }

    Ok(())
}
