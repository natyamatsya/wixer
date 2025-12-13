use std::fs;
use std::path::PathBuf;

use uuid::Uuid;

use wixer::upgrade::{upgrade_file, upgrade_files};

#[test]
fn upgrade_file_writes_upgraded_output_file() {
    let dir = std::env::temp_dir().join(format!("wixer_test_{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let input = dir.join("input.wxs");
    let output = dir.join("output.wxs");

    let content = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Test" Version="1.0.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package />
  </Product>
</Wix>"#;

    fs::write(&input, content).unwrap();

    upgrade_file(&input, &output).unwrap();

    let out = fs::read_to_string(&output).unwrap();
    assert!(out.contains("http://wixtoolset.org/schemas/v4/wxs"));
    assert!(out.contains("<Package"));
}

#[test]
fn upgrade_files_processes_multiple_files_with_shared_context() {
    let dir = std::env::temp_dir().join(format!("wixer_test_{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let input1 = dir.join("a.wxs");
    let input2 = dir.join("b.wxs");
    let output1 = dir.join("a_out.wxs");
    let output2 = dir.join("b_out.wxs");

    // File 1 provides the install dir property.
    fs::write(
        &input1,
        r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Test" Version="1.0.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package />
    <Property Id="WIXUI_INSTALLDIR" Value="INSTALLFOLDER" />
  </Product>
</Wix>"#,
    )
    .unwrap();

    // File 2 references WixUI_InstallDir so it should require UI namespace.
    fs::write(
        &input2,
        r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Test" Version="1.0.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package />
    <UIRef Id="WixUI_InstallDir" />
  </Product>
</Wix>"#,
    )
    .unwrap();

    let files: Vec<(PathBuf, PathBuf)> = vec![
        (input1.clone(), output1.clone()),
        (input2.clone(), output2.clone()),
    ];

    upgrade_files(&files).unwrap();

    let out1 = fs::read_to_string(&output1).unwrap();
    let out2 = fs::read_to_string(&output2).unwrap();

    assert!(out1.contains("http://wixtoolset.org/schemas/v4/wxs"));
    assert!(out2.contains("xmlns:ui=\"http://wixtoolset.org/schemas/v4/wxs/ui\""));
}
