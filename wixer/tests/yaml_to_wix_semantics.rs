use std::collections::HashSet;

use wixer::{AttributeStrategy, yaml_to_wix};

fn known_attributes() -> HashSet<String> {
    [
        "xmlns",
        "Id",
        "Name",
        "Version",
        "Manufacturer",
        "UpgradeCode",
        "Scope",
        "Title",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

#[test]
fn yaml_to_wix_emits_attributes_and_children() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: 'MyApp'
    Version: '1.0.0.0'
    Manufacturer: 'MyCo'
    UpgradeCode: 'd0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a'
    Scope: 'perMachine'
    Feature:
      Id: 'Main'
      Title: 'MyApp'
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
    assert_eq!(ns, "http://wixtoolset.org/schemas/v4/wxs");

    let package = wix
        .children()
        .find(|n| n.is_element() && n.has_tag_name((ns, "Package")))
        .unwrap();
    assert_eq!(package.attribute("Name").unwrap(), "MyApp");
    assert_eq!(package.attribute("Version").unwrap(), "1.0.0.0");
    assert_eq!(package.attribute("Manufacturer").unwrap(), "MyCo");
    assert_eq!(package.attribute("Scope").unwrap(), "perMachine");

    let feature = package
        .children()
        .find(|n| n.is_element() && n.has_tag_name((ns, "Feature")))
        .unwrap();
    assert_eq!(feature.attribute("Id").unwrap(), "Main");
    assert_eq!(feature.attribute("Title").unwrap(), "MyApp");
}

#[test]
fn yaml_to_wix_emits_text_from_value_property() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"WixLocalization:
  String:
    - Id: 'WelcomeText'
      _value: 'Welcome to MyApp'
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

    let s = doc
        .descendants()
        .find(|n| n.has_tag_name("String"))
        .unwrap();
    assert_eq!(s.attribute("Id").unwrap(), "WelcomeText");
    let combined: String = s
        .children()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect();
    assert_eq!(combined.trim(), "Welcome to MyApp");
}

#[test]
fn yaml_to_wix_repeats_elements_for_sequences() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Fragment:
    Component:
      - Id: 'A'
      - Id: 'B'
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

    let components: Vec<_> = doc
        .descendants()
        .filter(|n| n.is_element() && n.has_tag_name((ns, "Component")))
        .collect();
    assert_eq!(components.len(), 2);
    let ids: Vec<_> = components
        .iter()
        .map(|n| n.attribute("Id").unwrap().to_string())
        .collect();
    assert!(ids.contains(&"A".to_string()));
    assert!(ids.contains(&"B".to_string()));
}

#[test]
fn yaml_to_wix_writes_comment_before_element() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  _comment: 'Hello'
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
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
    assert!(xml.contains("<!--Hello-->"));
}
