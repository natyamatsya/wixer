use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

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

fn make_temp_dir(prefix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "wixer_test_{}_{}_{}",
        prefix,
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn yaml_to_wix_convert_file_reports_missing_include_file() {
    let dir = make_temp_dir("include_missing");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(
        &main,
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Fragment:
    !include does_not_exist.yaml
"#,
    )
    .unwrap();

    let err = yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap_err();

    assert!(err.to_string().contains("Failed to read file"));
    assert!(err.to_string().contains("does_not_exist.yaml"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_includes_value_context_replaces_mapping() {
    let dir = make_temp_dir("include_value_context");

    let included = dir.join("package.yaml");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(
        &included,
        r#"Name: 'FromInclude'
Version: '1.0.0.0'
"#,
    )
    .unwrap();

    fs::write(
        &main,
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package: !include package.yaml
"#,
    )
    .unwrap();

    yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    let xml = fs::read_to_string(&output).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let wix = doc.descendants().find(|n| n.has_tag_name("Wix")).unwrap();
    let ns = wix.tag_name().namespace().unwrap();

    let package = wix
        .children()
        .find(|n| n.is_element() && n.has_tag_name((ns, "Package")))
        .unwrap();
    assert_eq!(package.attribute("Name").unwrap(), "FromInclude");
    assert_eq!(package.attribute("Version").unwrap(), "1.0.0.0");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_reports_error_when_mixin_is_not_mapping() {
    let dir = make_temp_dir("include_mixin_non_mapping");

    let included = dir.join("list.yaml");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(&included, "- A\n- B\n").unwrap();

    fs::write(
        &main,
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Fragment:
    !include list.yaml
    Package:
      Name: 'MyApp'
"#,
    )
    .unwrap();

    let err = yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("is not a mapping, cannot be mixed into a mapping")
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_rejects_xml_content_in_yaml_file() {
    let dir = make_temp_dir("reject_xml");
    let input = dir.join("in.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(&input, "<?xml version=\"1.0\"?><Wix />").unwrap();

    let err = yaml_to_wix::convert_file(
        &input,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap_err();

    assert!(err.to_string().contains("contains XML content"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_expands_value_context_includes() {
    let dir = make_temp_dir("include_value_context");

    let included = dir.join("package.yaml");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(
        &included,
        r#"Name: 'FromInclude'
Version: '1.0.0.0'
"#,
    )
    .unwrap();

    fs::write(
        &main,
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    !include package.yaml
"#,
    )
    .unwrap();

    yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    let xml = fs::read_to_string(&output).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let wix = doc.descendants().find(|n| n.has_tag_name("Wix")).unwrap();
    let ns = wix.tag_name().namespace().unwrap();

    let package = wix
        .children()
        .find(|n| n.is_element() && n.has_tag_name((ns, "Package")))
        .unwrap();
    assert_eq!(package.attribute("Name").unwrap(), "FromInclude");
    assert_eq!(package.attribute("Version").unwrap(), "1.0.0.0");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_reports_error_when_mixin_is_not_mapping_into_mapping() {
    let dir = make_temp_dir("include_mixin_non_mapping_into_mapping");

    let included = dir.join("list.yaml");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(&included, "- A\n- B\n").unwrap();

    fs::write(
        &main,
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Fragment:
    Package:
      !include list.yaml
      Name: 'MyApp'
"#,
    )
    .unwrap();

    let err = yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("is not a mapping, cannot be mixed into a mapping")
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_merges_mapping_mixins() {
    let dir = make_temp_dir("include_mixin");

    let mixin = dir.join("mixin.yaml");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(
        &mixin,
        r#"Component:
  - Id: 'A'
"#,
    )
    .unwrap();

    fs::write(
        &main,
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Fragment:
    !include mixin.yaml
    Component:
      - Id: 'B'
"#,
    )
    .unwrap();

    yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    let xml = fs::read_to_string(&output).unwrap();
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

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_expands_sequence_includes() {
    let dir = make_temp_dir("include_seq_expand");

    let items = dir.join("items.yaml");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(
        &items,
        r#"- Id: 'A'
- Id: 'B'
"#,
    )
    .unwrap();

    fs::write(
        &main,
        r#"Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Fragment:
    Component:
      - !include items.yaml
      - Id: 'C'
"#,
    )
    .unwrap();

    yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    let xml = fs::read_to_string(&output).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let wix = doc.descendants().find(|n| n.has_tag_name("Wix")).unwrap();
    let ns = wix.tag_name().namespace().unwrap();

    let components: Vec<_> = doc
        .descendants()
        .filter(|n| n.is_element() && n.has_tag_name((ns, "Component")))
        .collect();
    assert_eq!(components.len(), 3);

    let ids: Vec<_> = components
        .iter()
        .map(|n| n.attribute("Id").unwrap().to_string())
        .collect();
    assert!(ids.contains(&"A".to_string()));
    assert!(ids.contains(&"B".to_string()));
    assert!(ids.contains(&"C".to_string()));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_reports_recursion_limit_for_include_cycles() {
    let dir = make_temp_dir("include_cycle");

    let a = dir.join("a.yaml");
    let b = dir.join("b.yaml");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(&a, "!include b.yaml\n").unwrap();
    fs::write(&b, "!include a.yaml\n").unwrap();

    fs::write(
        &main,
        r#"!include a.yaml
Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: 'MyApp'
"#,
    )
    .unwrap();

    let err = yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap_err();

    assert!(err.to_string().contains("Maximum recursion depth"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_appends_sequences_from_includes() {
    let dir = make_temp_dir("include_seq");

    let base = dir.join("base.yaml");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(
        &base,
        r#"Wix:
  Fragment:
    Component:
      - Id: 'A'
"#,
    )
    .unwrap();

    fs::write(
        &main,
        r#"!include base.yaml
Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Fragment:
    Component:
      - Id: 'B'
"#,
    )
    .unwrap();

    yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    let xml = fs::read_to_string(&output).unwrap();
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

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn yaml_to_wix_convert_file_resolves_include_end_to_end() {
    let dir = make_temp_dir("include");

    let base = dir.join("base.yaml");
    let main = dir.join("main.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(
        &base,
        r#"Wix:
  Package:
    Manufacturer: 'BaseManu'
    Version: '1.0.0.0'
"#,
    )
    .unwrap();

    fs::write(
        &main,
        r#"!include base.yaml
Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: 'OverrideName'
"#,
    )
    .unwrap();

    yaml_to_wix::convert_file(
        &main,
        &output,
        AttributeStrategy::Inline,
        &known_attributes(),
    )
    .unwrap();

    let xml = fs::read_to_string(&output).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();

    let wix = doc.descendants().find(|n| n.has_tag_name("Wix")).unwrap();
    let ns = wix.tag_name().namespace().unwrap();

    let package = wix
        .children()
        .find(|n| n.is_element() && n.has_tag_name((ns, "Package")))
        .unwrap();

    assert_eq!(package.attribute("Name").unwrap(), "OverrideName");
    assert_eq!(package.attribute("Manufacturer").unwrap(), "BaseManu");
    assert_eq!(package.attribute("Version").unwrap(), "1.0.0.0");

    let _ = fs::remove_dir_all(&dir);
}
