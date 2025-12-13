mod support;

use std::collections::HashSet;

use wixer::{AttributeStrategy, wix_to_yaml, yaml_to_wix};

fn known_attributes() -> HashSet<String> {
    ["xmlns", "Id", "Name", "Version", "Manufacturer", "Title"]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn roundtrip_yaml_to_xml_to_yaml_preserves_core_semantics() {
    let input_yaml: serde_yaml::Value = serde_yaml::from_str(
        r#"Wix:
  _comment: 'Hello'
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: 'MyApp'
    Version: '1.0.0.0'
    Manufacturer: 'MyCo'
    Feature:
      - Id: 'A'
        Title: 'FeatA'
      - Id: 'B'
        Title: 'FeatB'
"#,
    )
    .unwrap();

    let xml = yaml_to_wix::convert_value_to_string(
        &input_yaml,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    let yaml_out =
        wix_to_yaml::convert_xml_str_to_yaml_str(&xml, AttributeStrategy::Inline).unwrap();
    let out_val: serde_yaml::Value = serde_yaml::from_str(&yaml_out).unwrap();

    let wix = support::yaml_get(&out_val, "Wix");
    assert_eq!(
        support::yaml_get(wix, "_comment").as_str().unwrap(),
        "Hello"
    );
    assert_eq!(
        support::yaml_get(wix, "xmlns").as_str().unwrap(),
        "http://wixtoolset.org/schemas/v4/wxs"
    );

    let package = support::yaml_get(wix, "Package");
    assert_eq!(
        support::yaml_get(package, "Name").as_str().unwrap(),
        "MyApp"
    );
    assert_eq!(
        support::yaml_get(package, "Version").as_str().unwrap(),
        "1.0.0.0"
    );
    assert_eq!(
        support::yaml_get(package, "Manufacturer").as_str().unwrap(),
        "MyCo"
    );

    let features = support::yaml_get(package, "Feature").as_sequence().unwrap();
    assert_eq!(features.len(), 2);
    let ids: Vec<_> = features
        .iter()
        .map(|v| support::yaml_get(v, "Id").as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&"A".to_string()));
    assert!(ids.contains(&"B".to_string()));
}
