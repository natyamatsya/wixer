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
        if path.exists()
            && let Ok(file) = File::open(&path)
        {
            let reader = BufReader::new(file);
            if let Ok(attrs) = serde_json::from_reader(reader) {
                known_attributes = attrs;
            }
        }
    }

    let mut new_args = Vec::new();
    new_args.push("build".to_string());

    for arg in &args[1..] {
        let path = PathBuf::from(arg);

        let ext = path.extension().and_then(|e| e.to_str());

        // Check for known extensions
        let (is_valid, target_ext) = match ext {
            Some("wys") => (true, Some("wxs")),
            Some("wyi") => (true, Some("wxi")),
            Some("wyl") => (true, Some("wxl")),
            Some("yaml") | Some("yml") => (true, None), // Needs content check
            _ => (false, None),
        };

        if is_valid && path.exists() {
            let target_extension = if let Some(t) = target_ext {
                t
            } else {
                // Fallback for .yaml/.yml: check content
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(yaml_data) = serde_yaml::from_str::<Value>(&content) {
                        if is_wix_file(&yaml_data) {
                            if yaml_data.get("Include").is_some() {
                                "wxi"
                            } else if yaml_data.get("WixLocalization").is_some() {
                                "wxl"
                            } else {
                                "wxs"
                            }
                        } else {
                            // Not a WiX file, ignore
                            new_args.push(arg.clone());
                            continue;
                        }
                    } else {
                        // Not valid YAML, ignore
                        new_args.push(arg.clone());
                        continue;
                    }
                } else {
                    // Cannot read, ignore
                    new_args.push(arg.clone());
                    continue;
                }
            };

            // Convert
            let filename = path.file_stem().unwrap().to_string_lossy();
            let output_path = intermediates_dir.join(format!("{}.{}", filename, target_extension));

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
    // Check for standard Wix root
    if let Some(wix) = data.get("Wix") {
        return check_wix_namespace(wix);
    }
    // Check for Include root (wxi)
    if let Some(_include) = data.get("Include") {
        return true; // Simple check for now, technically namespaces might not be strict on Includes or mapped differently
    }
    // Check for WixLocalization root (wxl)
    if let Some(_wxl) = data.get("WixLocalization") {
        // WXL namespace is usually http://wixtoolset.org/schemas/v4/wxl
        // For now, accept it if the root exists
        return true;
    }
    false
}

fn check_wix_namespace(node: &Value) -> bool {
    // Check for namespace in various forms
    if let Some(ns) = node.get("xmlns")
        && ns.as_str() == Some(WIX_NAMESPACE)
    {
        return true;
    }
    if let Some(ns) = node.get("@xmlns")
        && ns.as_str() == Some(WIX_NAMESPACE)
    {
        return true;
    }
    // Also check separate attribute strategy
    if let Some(attrs) = node.get("attributes")
        && let Some(ns) = attrs.get("xmlns")
        && ns.as_str() == Some(WIX_NAMESPACE)
    {
        return true;
    }
    false
}
