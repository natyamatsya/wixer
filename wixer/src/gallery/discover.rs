use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::example_config::ExampleConfig;

use super::options::SourceType;

pub(super) struct Example {
    pub(super) name: String,
    pub(super) path: PathBuf,
    pub(super) wxs_files: Vec<PathBuf>,
}

/// Example ready for building
pub(super) struct BuildExample {
    pub(super) name: String,
    pub(super) source_dir: PathBuf,
    pub(super) config: Option<ExampleConfig>,
    /// Files to build (.wxs.yaml or .wxs)
    pub(super) build_files: Vec<PathBuf>,
}

/// Find examples from submodule (v3 WiX files)
pub(super) fn find_submodule_examples(
    source: &Path,
    filter: Option<&Vec<String>>,
) -> Result<Vec<Example>> {
    let mut examples = Vec::new();

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Skip non-example directories
        if name.starts_with('.') || name == "images" || name == ".github" {
            continue;
        }

        // Apply filter if specified
        if let Some(prefixes) = filter
            && !prefixes.iter().any(|p| name.starts_with(p))
        {
            continue;
        }

        // Find all .wxs and .wxi files in this example
        let wxs_files: Vec<PathBuf> = WalkDir::new(&path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "wxs" || ext == "wxi")
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        if !wxs_files.is_empty() {
            examples.push(Example {
                name,
                path,
                wxs_files,
            });
        }
    }

    // Sort by name
    examples.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(examples)
}

/// Discover examples to build based on source type
pub(super) fn discover_build_examples(
    source_type: &SourceType,
    yaml_gallery: &Path,
    v4_dir: &Path,
    configs_dir: &Path,
    filter: Option<&Vec<String>>,
) -> Result<Vec<BuildExample>> {
    let mut examples = Vec::new();

    // Discover from YAML gallery
    if (*source_type == SourceType::Yaml || *source_type == SourceType::All)
        && yaml_gallery.exists()
    {
        for entry in fs::read_dir(yaml_gallery)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if name.starts_with('.') {
                continue;
            }

            // Apply filter
            if let Some(prefixes) = filter
                && !prefixes.iter().any(|p| name.starts_with(p))
            {
                continue;
            }

            // Load config
            let config_path = configs_dir.join(format!("{}.yaml", name));
            let config = ExampleConfig::load(&config_path).ok();

            // Find main YAML source files (.wxs.yaml only - .wxi.yaml are includes)
            let yaml_main_files: Vec<PathBuf> = WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let fname = e.file_name().to_str().unwrap_or("");
                    fname.ends_with(".wxs.yaml")
                })
                .map(|e| e.path().to_path_buf())
                .collect();

            // Find YAML template files (.yaml.in) - these are templates for <?include?>
            let _yaml_template_files: Vec<PathBuf> = WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let fname = e.file_name().to_str().unwrap_or("");
                    fname.ends_with(".yaml.in")
                })
                .map(|e| e.path().to_path_buf())
                .collect();

            // Find XML files (for preprocessor examples)
            let xml_files: Vec<PathBuf> = WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let fname = e.file_name().to_str().unwrap_or("");
                    fname.ends_with(".wxs") || fname.ends_with(".wxi")
                })
                .map(|e| e.path().to_path_buf())
                .collect();

            // Determine build files:
            // Include all source files (.wxs.yaml, .wxs)
            // .yaml.in templates are handled by wixer build automatically
            let mut build_files = Vec::new();
            build_files.extend(yaml_main_files);
            build_files.extend(xml_files);
            // Don't include .yaml.in templates - wixer build exports them automatically

            if build_files.is_empty() {
                continue;
            }

            examples.push(BuildExample {
                name,
                source_dir: path,
                config,
                build_files,
            });
        }
    }

    // Discover from v4 (upgraded submodule) - only if not already found in YAML
    if (*source_type == SourceType::Submodule || *source_type == SourceType::All) && v4_dir.exists()
    {
        // Collect names first to avoid borrow issues
        let yaml_names: std::collections::HashSet<String> =
            examples.iter().map(|e| e.name.clone()).collect();

        for entry in fs::read_dir(v4_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if name.starts_with('.') {
                continue;
            }

            // Skip if already found in YAML gallery
            if yaml_names.contains(&name) {
                continue;
            }

            // Apply filter
            if let Some(prefixes) = filter
                && !prefixes.iter().any(|p| name.starts_with(p))
            {
                continue;
            }

            // Load config
            let config_path = configs_dir.join(format!("{}.yaml", name));
            let config = ExampleConfig::load(&config_path).ok();

            // Find WXS files
            let wxs_files: Vec<PathBuf> = WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|s| s.ends_with(".wxs"))
                        .unwrap_or(false)
                })
                .map(|e| e.path().to_path_buf())
                .collect();

            if wxs_files.is_empty() {
                continue;
            }

            examples.push(BuildExample {
                name,
                source_dir: path,
                config,
                build_files: wxs_files,
            });
        }
    }

    // Sort by name
    examples.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(examples)
}
