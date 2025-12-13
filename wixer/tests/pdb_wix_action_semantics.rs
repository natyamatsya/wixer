use serde_json::json;

use wixer::analysis::pdb::wix_action;

fn field(data: serde_json::Value) -> serde_json::Value {
    json!({"data": data})
}

#[test]
fn parse_wix_action_extracts_table_from_id_and_fields() {
    let id = "InstallExecuteSequence/SomeId";

    // fields: [1]=Action, [2]=Condition, [3]=Sequence, [4]=Before, [5]=After
    let mut fields = vec![serde_json::Value::Null; 6];
    fields[1] = field(json!("MyAction"));
    fields[2] = field(json!("NOT Installed"));
    fields[3] = field(json!(1500));
    fields[4] = field(json!("CostFinalize"));
    fields[5] = field(json!("InstallFiles"));

    let data = wix_action::parse_wix_action(id, &fields);

    assert_eq!(data.table, "InstallExecuteSequence");
    assert_eq!(data.action, "MyAction");
    assert_eq!(data.sequence, Some(1500));
    assert_eq!(data.condition.as_deref(), Some("NOT Installed"));
    assert_eq!(data.before.as_deref(), Some("CostFinalize"));
    assert_eq!(data.after.as_deref(), Some("InstallFiles"));
}

#[test]
fn analyze_wix_action_infers_execution_phase() {
    let id = "InstallExecuteSequence/SomeId";

    let mut fields = vec![serde_json::Value::Null; 6];
    fields[1] = field(json!("MyAction"));

    fields[3] = field(json!(-1));
    let report = wix_action::analyze_wix_action(id, &fields);
    assert!(report.contains("Execution Phase: Negative"));

    fields[3] = field(json!(100));
    let report = wix_action::analyze_wix_action(id, &fields);
    assert!(report.contains("Execution Phase: Initialization"));

    fields[3] = field(json!(2000));
    let report = wix_action::analyze_wix_action(id, &fields);
    assert!(report.contains("Execution Phase: File Transfer"));

    fields[3] = field(json!(7000));
    let report = wix_action::analyze_wix_action(id, &fields);
    assert!(report.contains("Execution Phase: Finalization"));
}

#[test]
fn analyze_wix_action_prints_unresolved_when_sequence_missing() {
    let id = "InstallExecuteSequence/SomeId";

    let fields: Vec<serde_json::Value> = vec![serde_json::Value::Null; 6];
    let report = wix_action::analyze_wix_action(id, &fields);

    assert!(report.contains("Scheduled Sequence: <Unresolved>"));
    assert!(report.contains("Condition: <None>"));
}
