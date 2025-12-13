use serde_json::json;

use wixer::analysis::pdb::custom_action;

fn field(data: serde_json::Value) -> serde_json::Value {
    json!({"data": data})
}

#[test]
fn analyze_launch_app_reports_expected_flags_for_deferred_exe_custom_action() {
    let mut fields = vec![serde_json::Value::Null; 13];

    fields[0] = field(json!(1280));
    fields[2] = field(json!(16));
    fields[4] = field(json!(2));
    fields[5] = field(json!(true));
    fields[6] = field(json!(true));
    fields[7] = field(json!(true));
    fields[8] = field(json!(false));
    fields[9] = field(json!(true));
    fields[11] = field(json!(true));

    let s = custom_action::analyze_launch_app(&fields);

    assert!(s.contains("Calculated MSI Type:"));
    assert!(s.contains("Behavior:"));

    assert!(s.contains("EXE"));
    assert!(s.contains("Source:File"));
    assert!(s.contains("Async"));
    assert!(s.contains("IgnoreResult"));
    assert!(s.contains("Deferred"));
    assert!(s.contains("NoImpersonate"));
    assert!(s.contains("Rollback"));
}

#[test]
fn analyze_launch_app_is_robust_to_missing_fields() {
    let fields: Vec<serde_json::Value> = vec![];
    let s = custom_action::analyze_launch_app(&fields);

    assert!(s.contains("Calculated MSI Type:"));
    assert!(s.contains("Behavior:"));
}

#[test]
fn analyze_launch_app_reports_commit_script_property_jscript() {
    let mut fields = vec![serde_json::Value::Null; 13];

    fields[0] = field(json!(1536));
    fields[2] = field(json!(48));
    fields[4] = field(json!(5));
    fields[5] = field(json!(false));
    fields[6] = field(json!(false));
    fields[7] = field(json!(false));
    fields[8] = field(json!(true));
    fields[11] = field(json!(false));
    fields[9] = field(json!(false));

    let s = custom_action::analyze_launch_app(&fields);

    assert!(s.contains("JScript"));
    assert!(s.contains("Source:Property"));
    assert!(s.contains("Deferred"));
    assert!(s.contains("Commit"));
    assert!(!s.contains("NoImpersonate"));
}

#[test]
fn analyze_launch_app_reports_directory_dll_immediate() {
    let mut fields = vec![serde_json::Value::Null; 13];

    fields[0] = field(json!(0));
    fields[2] = field(json!(32));
    fields[4] = field(json!(1));
    fields[8] = field(json!(true));

    let s = custom_action::analyze_launch_app(&fields);

    assert!(s.contains("DLL"));
    assert!(s.contains("Source:Directory"));
    assert!(!s.contains("Deferred"));
}
