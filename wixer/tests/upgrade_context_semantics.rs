use wixer::upgrade::UpgradeContext;

#[test]
fn upgrade_context_fallback_scanning_does_not_set_property_without_value_attribute() {
    let mut ctx = UpgradeContext::new();

    let content = r#"<?xml version="1.0"?>
<?include Variables.wxi?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product>
    <Property Id="WIXUI_INSTALLDIR" />
    <UIRef Id="WixUI_InstallDir"/>
  </Product>
</Wix>"#;

    ctx.analyze_content(content).unwrap();

    assert_eq!(ctx.get_wixui_install_directory(), None);
    assert!(ctx.has_ui_ref("WixUI_InstallDir"));
    assert!(ctx.needs_ui_namespace);
}

#[test]
fn upgrade_context_fallback_scanning_runs_when_xml_is_invalid() {
    let mut ctx = UpgradeContext::new();

    let content = r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product>
    <Property Id="WIXUI_INSTALLDIR" Value="INSTALLFOLDER"/>
    <UIRef Id="WixUI_InstallDir"/>
  </Product
</Wix>"#;

    ctx.analyze_content(content).unwrap();

    assert_eq!(ctx.get_wixui_install_directory(), Some("INSTALLFOLDER"));
    assert!(ctx.has_ui_ref("WixUI_InstallDir"));
    assert!(ctx.needs_ui_namespace);
}

#[test]
fn upgrade_context_fallback_scanning_detects_non_install_dir_ui_refs() {
    let mut ctx = UpgradeContext::new();

    let content = r#"<?xml version="1.0"?>
<?include Variables.wxi?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product>
    <UIRef Id="WixUI_Mondo"/>
  </Product>
</Wix>"#;

    ctx.analyze_content(content).unwrap();
    assert!(ctx.has_ui_ref("WixUI_Mondo"));
    assert!(ctx.needs_ui_namespace);
}

#[test]
fn upgrade_context_analyze_files_skips_missing_paths() {
    use std::path::Path;

    let missing = Path::new("this_file_does_not_exist.wxs");
    let ctx = wixer::upgrade::analyze_files(&[missing]).unwrap();

    assert!(!ctx.needs_ui_namespace);
    assert!(ctx.ui_refs.is_empty());
    assert!(ctx.properties.is_empty());
}

#[test]
fn upgrade_context_analyze_files_aggregates_context() {
    use std::fs;
    use std::path::Path;

    let dir = Path::new("target/test_upgrade_context_semantics");
    let _ = fs::create_dir_all(dir);

    let f1 = dir.join("a.wxs");
    let f2 = dir.join("b.wxs");

    fs::write(
        &f1,
        r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product>
    <Property Id="WIXUI_INSTALLDIR" Value="INSTALLFOLDER" />
  </Product>
</Wix>"#,
    )
    .unwrap();

    fs::write(
        &f2,
        r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product>
    <UIRef Id="WixUI_InstallDir" />
  </Product>
</Wix>"#,
    )
    .unwrap();

    let ctx = wixer::upgrade::analyze_files(&[&f1, &f2]).unwrap();

    assert_eq!(ctx.get_wixui_install_directory(), Some("INSTALLFOLDER"));
    assert!(ctx.has_ui_ref("WixUI_InstallDir"));
    assert!(ctx.needs_ui_namespace);
}
