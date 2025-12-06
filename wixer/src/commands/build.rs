use crate::common::AttributeStrategy;
use crate::utils::find_wixer_root;
use crate::yaml_to_wix;
use anyhow::Result;
use serde_yaml::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use tracing::{error, info};

const WIX_NAMESPACE: &str = "http://wixtoolset.org/schemas/v4/wxs";

pub fn run(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        error!("No arguments provided for build");
        std::process::exit(1);
    }

    // Check if it's actually the 'build' command
    if args[0] != "build" {
        // Pass through other commands to wix
        let status = std::process::Command::new("wix")
            .args(args)
            .status()
            .unwrap_or_else(|e| {
                error!("Failed to execute wix: {}", e);
                std::process::exit(1);
            });
        std::process::exit(status.code().unwrap_or(1));
    }

    // It is 'build', let's intercept inputs
    let wixer_root = find_wixer_root().unwrap_or_else(|_| {
        // Fallback if not in a wixer project, use temp dir
        std::env::temp_dir().join("wixer_intermediates")
    });

    let intermediates_dir = wixer_root.join("intermediates");
    if !intermediates_dir.exists() {
        std::fs::create_dir_all(&intermediates_dir).expect("Failed to create intermediates dir");
    }

    // Load known attributes
    let mut known_attributes = HashSet::new();
    if let Ok(root) = find_wixer_root() {
        let path = root
            .join("cache")
            .join("json-schema")
            .join("known_attributes.json");
        if path.exists() {
            if let Ok(file) = File::open(&path) {
                let reader = BufReader::new(file);
                if let Ok(attrs) = serde_json::from_reader(reader) {
                    known_attributes = attrs;
                }
            }
        }
    }

    let mut new_args = Vec::new();
    new_args.push("build".to_string());

    for arg in &args[1..] {
        let path = PathBuf::from(arg);
        let is_yaml = path
            .extension()
            .map_or(false, |ext| ext == "yaml" || ext == "yml");

        if is_yaml && path.exists() {
            // Check content for Wix root and namespace
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(yaml_data) = serde_yaml::from_str::<Value>(&content) {
                    if is_wix_file(&yaml_data) {
                        // Convert
                        let filename = path.file_stem().unwrap().to_string_lossy();
                        let output_path = intermediates_dir.join(format!("{}.wxs", filename));

                        match yaml_to_wix::convert_file(
                            &path,
                            &output_path,
                            AttributeStrategy::Inline,
                            &known_attributes,
                        ) {
                            Ok(_) => {
                                new_args.push(output_path.to_string_lossy().to_string());
                                continue;
                            }
                            Err(e) => {
                                error!(file = %path.display(), "Failed to convert: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                }
            }
        }
        new_args.push(arg.clone());
    }

    info!("Running: wix {}", new_args.join(" "));
    let status = std::process::Command::new("wix")
        .args(&new_args[1..]) // Skip "build" as it is in new_args[0] but we ran "wix"
        .status()
        .unwrap_or_else(|e| {
            error!("Failed to execute wix: {}", e);
            std::process::exit(1);
        });
    std::process::exit(status.code().unwrap_or(1));
}

fn is_wix_file(data: &Value) -> bool {
    if let Some(wix) = data.get("Wix") {
        // Check for namespace in various forms
        if let Some(ns) = wix.get("xmlns") {
            if ns.as_str() == Some(WIX_NAMESPACE) {
                return true;
            }
        }
        if let Some(ns) = wix.get("@xmlns") {
            if ns.as_str() == Some(WIX_NAMESPACE) {
                return true;
            }
        }
        // Also check separate attribute strategy
        if let Some(attrs) = wix.get("attributes") {
            if let Some(ns) = attrs.get("xmlns") {
                if ns.as_str() == Some(WIX_NAMESPACE) {
                    return true;
                }
            }
        }
    }
    false
}
