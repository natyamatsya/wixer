use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_recursive_export() {
    let input_dir = Path::new("tests/export_test_data");
    let output_dir = Path::new("target/test_export_output");

    // Clean output
    if output_dir.exists() {
        fs::remove_dir_all(output_dir).unwrap();
    }

    let status = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "wixer",
            "--",
            "export",
            input_dir.to_str().unwrap(),
            output_dir.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run wixer export");

    assert!(status.success(), "Export command failed");

    // Verify outputs
    let frag1_wxs = output_dir.join("fragment1.wxs");
    assert!(frag1_wxs.exists(), "fragment1.wxs should exist");

    let template_wxs_in = output_dir.join("subdir/template.wxs.in");
    assert!(
        template_wxs_in.exists(),
        "subdir/template.wxs.in should exist"
    );

    // Check content
    let content = fs::read_to_string(frag1_wxs).unwrap();
    assert!(
        content.contains("<Fragment>"),
        "Should contain Fragment element"
    );
    assert!(content.contains("MainComponents"), "Should contain ID");

    let template_content = fs::read_to_string(template_wxs_in).unwrap();
    assert!(
        template_content.contains("@PRODUCT_NAME@"),
        "Should preserve variables"
    );
}
