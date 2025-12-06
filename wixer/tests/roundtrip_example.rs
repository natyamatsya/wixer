use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_roundtrip_example_installer() {
    // Setup paths
    let test_dir = Path::new("target/test_roundtrip_example");
    fs::create_dir_all(test_dir).expect("Failed to create test dir");

    let original_wxs = "examples/wix/v4/00_HelloWorldInstaller/Product.wxs";
    let generated_yaml = test_dir.join("example_installer.yaml");
    let generated_wxs = test_dir.join("example_installer_generated.wxs");

    // 1. Convert WXS to YAML
    let status = Command::new("cargo")
        .args(&[
            "run",
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
            "--",
            "yaml-to-wix",
            generated_yaml.to_str().unwrap(),
            generated_wxs.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run yaml-to-wix");
    assert!(status.success(), "yaml-to-wix failed");

    // 3. Verify output exists and compare critical sections
    assert!(generated_wxs.exists());

    let content = fs::read_to_string(&generated_wxs).unwrap();

    // Verify key elements from Product.wxs are present
    assert!(
        content.contains("Hello World"),
        "Should contain product name"
    );
    assert!(
        content.contains("Acme Corporation"),
        "Should contain manufacturer"
    );
    assert!(
        content.contains("ProgramFilesFolder"),
        "Should contain standard directories"
    );
    assert!(content.contains("Component"), "Should contain components");
    assert!(content.contains("File"), "Should contain files");
    assert!(content.contains("Feature"), "Should contain features");

    // Optional: Read back the original file and compare structure/content more deeply if needed
    // But text containment is good enough for a basic roundtrip verify
}
