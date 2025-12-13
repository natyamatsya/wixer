use std::fs;
use std::path::{Path, PathBuf};

use wixer::validate;

fn make_temp_dir(prefix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "wixer_test_{}_{}_{}",
        prefix,
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn validate_file_removes_xmlns_and_passes_schema_validation() {
    let dir = make_temp_dir("validate_file");

    let schema_path = dir.join("wix.schema.json");
    let yaml_path = dir.join("Product.wxs.yaml");

    fs::write(
        &schema_path,
        r#"{
  "$schema": "https://json-schema.org/draft-07/schema#",
  "type": "object",
  "additionalProperties": false,
  "required": ["Wix"],
  "properties": {
    "Wix": {
      "type": "object",
      "additionalProperties": true,
      "required": ["Package"],
      "properties": {
        "Package": { "type": "object" }
      }
    }
  }
}
"#,
    )
    .unwrap();

    fs::write(
        &yaml_path,
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: 'MyApp'
"#,
    )
    .unwrap();

    validate::validate_file(&schema_path, &yaml_path).unwrap();

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn validate_file_returns_error_when_schema_rejects_document() {
    let dir = make_temp_dir("validate_file_error");

    let schema_path = dir.join("wix.schema.json");
    let yaml_path = dir.join("Product.wxs.yaml");

    fs::write(
        &schema_path,
        r#"{
  "$schema": "https://json-schema.org/draft-07/schema#",
  "type": "object",
  "additionalProperties": false,
  "required": ["Wix"],
  "properties": {
    "Wix": {
      "type": "object",
      "additionalProperties": false,
      "required": ["Package"],
      "properties": {
        "Package": { "type": "object" }
      }
    }
  }
}
"#,
    )
    .unwrap();

    fs::write(
        &yaml_path,
        r#"Wix:
  Package:
    Name: 'MyApp'
  Extra: 'nope'
"#,
    )
    .unwrap();

    let err = validate::validate_file(&schema_path, &yaml_path).unwrap_err();
    assert!(err.to_string().contains("Validation failed"));

    let _ = fs::remove_dir_all(&dir);
}

fn write_json(path: &Path, s: &str) {
    fs::write(path, s).unwrap();
}

#[test]
fn validate_directory_uses_extension_schema_selection() {
    let dir = make_temp_dir("validate_dir");

    let schema_dir = dir.join("schemas");
    let yaml_dir = dir.join("yaml");
    fs::create_dir_all(&schema_dir).unwrap();
    fs::create_dir_all(&yaml_dir).unwrap();

    write_json(
        &schema_dir.join("wix.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft-07/schema#",
  "type": "object",
  "additionalProperties": false,
  "required": ["Wix"],
  "properties": {
    "Wix": {
      "type": "object",
      "additionalProperties": true,
      "required": ["Package"],
      "properties": {
        "Package": { "type": "object" }
      }
    }
  }
}
"#,
    );

    write_json(
        &schema_dir.join("wixloc.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft-07/schema#",
  "type": "object",
  "additionalProperties": false,
  "required": ["WixLocalization"],
  "properties": {
    "WixLocalization": {
      "type": "object",
      "additionalProperties": true
    }
  }
}
"#,
    );

    fs::write(
        yaml_dir.join("Product.wxs.yaml"),
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: 'MyApp'
"#,
    )
    .unwrap();

    fs::write(
        yaml_dir.join("en-us.wxl.yaml"),
        r#"WixLocalization:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxl'
"#,
    )
    .unwrap();

    validate::validate_directory(&schema_dir, &yaml_dir).unwrap();

    let _ = fs::remove_dir_all(&dir);
}
