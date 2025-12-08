use crate::common::AttributeStrategy;
use crate::utils::load_known_attributes;
use crate::wix_to_yaml;
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
        "Importing WiX files recursively"
    );

    fs::create_dir_all(output_dir)?;

    let known_attributes = load_known_attributes(None, attribute_strategy == AttributeStrategy::Inline);

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
            // Supported WiX extensions
            let supported_exts = ["wxs", "wxi", "wxl", "wsi"];

            let mut matched_ext = None;
            let mut is_template = false;

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
                // Calculate relative path to preserve structure
                let relative_path = path.strip_prefix(input_dir).unwrap_or(path);
                let output_path = output_dir.join(relative_path);

                // Use new .wxs.yaml format instead of legacy .ywxs
                let target_ext = format!("{}.yaml", ext);

                let final_output_path = if is_template {
                    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
                    // Remove .{ext}.in and add .{ext}.yaml.in
                    let suffix = format!(".{}.in", ext);
                    let stem = file_name.trim_end_matches(&suffix);
                    parent.join(format!("{}.{}.in", stem, target_ext))
                } else {
                    // Remove .{ext} and add .{ext}.yaml
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

                info!(input = %path.display(), output = %final_output_path.display(), "Importing");

                // Check for preprocessor directives that can't be converted to YAML
                let content = fs::read_to_string(path).unwrap_or_default();
                
                // <?if?> and <?define?> can't be converted - copy as-is
                if content.contains("<?if") || content.contains("<?define") {
                    info!(file = %path.display(), "Skipping YAML conversion (conditional preprocessor directives)");
                    fs::copy(path, &final_output_path).with_context(|| {
                        format!("Failed to copy {}", path.display())
                    })?;
                    count += 1;
                } else if let Err(e) = wix_to_yaml::convert_file(
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

    info!(imported = count, errors = errors, "Import complete");

    if errors > 0 {
        anyhow::bail!("Completed with {} errors", errors);
    }

    Ok(())
}
