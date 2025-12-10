use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_roundtrip() {
    // Setup paths
    let test_dir = Path::new("target/test_roundtrip");
    fs::create_dir_all(test_dir).expect("Failed to create test dir");

    // We are running from wixer root in cargo test, so path to sample is tests/sample.wxs
    let original_wxs = "tests/sample.wxs";
    let generated_yaml = test_dir.join("sample.yaml");
    let generated_wxs = test_dir.join("sample_generated.wxs");

    // 1. Convert WXS to YAML
    // Note: using cargo run to execute the binary
    let status = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "wixer",
            "--",
            "wix-to-yaml",
            original_wxs,
            generated_yaml.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run wix-to-yaml");
    assert!(status.success(), "wix-to-yaml failed");

    // 2. Convert YAML back to WXS
    let status = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "wixer",
            "--",
            "yaml-to-wix",
            generated_yaml.to_str().unwrap(),
            generated_wxs.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run yaml-to-wix");
    assert!(status.success(), "yaml-to-wix failed");

    // 3. Verify output exists
    assert!(generated_wxs.exists());

    let content = fs::read_to_string(&generated_wxs).unwrap();
    println!("Generated WXS:\n{}", content);

    // Basic containment checks
    assert!(
        content.contains("<Wix"),
        "Output should contain root element"
    );
    assert!(content.contains("Package"), "Output should contain Package");
    assert!(
        content.contains("TestPackage"),
        "Output should contain Name attribute"
    );
    assert!(
        content.contains("yes"),
        "Output should contain boolean attribute mapped correctly"
    );
}
