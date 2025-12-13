use wixer::upgrade::{UpgradeContext, upgrade_content_with_context};

#[test]
fn upgrade_pipeline_applies_core_transformations() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <Condition Message="Nope">A AND B</Condition>

    <Property Id="WIXUI_INSTALLDIR" Value="INSTALLFOLDER" />
    <UIRef Id="WixUI_InstallDir" />

    <CustomAction Id="CA1" BinaryKey="Bin" FileKey="Foo" Win64="yes" />
    <CustomAction Id="CA2" BinaryKey="WixCA" DllEntry="WixQuietExec" />

    <RegistrySearch Id="RS1" Win64="yes" />

    <Component Id="C1" Win64="yes">
      <File Id="F1" Source="a.txt" />
    </Component>

    <Control Id="Ctl">
      <Text>Hello</Text>
      <Condition Action="disable">NOT Installed</Condition>
    </Control>

    <Publish Dialog="Welcome">1</Publish>
  </Product>
</Wix>
"#;

    let mut ctx = UpgradeContext::new();
    ctx.analyze_content(input).unwrap();

    let out = upgrade_content_with_context(input, &ctx).unwrap();

    // Basic v4 root
    assert!(out.contains("<Wix xmlns=\"http://wixtoolset.org/schemas/v4/wxs\""));
    assert!(out.contains("xmlns:ui=\"http://wixtoolset.org/schemas/v4/wxs/ui\""));

    // Condition -> Launch
    assert!(out.contains("<Launch"));
    assert!(out.contains("Condition=\"A AND B\""));
    assert!(out.contains("Message=\"Nope\""));

    // UIRef -> ui:WixUI + WIXUI_INSTALLDIR skipped
    assert!(out.contains("<ui:WixUI"));
    assert!(out.contains("InstallDirectory=\"INSTALLFOLDER\""));
    assert!(!out.contains("WIXUI_INSTALLDIR"));

    // CustomAction attribute renames
    assert!(out.contains("<CustomAction"));
    assert!(out.contains("BinaryRef=\"Bin\""));
    assert!(out.contains("FileRef=\"Foo\""));

    // WixQuietExec mapping
    assert!(out.contains("BinaryRef=\"Wix4UtilCA_X86\""));

    // Deprecated attr removed
    assert!(!out.contains("Win64=\"yes\""));

    // Control Text and Condition children were converted/skipped
    assert!(out.contains("Text=\"Hello\""));
    assert!(out.contains("DisableCondition=\"NOT Installed\""));
    assert!(!out.contains("<Text>"));
    assert!(!out.contains("<Condition Action=\"disable\">"));

    // Inner text to Condition attribute
    assert!(out.contains("<Publish"));
    assert!(out.contains("Condition=\"1\""));
}

#[test]
fn upgrade_pipeline_converts_custom_and_setproperty_inner_text_to_condition() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <Custom Action="DoAction">NOT Installed</Custom>
    <SetProperty Id="P1" Value="V1">A AND B</SetProperty>
  </Product>
</Wix>
"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("<Custom"));
    assert!(out.contains("Condition=\"NOT Installed\""));
    assert!(out.contains("<SetProperty"));
    assert!(out.contains("Condition=\"A AND B\""));
}

#[test]
fn upgrade_pipeline_control_condition_enable_disable_are_converted_and_other_actions_ignored() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <Control Id="Ctl">
      <Condition Action="enable">A</Condition>
      <Condition Action="disable">B</Condition>
      <Condition Action="show">C</Condition>
    </Control>
  </Product>
</Wix>
"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("<Control"));
    assert!(out.contains("EnableCondition=\"A\""));
    assert!(out.contains("DisableCondition=\"B\""));
    assert!(!out.contains("Action=\"show\""));
    assert!(!out.contains(">C<"));
}

#[test]
fn upgrade_pipeline_removes_win64_attributes_even_when_element_has_children() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <Component Id="C1" Win64="yes">
      <File Id="F1" Source="a.txt" />
    </Component>
  </Product>
</Wix>
"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("<Component"));
    assert!(out.contains("Id=\"C1\""));
    assert!(!out.contains("Win64=\"yes\""));
    assert!(out.contains("<File"));
}

