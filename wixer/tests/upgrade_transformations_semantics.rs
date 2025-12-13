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
