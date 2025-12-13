use std::collections::HashSet;

use wixer::{AttributeStrategy, yaml_to_wix};

fn known_attributes() -> HashSet<String> {
    ["Id", "Name", "Version", "Title"]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn yaml_to_wix_prefix_strategy_requires_at_prefix_for_attributes() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  Package:
    "@Name": 'AttrName'
    Name: 'ChildName'
"#,
    )
    .unwrap();

    let xml = yaml_to_wix::convert_value_to_string(
        &yaml_data,
        AttributeStrategy::Prefix,
        &known_attributes(),
    )
    .unwrap();

    let doc = roxmltree::Document::parse(&xml).unwrap();
    let wix = doc.descendants().find(|n| n.has_tag_name("Wix")).unwrap();
    let package = wix
        .children()
        .find(|n| n.is_element() && n.has_tag_name("Package"))
        .unwrap();

    assert_eq!(package.attribute("Name").unwrap(), "AttrName");

    let name_child = package
        .children()
        .find(|n| n.is_element() && n.has_tag_name("Name"))
        .unwrap();
    assert_eq!(name_child.text().unwrap(), "ChildName");
}

#[test]
fn yaml_to_wix_separate_strategy_uses_attributes_and_elements_properties() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  Package:
    attributes:
      Name: 'MyApp'
      Version: '1.0.0.0'
    elements:
      Feature:
        attributes:
          Id: 'Main'
"#,
    )
    .unwrap();

    let xml = yaml_to_wix::convert_value_to_string(
        &yaml_data,
        AttributeStrategy::Separate,
        &known_attributes(),
    )
    .unwrap();

    let doc = roxmltree::Document::parse(&xml).unwrap();
    let wix = doc.descendants().find(|n| n.has_tag_name("Wix")).unwrap();
    let package = wix
        .children()
        .find(|n| n.is_element() && n.has_tag_name("Package"))
        .unwrap();

    assert_eq!(package.attribute("Name").unwrap(), "MyApp");
    assert_eq!(package.attribute("Version").unwrap(), "1.0.0.0");

    let feature = package
        .children()
        .find(|n| n.is_element() && n.has_tag_name("Feature"))
        .unwrap();
    assert_eq!(feature.attribute("Id").unwrap(), "Main");
}

#[test]
fn yaml_to_wix_separate_strategy_treats_plain_scalars_as_elements() {
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  Package:
    Name: 'MyApp'
"#,
    )
    .unwrap();

    let xml = yaml_to_wix::convert_value_to_string(
        &yaml_data,
        AttributeStrategy::Separate,
        &known_attributes(),
    )
    .unwrap();

    let doc = roxmltree::Document::parse(&xml).unwrap();
    let wix = doc.descendants().find(|n| n.has_tag_name("Wix")).unwrap();
    let package = wix
        .children()
        .find(|n| n.is_element() && n.has_tag_name("Package"))
        .unwrap();

    assert!(package.attribute("Name").is_none());

    let name_child = package
        .children()
        .find(|n| n.is_element() && n.has_tag_name("Name"))
        .unwrap();
    assert_eq!(name_child.text().unwrap(), "MyApp");
}
