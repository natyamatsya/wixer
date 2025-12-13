use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use wixer::{AttributeStrategy, xsd_to_json};

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

fn read_json(path: &Path) -> Value {
    let s = fs::read_to_string(path).unwrap();
    serde_json::from_str(&s).unwrap()
}

#[test]
fn xsd_to_json_converts_local_xsd_and_uses_cache_on_second_run() {
    let dir = make_temp_dir("xsd_to_json_cache");
    let out_dir = dir.join("out");
    fs::create_dir_all(&out_dir).unwrap();

    let xsd_path = dir.join("sample.xsd");

    let xsd = r#"<?xml version="1.0" encoding="utf-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
  <xs:simpleType name="ScopeType">
    <xs:restriction base="xs:string">
      <xs:enumeration value="perMachine"/>
      <xs:enumeration value="perUser"/>
    </xs:restriction>
  </xs:simpleType>

  <xs:element name="Name" type="xs:string"/>
  <xs:element name="Feature" type="xs:string"/>

  <xs:complexType name="PackageType">
    <xs:annotation>
      <xs:documentation>Package doc</xs:documentation>
    </xs:annotation>

    <xs:sequence>
      <xs:element ref="Name" minOccurs="1" maxOccurs="1"/>
      <xs:element ref="Feature" minOccurs="0" maxOccurs="unbounded"/>
      <xs:any minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>

    <xs:anyAttribute/>

    <xs:attribute name="Id" type="xs:string" use="required">
      <xs:annotation>
        <xs:documentation>Id doc</xs:documentation>
      </xs:annotation>
    </xs:attribute>

    <xs:attribute name="Scope" type="ScopeType"/>
  </xs:complexType>

  <xs:element name="Package" type="PackageType"/>
</xs:schema>
"#;

    fs::write(&xsd_path, xsd).unwrap();

    let attrs1 = xsd_to_json::convert_xsd(
        xsd_path.to_str().unwrap(),
        None,
        &out_dir,
        AttributeStrategy::Prefix,
        true,
        true,
        None,
    )
    .unwrap();

    assert!(attrs1.contains("Id"));
    assert!(attrs1.contains("Scope"));

    let schema_path = out_dir.join("sample.schema.json");
    let hash_path = out_dir.join("sample.schema.sha256");
    let attributes_path = out_dir.join("sample.schema.attributes.json");

    assert!(schema_path.exists());
    assert!(hash_path.exists());
    assert!(attributes_path.exists());

    let schema = read_json(&schema_path);
    let defs = schema.get("definitions").unwrap().as_object().unwrap();

    let package_type = defs.get("PackageType").unwrap();
    assert_eq!(
        package_type.get("description").unwrap().as_str().unwrap(),
        "Package doc"
    );

    let pkg_props = package_type.get("properties").unwrap().as_object().unwrap();
    assert!(pkg_props.contains_key("@Id"));
    assert!(pkg_props.contains_key("@Scope"));
    assert!(pkg_props.contains_key("Name"));
    assert!(pkg_props.contains_key("Feature"));

    assert!(
        package_type
            .get("additionalProperties")
            .unwrap()
            .as_bool()
            .unwrap()
    );

    // Required should include the required attribute and required element (Name)
    let required = package_type.get("required").unwrap().as_array().unwrap();
    let required_strs: Vec<_> = required
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(required_strs.contains(&"@Id".to_string()));
    assert!(required_strs.contains(&"Name".to_string()));

    // Ensure the attribute annotation was captured
    let id_schema = pkg_props.get("@Id").unwrap();
    assert_eq!(
        id_schema.get("description").unwrap().as_str().unwrap(),
        "Id doc"
    );

    // Ensure array-ish schema for Feature (maxOccurs=unbounded)
    let feature_schema = pkg_props.get("Feature").unwrap();
    assert!(feature_schema.get("oneOf").is_some());

    // Second run should hit the cache branch and return the same attributes
    let attrs2 = xsd_to_json::convert_xsd(
        xsd_path.to_str().unwrap(),
        None,
        &out_dir,
        AttributeStrategy::Prefix,
        true,
        true,
        None,
    )
    .unwrap();

    assert_eq!(attrs1, attrs2);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn xsd_to_json_can_emit_root_ref_schema_when_root_element_is_specified() {
    let dir = make_temp_dir("xsd_to_json_root");
    let out_dir = dir.join("out");
    fs::create_dir_all(&out_dir).unwrap();

    let xsd_path = dir.join("sample.xsd");

    let xsd = r#"<?xml version="1.0" encoding="utf-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
  <xs:complexType name="PackageType">
    <xs:sequence>
      <xs:element name="Name" type="xs:string"/>
    </xs:sequence>
  </xs:complexType>
  <xs:element name="Package" type="PackageType"/>
</xs:schema>
"#;

    fs::write(&xsd_path, xsd).unwrap();

    let out_path = out_dir.join("rooted.schema.json");

    xsd_to_json::convert_xsd(
        xsd_path.to_str().unwrap(),
        Some(out_path.to_str().unwrap()),
        &out_dir,
        AttributeStrategy::Inline,
        false,
        false,
        Some("Package"),
    )
    .unwrap();

    let schema = read_json(&out_path);
    assert!(schema.get("$ref").is_some());

    let _ = fs::remove_dir_all(&dir);
}
