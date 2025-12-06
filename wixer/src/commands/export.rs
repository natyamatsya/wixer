use crate::common::AttributeStrategy;
use crate::utils::find_wixer_root;
use crate::yaml_to_wix;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use tracing::{error, info, warn};
use walkdir::WalkDir;

pub fn run(
    input_dir: &Path,
    output_dir: &Path,
    attribute_strategy: AttributeStrategy,
) -> Result<()> {
    info!(
        input_dir = %input_dir.display(),
        output_dir = %output_dir.display(),
        "Exporting YAML files to WiX recursively"
    );

    fs::create_dir_all(output_dir)?;

    // Load known attributes
    let mut known_attributes = HashSet::new();
    if let Ok(root) = find_wixer_root() {
        let path = root
            .join("cache")
            .join("json-schema")
            .join("known_attributes.json");
        if path.exists()
            && let Ok(file) = File::open(&path)
        {
            let reader = BufReader::new(file);
            if let Ok(attrs) = serde_json::from_reader(reader) {
                known_attributes = attrs;
            }
        }
    } else {
        warn!("Could not find project root, attribute detection might be limited");
    }

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
        if path.is_file()
            && let Some(file_name) = path.file_name().and_then(|n| n.to_str())
        {
            // Check for known YAML extensions (prepend y)
            // ywxs, ywxi, ywxl, ywsi
            // Also check for .in templates
            let mut matched_ext = None;
            let mut is_template = false;

            let supported_exts = ["ywxs", "ywxi", "ywxl", "ywsi"];

            for ext in supported_exts {
                if file_name.ends_with(&format!(".{}", ext)) {
                    matched_ext = Some(ext);
                    is_template = false;
                    break;
                } else if file_name.ends_with(&format!(".{}.in", ext)) {
                    matched_ext = Some(ext);
                    is_template = true;
                    break;
                }
            }

            if let Some(ext) = matched_ext {
                // Calculate relative path
                let relative_path = path.strip_prefix(input_dir).unwrap_or(path);
                let output_path = output_dir.join(relative_path);

                // Determine target extension (remove leading 'y')
                // ywxs -> wxs
                let target_ext = &ext[1..];

                let final_output_path = if is_template {
                    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
                    // Remove .{ext}.in and add .{target_ext}.in
                    let suffix = format!(".{}.in", ext);
                    let stem = file_name.trim_end_matches(&suffix);
                    parent.join(format!("{}.{}.in", stem, target_ext))
                } else {
                    // Remove .{ext} and add .{target_ext}
                    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
                    let suffix = format!(".{}", ext);
                    let stem = file_name.trim_end_matches(&suffix);
                    parent.join(format!("{}.{}", stem, target_ext))
                };

                // Create parent directories
                if let Some(parent) = final_output_path.parent() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("Failed to create directory: {}", parent.display())
                    })?;
                }

                info!(input = %path.display(), output = %final_output_path.display(), "Exporting");

                if let Err(e) = yaml_to_wix::convert_file(
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

    info!(exported = count, errors = errors, "Export complete");

    if errors > 0 {
        anyhow::bail!("Completed with {} errors", errors);
    }

    Ok(())
}
