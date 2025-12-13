use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use wixer::{yaml_to_wix, AttributeStrategy};

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
    let dir = std::env::temp_dir().join(format!("wixer_test_{}_{}_{}", prefix, std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn yaml_to_wix_convert_file_rejects_xml_content_in_yaml_file() {
    let dir = make_temp_dir("reject_xml");
    let input = dir.join("in.wxs.yaml");
    let output = dir.join("out.wxs");

    fs::write(&input, "<?xml version=\"1.0\"?><Wix />").unwrap();

    let err = yaml_to_wix::convert_file(&input, &output, AttributeStrategy::Inline, &known_attributes())
        .unwrap_err();

    assert!(err.to_string().contains("contains XML content"));

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

    yaml_to_wix::convert_file(&main, &output, AttributeStrategy::Inline, &known_attributes()).unwrap();

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
