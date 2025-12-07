use crate::common::AttributeStrategy;
use crate::constants::{EXT_WSI_YAML, EXT_WXI_YAML, EXT_WXL_YAML, EXT_WXS_YAML};
use crate::utils::load_known_attributes;
use crate::yaml_to_wix;
use anyhow::{Context, Result};
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
        "Exporting YAML files to WiX recursively"
    );

    fs::create_dir_all(output_dir)?;

    // Load known attributes
    let known_attributes =
        load_known_attributes(None, attribute_strategy == AttributeStrategy::Inline);

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

            let supported_exts = [
                (EXT_WXS_YAML, "wxs"),
                (EXT_WXI_YAML, "wxi"),
                (EXT_WXL_YAML, "wxl"),
                (EXT_WSI_YAML, "wsi"),
            ];

            for (ext, target) in supported_exts {
                if file_name.ends_with(ext) {
                    matched_ext = Some((ext, target));
                    is_template = false;
                    break;
                } else if file_name.ends_with(&format!("{}.in", ext)) {
                    matched_ext = Some((ext, target));
                    is_template = true;
                    break;
                }
            }

            if let Some((ext, target_ext)) = matched_ext {
                // Calculate relative path
                let relative_path = path.strip_prefix(input_dir).unwrap_or(path);
                let output_path = output_dir.join(relative_path);

                let final_output_path = if is_template {
                    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
                    // Remove .{ext}.in and add .{target_ext}.in
                    let suffix = format!("{}.in", ext);
                    let stem = file_name.trim_end_matches(&suffix);
                    parent.join(format!("{}.{}.in", stem, target_ext))
                } else {
                    // Remove .{ext} and add .{target_ext}
                    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
                    let stem = file_name.trim_end_matches(ext).trim_end_matches('.');
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
