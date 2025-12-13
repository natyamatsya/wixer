use serde_yaml::Value;

use wixer::analysis::yaml::scheduling_warnings;

#[test]
fn scheduling_warnings_handles_sequences_and_nested_sequence_entries() {
    let yaml: Value = serde_yaml::from_str(
        r#"
Wix:
  Package:
    CustomAction:
      - Id: CA_Deferred
        Execute: DEFERRED
        Directory: INSTALLFOLDER
        ExeCommand: echo test
      - Id: CA_Immediate
        Execute: immediate
        Directory: INSTALLFOLDER
        ExeCommand: echo test
    InstallExecuteSequence:
      SomeContainer:
        Custom:
          - Action: CA_Deferred
            After: InstallFinalize
          - Action: CA_Immediate
            After: InstallFinalize
"#,
    )
    .unwrap();

    let warnings = scheduling_warnings::find_deferred_after_finalize(&yaml);
    assert_eq!(warnings.len(), 1);
    assert!(warnings.iter().any(|w| w.action_id == "CA_Deferred"));
}

#[test]
fn scheduling_warnings_returns_empty_when_no_deferred_actions_exist() {
    let yaml: Value = serde_yaml::from_str(
        r#"
Wix:
  Package:
    CustomAction:
      Id: CA_Immediate
      Execute: immediate
    InstallExecuteSequence:
      Custom:
        Action: CA_Immediate
        After: InstallFinalize
"#,
    )
    .unwrap();

    let warnings = scheduling_warnings::find_deferred_after_finalize(&yaml);
    assert!(warnings.is_empty());
}

#[test]
fn scheduling_warnings_ignores_custom_entries_without_after_install_finalize() {
    let yaml: Value = serde_yaml::from_str(
        r#"
Wix:
  Package:
    CustomAction:
      Id: CA_Deferred
      Execute: deferred
    InstallExecuteSequence:
      Custom:
        - Action: CA_Deferred
          Before: InstallFinalize
        - Action: CA_Deferred
          After: InstallFiles
"#,
    )
    .unwrap();

    let warnings = scheduling_warnings::find_deferred_after_finalize(&yaml);
    assert!(warnings.is_empty());
}