#[test]
fn upgrade_pipeline_wixui_install_dir_fallback_finds_sibling_property_and_skips_it() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <Property Id="WIXUI_INSTALLDIR" Value="INSTALLFOLDER" />
    <UIRef Id="WixUI_InstallDir" />
    <UIRef Id="WixUI_Minimal" />
  </Product>
</Wix>
"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("xmlns:ui=\"http://wixtoolset.org/schemas/v4/wxs/ui\""));
    assert!(out.contains("<ui:WixUI"));
    assert!(out.contains("InstallDirectory=\"INSTALLFOLDER\""));
    assert!(!out.contains("WIXUI_INSTALLDIR"));

    assert!(out.contains("Id=\"WixUI_Minimal\""));
}

#[test]
fn upgrade_pipeline_does_not_emit_launch_when_condition_text_is_empty() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <Condition Message="Nope">   
    </Condition>
  </Product>
</Wix>
"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(!out.contains("<Launch"));
}

#[test]
fn upgrade_pipeline_custom_action_transform_maps_binarykey_and_filekey_without_quiet_exec() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <CustomAction Id="CA1" BinaryKey="Bin" FileKey="Foo" DllEntry="SomethingElse" />
  </Product>
</Wix>
"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("BinaryRef=\"Bin\""));
    assert!(out.contains("FileRef=\"Foo\""));
    assert!(!out.contains("BinaryKey=\"Bin\""));
    assert!(!out.contains("FileKey=\"Foo\""));
}

#[test]
fn upgrade_pipeline_custom_action_transform_maps_wix_quiet_exec_64_variant() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <CustomAction Id="CA2" BinaryKey="WixCA" DllEntry="WixQuietExec64" />
  </Product>
</Wix>
"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("BinaryRef=\"Wix4UtilCA_X86\""));
}

#[test]
fn upgrade_pipeline_condition_to_launch_uses_default_message_when_missing() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <Condition>Installed</Condition>
  </Product>
</Wix>
"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("<Launch"));
    assert!(out.contains("Condition=\"Installed\""));
    assert!(out.contains("Message=\"Installation cannot continue.\""));
}

#[test]
fn upgrade_pipeline_control_text_to_attribute_preserves_other_children() {
    let input = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MyApp" Language="1033" Version="1.0.0.0" Manufacturer="MyCo" UpgradeCode="d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a">
    <Control Id="Ctl">
      <Text>Hello</Text>
      <Control Id="ChildCtl" />
    </Control>
  </Product>
</Wix>
"#;

    let ctx = UpgradeContext::new();
    let out = upgrade_content_with_context(input, &ctx).unwrap();

    assert!(out.contains("<Control"));
    assert!(out.contains("Text=\"Hello\""));
    assert!(out.contains("<Control Id=\"ChildCtl\""));
    assert!(!out.contains("<Text>Hello</Text>"));
}

#[test]
fn upgrade_transformations_expose_stable_names_for_debugging() {
    use wixer::upgrade::{
        ConditionToLaunch, ControlConditionToAttribute, ControlTextToAttribute,
        CustomActionTransform, InnerTextToCondition, RemoveDeprecatedAttributes,
        SkipControlTextAndCondition, SkipWixUIInstallDirProperty, Transformation, UIRefToWixUI,
        standard_transformations,
    };

    let mut names = Vec::new();
    for t in standard_transformations() {
        names.push(t.name());
    }

    // Also call name() on concrete types directly to exercise those methods.
    names.push(ConditionToLaunch.name());
    names.push(InnerTextToCondition.name());
    names.push(CustomActionTransform.name());
    names.push(RemoveDeprecatedAttributes.name());
    names.push(ControlTextToAttribute.name());
    names.push(ControlConditionToAttribute.name());
    names.push(SkipControlTextAndCondition.name());
    names.push(UIRefToWixUI.name());
    names.push(SkipWixUIInstallDirProperty.name());

    // Ensure they are non-empty and contain a couple expected values.
    assert!(names.iter().all(|n| !n.trim().is_empty()));
    assert!(names.contains(&"ConditionToLaunch"));
    assert!(names.contains(&"CustomActionTransform"));
}
