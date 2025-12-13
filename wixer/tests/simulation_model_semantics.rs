use std::collections::HashMap;

use wixer::simulation::{
    ActionResult, ExecutionEvent, ExecutionTrace, InstallMode, PropertyValue, Sequence,
    SequenceTable,
};

#[test]
fn install_mode_default_properties_match_expected_modes() {
    let p = InstallMode::FirstInstall.default_properties();
    assert!(matches!(p.get("Installed"), Some(PropertyValue::Null)));

    let p = InstallMode::Upgrade.default_properties();
    assert!(matches!(
        p.get("WIX_UPGRADE_DETECTED"),
        Some(PropertyValue::String(_))
    ));

    let p = InstallMode::Uninstall.default_properties();
    assert!(matches!(p.get("REMOVE"), Some(PropertyValue::String(_))));

    let p = InstallMode::Repair.default_properties();
    assert!(matches!(p.get("REINSTALL"), Some(PropertyValue::String(_))));
}

#[test]
fn install_mode_default_properties_cover_remaining_modes() {
    let p = InstallMode::Maintenance.default_properties();
    assert!(matches!(p.get("Installed"), Some(PropertyValue::String(_))));
    assert!(matches!(p.get("REMOVE"), Some(PropertyValue::Null)));

    let p = InstallMode::RemovingForUpgrade.default_properties();
    assert!(matches!(p.get("Installed"), Some(PropertyValue::String(_))));
    assert!(matches!(p.get("REMOVE"), Some(PropertyValue::String(_))));
}

#[test]
fn sequence_table_name_covers_all_variants() {
    assert_eq!(SequenceTable::InstallUISequence.name(), "InstallUISequence");
    assert_eq!(
        SequenceTable::InstallExecuteSequence.name(),
        "InstallExecuteSequence"
    );
    assert_eq!(SequenceTable::AdminUISequence.name(), "AdminUISequence");
    assert_eq!(
        SequenceTable::AdminExecuteSequence.name(),
        "AdminExecuteSequence"
    );
    assert_eq!(
        SequenceTable::AdvertiseExecuteSequence.name(),
        "AdvertiseExecuteSequence"
    );
}

#[test]
fn property_value_helpers_work() {
    let v = PropertyValue::Null;
    assert!(!v.is_set());
    assert_eq!(v.as_str(), None);

    let v = PropertyValue::String("x".into());
    assert!(v.is_set());
    assert_eq!(v.as_str(), Some("x"));

    let v = PropertyValue::Integer(1);
    assert!(v.is_set());
    assert_eq!(v.as_str(), None);
}

#[test]
fn execution_trace_counts_success_skipped_failed_and_updates_success_flag() {
    let mut trace = ExecutionTrace::new(InstallMode::FirstInstall);

    trace.add_event(ExecutionEvent {
        sequence_table: SequenceTable::InstallExecuteSequence,
        action_id: "A".into(),
        action_type: wixer::simulation::ActionType::Standard,
        sequence_number: 1,
        condition: None,
        condition_result: true,
        result: ActionResult::Success,
        properties_modified: vec![],
        timestamp_ms: 0,
    });

    trace.add_event(ExecutionEvent {
        sequence_table: SequenceTable::InstallExecuteSequence,
        action_id: "B".into(),
        action_type: wixer::simulation::ActionType::Standard,
        sequence_number: 2,
        condition: Some("0".into()),
        condition_result: false,
        result: ActionResult::Skipped,
        properties_modified: vec![],
        timestamp_ms: 1,
    });

    trace.add_event(ExecutionEvent {
        sequence_table: SequenceTable::InstallExecuteSequence,
        action_id: "C".into(),
        action_type: wixer::simulation::ActionType::Standard,
        sequence_number: 3,
        condition: None,
        condition_result: true,
        result: ActionResult::Failed("oops".into()),
        properties_modified: vec![],
        timestamp_ms: 2,
    });

    assert!(!trace.success);
    assert_eq!(trace.executed_events().count(), 1);
    assert_eq!(trace.skipped_events().count(), 1);

    let s = trace.summary();
    assert!(s.contains("FAILED"));
    assert!(s.contains("1 executed"));
    assert!(s.contains("1 skipped"));
    assert!(s.contains("1 failed"));
}

#[test]
fn sequence_sorted_actions_sorts_by_sequence_number_with_defaulting() {
    let mut seq = Sequence::new(SequenceTable::InstallExecuteSequence);

    let mut a = wixer::simulation::Action::new_standard("A", 10);
    a.sequence_number = Some(10);

    let mut b = wixer::simulation::Action::new_standard("B", 5);
    b.sequence_number = Some(5);

    seq.add_action(a);
    seq.add_action(b);

    let sorted = seq.sorted_actions();
    assert_eq!(sorted[0].id, "B");
    assert_eq!(sorted[1].id, "A");

    let _: HashMap<String, PropertyValue> = HashMap::new();
}
