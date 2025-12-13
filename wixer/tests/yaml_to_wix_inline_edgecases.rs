use std::collections::HashSet;

use wixer::{yaml_to_wix, AttributeStrategy};

fn known_attributes() -> HashSet<String> {
    ["Compressed", "InstallPrivileges", "Name", "Id"]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn yaml_to_wix_inline_bool_attributes_map_to_yes_no() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: 'MyApp'
    Compressed: true
"#,
    )
    .unwrap();

    let xml = yaml_to_wix::convert_value_to_string(
        &yaml_data,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    let doc = roxmltree::Document::parse(&xml).unwrap();
    let wix = doc.descendants().find(|n| n.has_tag_name("Wix")).unwrap();
    let ns = wix.tag_name().namespace().unwrap();
    let package = wix
        .children()
        .find(|n| n.is_element() && n.has_tag_name((ns, "Package")))
        .unwrap();

    assert_eq!(package.attribute("Compressed").unwrap(), "yes");
}

#[test]
fn yaml_to_wix_inline_number_attributes_are_stringified() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: 'MyApp'
    InstallPrivileges: 1
"#,
    )
    .unwrap();

    let xml = yaml_to_wix::convert_value_to_string(
        &yaml_data,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    let doc = roxmltree::Document::parse(&xml).unwrap();
    let wix = doc.descendants().find(|n| n.has_tag_name("Wix")).unwrap();
    let ns = wix.tag_name().namespace().unwrap();

    let package = wix
        .children()
        .find(|n| n.is_element() && n.has_tag_name((ns, "Package")))
        .unwrap();

    assert_eq!(package.attribute("InstallPrivileges").unwrap(), "1");
}

#[test]
fn yaml_to_wix_inline_emits_multiple_xmlns_prefix_declarations() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  "xmlns:ui": 'http://wixtoolset.org/schemas/v4/wxs/ui'
  "xmlns:util": 'http://wixtoolset.org/schemas/v4/wxs/util'
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

    assert!(
        xml.contains("xmlns:ui=\"http://wixtoolset.org/schemas/v4/wxs/ui\""),
        "Expected ui namespace declaration"
    );
    assert!(
        xml.contains("xmlns:util=\"http://wixtoolset.org/schemas/v4/wxs/util\""),
        "Expected util namespace declaration"
    );
}
