use std::collections::HashSet;

use wixer::{AttributeStrategy, wix_to_yaml, yaml_to_wix};

fn known_attributes() -> HashSet<String> {
    ["Name", "Id", "Version", "UpgradeCode"]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn wix_to_yaml_preserves_general_cmake_at_var_at_placeholders_in_attribute_values() {
    // These placeholders are common in CMake configure_file / CPack templates.
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="@PRODUCT_NAME@" Version="@PRODUCT_VERSION@" UpgradeCode="PUT-GUID-HERE" />
</Wix>
"#;

    let yaml_str =
        wix_to_yaml::convert_xml_str_to_yaml_str(xml, AttributeStrategy::Inline).unwrap();

    // The key invariant: placeholders must not be eaten or rewritten.
    assert!(yaml_str.contains("@PRODUCT_NAME@"));
    assert!(yaml_str.contains("@PRODUCT_VERSION@"));
}

#[test]
fn wix_to_yaml_preserves_lowercase_and_digit_placeholders() {
    // Allow typical variables like @project_name@ or @VERSION_1@.
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="@project_name@" Version="@VERSION_1@" />
</Wix>
"#;

    let yaml_str =
        wix_to_yaml::convert_xml_str_to_yaml_str(xml, AttributeStrategy::Inline).unwrap();

    assert!(yaml_str.contains("@project_name@"));
    assert!(yaml_str.contains("@VERSION_1@"));
}

#[test]
fn yaml_to_wix_does_not_treat_at_var_at_placeholders_as_yaml_attributes() {
    // In YAML, we represent placeholders as string values. YAML->XML should keep
    // @VAR@ untouched (it should stay an attribute value, not become a YAML key).
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: '@PRODUCT_NAME@'
    Version: '@PRODUCT_VERSION@'
"#,
    )
    .unwrap();

    let xml = yaml_to_wix::convert_value_to_string(
        &yaml_data,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    assert!(xml.contains("Name=\"@PRODUCT_NAME@\""));
    assert!(xml.contains("Version=\"@PRODUCT_VERSION@\""));
}

#[test]
fn yaml_to_wix_restores_both_cpack_custom_and_patch_xmlns_tokens() {
    // Both tokens are handled as special cases because they can break XML validity
    // (they are used to inject xmlns declarations).
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  CPACK_WIX_CUSTOM_XMLNS_EXPANDED: 1
  CPACK_WIX_PATCH_XMLNS_EXPANDED: 1
  Package:
    Name: 'MyApp'
"#,
    )
    .unwrap();

    let xml = yaml_to_wix::convert_value_to_string(
        &yaml_data,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    assert!(xml.contains("@CPACK_WIX_CUSTOM_XMLNS_EXPANDED@"));
    assert!(xml.contains("@CPACK_WIX_PATCH_XMLNS_EXPANDED@"));
    assert!(!xml.contains("CPACK_WIX_CUSTOM_XMLNS_EXPANDED=\"1\""));
    assert!(!xml.contains("CPACK_WIX_PATCH_XMLNS_EXPANDED=\"1\""));
}
