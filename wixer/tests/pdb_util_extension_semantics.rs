use serde_json::json;

use wixer::analysis::pdb::util_extension;

fn field(data: serde_json::Value) -> serde_json::Value {
    json!({"data": data})
}

#[test]
fn analyze_close_app_reports_combined_strategy_flags() {
    // [0] Target, [3] Attributes
    let mut fields = vec![serde_json::Value::Null; 8];

    fields[0] = field(json!("MyApp.exe"));

    // Prompt + CloseMessage + ElevatedCloseMessage + TerminateProcess + RebootPrompt
    let attrs = util_extension::CloseAppAttributes::PROMPT_TO_CONTINUE
        | util_extension::CloseAppAttributes::CLOSE_MESSAGE
        | util_extension::CloseAppAttributes::ELEVATED_CLOSE_MESSAGE
        | util_extension::CloseAppAttributes::TERMINATE_PROCESS
        | util_extension::CloseAppAttributes::REBOOT_PROMPT;
    fields[3] = field(json!(attrs));

    let report = util_extension::analyze_close_app(&fields);

    assert!(report.contains("Target Process: MyApp.exe"));
    assert!(report.contains("PROMPT user"));
    assert!(report.contains("WM_CLOSE"));

    // Deferred section should appear because elevated/terminate bits are set
    assert!(report.contains("Schedule DEFERRED action"));
    assert!(report.contains("Retry closing via WM_CLOSE (Elevated)"));
    assert!(report.contains("FORCE KILL"));

    assert!(report.contains("require REBOOT"));
}

#[test]
fn analyze_close_app_is_robust_to_missing_fields() {
    let fields: Vec<serde_json::Value> = vec![];
    let report = util_extension::analyze_close_app(&fields);

    assert!(report.contains("Target Process: ?"));
    assert!(report.contains("Strategy"));
}
