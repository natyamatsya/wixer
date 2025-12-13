mod support;

use wixer::{AttributeStrategy, wix_to_yaml};

#[test]
fn wix_to_yaml_prefix_strategy_prefixes_attributes_with_at() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix>
  <Package Name="MyApp" />
</Wix>
"#;

    let yaml_str =
        wix_to_yaml::convert_xml_str_to_yaml_str(xml, AttributeStrategy::Prefix).unwrap();
    let root: serde_yaml::Value = serde_yaml::from_str(&yaml_str).unwrap();

    let wix = support::yaml_get(&root, "Wix");
    let package = support::yaml_get(wix, "Package");

    assert_eq!(
        support::yaml_get(package, "@Name").as_str().unwrap(),
        "MyApp"
    );
}
