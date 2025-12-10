use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_recursive_import() {
    let input_dir = Path::new("tests/import_test_data");
    let output_dir = Path::new("target/test_import_output");

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
            "import",
            input_dir.to_str().unwrap(),
            output_dir.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run wixer import");

    assert!(status.success(), "Import command failed");

    // Verify outputs
    let frag1_yaml = output_dir.join("fragment1.wxs.yaml");
    assert!(frag1_yaml.exists(), "fragment1.wxs.yaml should exist");

    let template_yaml_in = output_dir.join("subdir/template.wxs.yaml.in");
    assert!(
        template_yaml_in.exists(),
        "subdir/template.wxs.yaml.in should exist"
    );

    let config_yaml = output_dir.join("config.wxi.yaml");
    assert!(config_yaml.exists(), "config.wxi.yaml should exist");

    let localization_yaml = output_dir.join("en-us.wxl.yaml");
    assert!(localization_yaml.exists(), "en-us.wxl.yaml should exist");

    // Check content of template to ensure @VAR@ syntax is preserved/handled
    let content = fs::read_to_string(template_yaml_in).unwrap();
    assert!(
        content.contains("@PRODUCT_NAME@"),
        "Should preserve CMake variables"
    );

    // Check config.wxi.yaml
    let config_content = fs::read_to_string(config_yaml).unwrap();
    assert!(
        config_content.contains("Include"),
        "Should import Include element"
    );
    assert!(
        config_content.contains("WixPerMachineFolder"),
        "Should import property value"
    );

    // Check localization
    let wxl_content = fs::read_to_string(localization_yaml).unwrap();
    assert!(
        wxl_content.contains("WixLocalization"),
        "Should import WixLocalization element"
    );
    assert!(
        wxl_content.contains("Welcome to My Application"),
        "Should import string value"
    );

    // Check comments
    let comment_yaml = output_dir.join("comment.wxs.yaml");
    assert!(comment_yaml.exists(), "comment.wxs.yaml should exist");
    let comment_content = fs::read_to_string(comment_yaml).unwrap();
    assert!(
        comment_content.contains("_comment: ' This is a test comment '"),
        "Should preserve XML comments"
    );
}
