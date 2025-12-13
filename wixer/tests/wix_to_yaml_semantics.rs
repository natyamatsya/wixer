mod support;

use wixer::{AttributeStrategy, wix_to_yaml};

#[test]
fn wix_to_yaml_captures_attributes_repeated_elements_and_text() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="MyApp">
    <Feature Id="A" />
    <Feature Id="B" />
  </Package>
</Wix>
"#;

    let yaml_str =
        wix_to_yaml::convert_xml_str_to_yaml_str(xml, AttributeStrategy::Inline).unwrap();
    let root: serde_yaml::Value = serde_yaml::from_str(&yaml_str).unwrap();

    let wix = support::yaml_get(&root, "Wix");
    assert_eq!(
        support::yaml_get(wix, "xmlns").as_str().unwrap(),
        "http://wixtoolset.org/schemas/v4/wxs"
    );

    let package = support::yaml_get(wix, "Package");
    assert_eq!(
        support::yaml_get(package, "Name").as_str().unwrap(),
        "MyApp"
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

#[test]
fn wix_to_yaml_stores_inner_text_as_value() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<WixLocalization>
  <String Id="WelcomeText">Welcome to MyApp</String>
</WixLocalization>
"#;

    let yaml_str =
        wix_to_yaml::convert_xml_str_to_yaml_str(xml, AttributeStrategy::Inline).unwrap();
    let root: serde_yaml::Value = serde_yaml::from_str(&yaml_str).unwrap();

    let wxl = support::yaml_get(&root, "WixLocalization");
    let s = support::yaml_get(wxl, "String");
    assert_eq!(support::yaml_get(s, "Id").as_str().unwrap(), "WelcomeText");
    assert_eq!(
        support::yaml_get(s, "_value").as_str().unwrap(),
        "Welcome to MyApp"
    );
}

#[test]
fn wix_to_yaml_converts_include_processing_instruction() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix>
  <?include Common.wxi?>
  <Package Name="MyApp" />
</Wix>
"#;

    let yaml_str =
        wix_to_yaml::convert_xml_str_to_yaml_str(xml, AttributeStrategy::Inline).unwrap();
    let root: serde_yaml::Value = serde_yaml::from_str(&yaml_str).unwrap();

    let wix = support::yaml_get(&root, "Wix");
    assert_eq!(
        support::yaml_get(wix, "_include").as_str().unwrap(),
        "!include Common.wxi.yaml"
    );
}

#[test]
fn wix_to_yaml_ignores_non_include_processing_instructions() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix>
  <?define Foo=Bar?>
  <Package Name="MyApp" />
</Wix>
"#;

    let yaml_str =
        wix_to_yaml::convert_xml_str_to_yaml_str(xml, AttributeStrategy::Inline).unwrap();
    let root: serde_yaml::Value = serde_yaml::from_str(&yaml_str).unwrap();

    let wix = support::yaml_get(&root, "Wix");
    assert!(wix.get("_include").is_none());
}

#[test]
fn wix_to_yaml_preserves_comments_as_comment_property() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Wix>
  <!--Hello-->
  <Package Name="MyApp" />
</Wix>
"#;

    let yaml_str =
        wix_to_yaml::convert_xml_str_to_yaml_str(xml, AttributeStrategy::Inline).unwrap();
    let root: serde_yaml::Value = serde_yaml::from_str(&yaml_str).unwrap();

    let wix = support::yaml_get(&root, "Wix");
    let package = support::yaml_get(wix, "Package");
    assert_eq!(
        support::yaml_get(package, "_comment").as_str().unwrap(),
        "Hello"
    );
}
