use wixer::upgrade::{UpgradeContext, upgrade_content_with_context};

#[test]
fn upgrade_pipeline_upgrades_include_root_with_and_without_children() {
    let input_empty = r#"<?xml version="1.0" encoding="utf-8"?>
<Include xmlns="http://schemas.microsoft.com/wix/2006/wi"/>"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input_empty, &ctx).unwrap();
    assert!(out.contains("<Include xmlns=\"http://wixtoolset.org/schemas/v4/wxs\""));

    let input_children = r#"<?xml version="1.0" encoding="utf-8"?>
<Include xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <Directory Id="ProgramFilesFolder" />
  </Fragment>
</Include>"#;

    let out = upgrade_content_with_context(input_children, &ctx).unwrap();
    assert!(out.contains("<Include xmlns=\"http://wixtoolset.org/schemas/v4/wxs\""));
    assert!(out.contains("<StandardDirectory"));
}

#[test]
fn upgrade_pipeline_handles_targetdir_skip_and_standard_directory_children() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <Directory Id="TARGETDIR">
      <Directory Id="ProgramFilesFolder">
        <Component Id="C1" />
      </Directory>
    </Directory>
  </Fragment>
</Wix>"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(!out.contains("Id=\"TARGETDIR\""));
    assert!(out.contains("<StandardDirectory"));
    assert!(out.contains("Id=\"ProgramFilesFolder\""));
    assert!(out.contains("<Component"));
}

#[test]
fn upgrade_pipeline_merges_inner_package_attributes_and_renames_scope() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Test" Version="1.0.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package InstallerVersion="200" Compressed="yes" InstallScope="perMachine" />
  </Product>
</Wix>"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("<Package"));
    assert!(out.contains("ProductCode=\"*\""));
    assert!(out.contains("Scope=\"perMachine\""));
    assert!(!out.contains("InstallScope=\"perMachine\""));
    assert!(!out.contains("InstallerVersion=\"200\""));
}

#[test]
fn upgrade_pipeline_preserves_comment_in_prolog_between_decl_and_root() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<!--prolog comment-->
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Test" Version="1.0.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package />
  </Product>
</Wix>"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("<!--prolog comment-->"));
    assert!(out.contains("<Wix xmlns=\"http://wixtoolset.org/schemas/v4/wxs\""));
}
