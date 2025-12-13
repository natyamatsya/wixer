use serde_yaml::Value;

use wixer::analysis::yaml::condition_validation;

#[test]
fn condition_validation_ignores_likely_valid_properties() {
    let yaml: Value = serde_yaml::from_str(
        r#"
Wix:
  Package:
    InstallExecuteSequence:
      Custom:
        - Action: CA_Test
          Before: InstallFinalize
          Condition: _INTERNAL AND ABC AND WIXUI_SOMETHING
"#,
    )
    .unwrap();

    let warnings = condition_validation::find_undefined_properties(&yaml);
    assert!(warnings.is_empty());
}

#[test]
fn condition_validation_skips_numbers_and_quoted_strings_in_property_extraction() {
    let yaml: Value = serde_yaml::from_str(
        r#"
Wix:
  Package:
    InstallExecuteSequence:
      Custom:
        - Action: CA_Test
          Before: InstallFinalize
          Condition: 1 AND "Installed" AND 'X'
"#,
    )
    .unwrap();

    let warnings = condition_validation::find_undefined_properties(&yaml);
    assert!(warnings.is_empty());
}

#[test]
fn condition_validation_suggests_case_correct_property_name() {
    let yaml: Value = serde_yaml::from_str(
        r#"
Wix:
  Package:
    Property:
      Id: MyProp
      Value: "1"
    InstallExecuteSequence:
      Custom:
        Action: CA_Test
        Before: InstallFinalize
        Condition: myprop
"#,
    )
    .unwrap();

    let warnings = condition_validation::find_undefined_properties(&yaml);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].suggestion.as_ref().unwrap().contains("MyProp"));
}
