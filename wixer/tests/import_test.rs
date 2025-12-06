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
            "--",
            "import",
            input_dir.to_str().unwrap(),
            output_dir.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run wixer import");

    assert!(status.success(), "Import command failed");

    // Verify outputs
    let frag1_wys = output_dir.join("fragment1.wys");
    assert!(frag1_wys.exists(), "fragment1.wys should exist");

    let template_wys_in = output_dir.join("subdir/template.wys.in");
    assert!(
        template_wys_in.exists(),
        "subdir/template.wys.in should exist"
    );

    let config_wyi = output_dir.join("config.wyi");
    assert!(config_wyi.exists(), "config.wyi should exist");

    let localization_wyl = output_dir.join("en-us.wyl");
    assert!(localization_wyl.exists(), "en-us.wyl should exist");

    // Check content of template to ensure @VAR@ syntax is preserved/handled
    let content = fs::read_to_string(template_wys_in).unwrap();
    assert!(
        content.contains("@PRODUCT_NAME@"),
        "Should preserve CMake variables"
    );

    // Check config.wyi
    let config_content = fs::read_to_string(config_wyi).unwrap();
    assert!(
        config_content.contains("Include"),
        "Should import Include element"
    );
    assert!(
        config_content.contains("WixPerMachineFolder"),
        "Should import property value"
    );

    // Check localization
    let wxl_content = fs::read_to_string(localization_wyl).unwrap();
    assert!(
        wxl_content.contains("WixLocalization"),
        "Should import WixLocalization element"
    );
    assert!(
        wxl_content.contains("Welcome to My Application"),
        "Should import string value"
    );
}
