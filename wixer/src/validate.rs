use anyhow::{Context, Result};
use jsonschema::JSONSchema;
use std::fs;
use std::path::Path;

pub fn validate_file(schema_path: &Path, yaml_path: &Path) -> Result<()> {
    println!(
        "Validating {} against {}...",
        yaml_path.display(),
        schema_path.display()
    );

    // Load Schema
    let schema_content = fs::read_to_string(schema_path)
        .with_context(|| format!("Failed to read schema file: {}", schema_path.display()))?;
    let schema_json: serde_json::Value =
        serde_json::from_str(&schema_content).with_context(|| "Failed to parse schema JSON")?;

    // Compile Schema
    let compiled_schema =
        JSONSchema::compile(&schema_json).map_err(|e| anyhow::anyhow!("Invalid schema: {}", e))?;

    // Load YAML and convert to JSON
    let yaml_content = fs::read_to_string(yaml_path)
        .with_context(|| format!("Failed to read YAML file: {}", yaml_path.display()))?;
    let yaml_value: serde_yaml::Value =
        serde_yaml::from_str(&yaml_content).with_context(|| "Failed to parse YAML")?;

    let json_value =
        serde_json::to_value(yaml_value).with_context(|| "Failed to convert YAML to JSON")?;

    // Validate
    let result = compiled_schema.validate(&json_value);

    if let Err(errors) = result {
        println!("✗ Validation failed:");
        for error in errors {
            println!("  - {} (at {})", error, error.instance_path);
        }
        anyhow::bail!("Validation failed");
    } else {
        println!("✓ YAML is valid!");
    }

    Ok(())
}

pub fn validate_directory(schema_dir: &Path, yaml_dir: &Path) -> Result<()> {
    println!(
        "Validating YAML files in {} against schemas in {}...",
        yaml_dir.display(),
        schema_dir.display()
    );

    // This assumes a simple mapping or tries to match schema by name
    // For simplicity, we'll assume there is a 'wix.schema.json' or similar main schema
    // Or we could validate each YAML file against a schema with a matching name.

    let default_schema = schema_dir.join("wix.schema.json");
    if !default_schema.exists() {
        println!("Warning: wix.schema.json not found in schema directory. Will attempt to match by filename.");
    }

    let mut count = 0;
    let mut success_count = 0;

    for entry in fs::read_dir(yaml_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file()
            && let Some(ext) = path.extension()
                && (ext == "yaml" || ext == "yml") {
                    count += 1;

                    // Determine schema to use
                    // 1. Try file specific schema: name.schema.json
                    // 2. Try default wix.schema.json

                    let filename = path.file_stem().unwrap().to_string_lossy();
                    let specific_schema = schema_dir.join(format!("{}.schema.json", filename));

                    let schema_to_use = if specific_schema.exists() {
                        specific_schema
                    } else if default_schema.exists() {
                        default_schema.clone()
                    } else {
                        println!("Skipping {}: No matching schema found", path.display());
                        continue;
                    };

                    match validate_file(&schema_to_use, &path) {
                        Ok(_) => success_count += 1,
                        Err(_) => println!("Validation failed for {}", path.display()),
                    }
                    println!();
                }
    }

    println!("Validation complete: {}/{} valid", success_count, count);

    if success_count < count {
        anyhow::bail!("Some files failed validation");
    }

    Ok(())
}
