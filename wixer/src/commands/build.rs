use crate::common::AttributeStrategy;
use crate::utils::find_wixer_root;
use crate::yaml_to_wix;
use anyhow::Result;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

pub fn run(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        eprintln!("No arguments provided for build");
        std::process::exit(1);
    }

    // Check if it's actually the 'build' command
    if args[0] != "build" {
        // Pass through other commands to wix
        let status = std::process::Command::new("wix")
            .args(args)
            .status()
            .unwrap_or_else(|e| {
                eprintln!("Failed to execute wix: {}", e);
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
            // Check content for Wix root
            if let Ok(content) = std::fs::read_to_string(&path) {
                if content.contains("Wix:") || content.contains("Wix") {
                    // Simple check, parsed later
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
                            eprintln!("Failed to convert {}: {}", path.display(), e);
                            // Fallback to original arg? Or fail?
                            // Fail is safer
                            std::process::exit(1);
                        }
                    }
                }
            }
        }
        new_args.push(arg.clone());
    }

    println!("Running: wix {}", new_args.join(" "));
    let status = std::process::Command::new("wix")
        .args(&new_args[1..]) // Skip "build" as it is in new_args[0] but we ran "wix"
        .status()
        .unwrap_or_else(|e| {
            eprintln!("Failed to execute wix: {}", e);
            std::process::exit(1);
        });
    std::process::exit(status.code().unwrap_or(1));
}
