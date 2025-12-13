mod test_support;

use serde_json::json;

use wixer::analysis::pdb::pdb_validation;
use wixer::analysis::pdb::pdb_validation::WarningKind;
use wixer::simulation::{Action, CustomActionType, SequencePosition, Simulator};

#[test]
fn pdb_validation_reports_missing_action_for_non_standard_actions() {
    let sim = test_support::SimBuilder::new()
        .action(test_support::ActionBuilder::dll_binary(
            "MyCustomAction",
            "Entry",
            "Bin",
        ))
        .build();

    let pdb = test_support::PdbBuilder::new().build();

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(
        warnings
            .iter()
            .any(|w| w.kind == WarningKind::MissingAction)
    );
}

#[test]
fn pdb_validation_ignores_missing_standard_actions() {
    let sim = test_support::SimBuilder::new()
        .action(test_support::ActionBuilder::dll_binary(
            "InstallFiles",
            "Entry",
            "Bin",
        ))
        .build();

    let pdb = test_support::PdbBuilder::new().build();

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(
        !warnings
            .iter()
            .any(|w| w.kind == WarningKind::MissingAction)
    );
}

#[test]
fn pdb_validation_reports_extra_action_and_ignores_extension_prefixed_actions() {
    let sim = Simulator::new();

    let pdb = test_support::pdb::pdb_with_symbols(vec![
        test_support::pdb::custom_action_symbol("ExtraCustom", vec![]),
        test_support::pdb::custom_action_symbol("Wix4CloseApplications_X86", vec![]),
    ]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(warnings.iter().any(|w| w.kind == WarningKind::ExtraAction));
    assert!(
        !warnings
            .iter()
            .any(|w| w.message.contains("Wix4CloseApplications_X86"))
    );
}

#[test]
fn pdb_validation_reports_condition_and_scheduling_mismatches() {
    let sim = test_support::SimBuilder::new()
        .action(
            test_support::ActionBuilder::dll_binary("MyCustom", "Entry", "Bin")
                .condition("A AND B")
                .before("InstallFiles"),
        )
        .build();

    let pdb = test_support::PdbBuilder::new()
        .custom_action("MyCustom")
        .condition("A OR B")
        .after_action("InstallFinalize")
        .finish()
        .build();

    let warnings = pdb_validation::validate(&pdb, &sim);

    assert!(
        warnings
            .iter()
            .any(|w| w.kind == WarningKind::ConditionMismatch)
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.kind == WarningKind::SchedulingMismatch)
    );
}

#[test]
fn pdb_validation_returns_no_warnings_when_both_sides_are_empty() {
    let sim = Simulator::new();
    let pdb = test_support::pdb::pdb_with_symbols(vec![]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(warnings.is_empty());
}

#[test]
fn pdb_validation_reports_scheduling_mismatch_for_after() {
    let mut sim = Simulator::new();

    let mut action = Action::new_custom(
        "MyCustomAfter",
        CustomActionType::DllBinary {
            dll_entry: "Entry".into(),
            binary_ref: "Bin".into(),
        },
    );
    action.position = Some(SequencePosition::After("InstallFinalize".into()));
    sim.add_custom_action(action);

    let pdb = test_support::pdb::pdb_with_symbols(vec![test_support::pdb::custom_action_symbol(
        "MyCustomAfter",
        vec![json!({"Before": "InstallFiles"})],
    )]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(
        warnings
            .iter()
            .any(|w| w.kind == WarningKind::SchedulingMismatch)
    );
}

#[test]
fn pdb_validation_does_not_report_scheduling_mismatch_when_after_matches() {
    let mut sim = Simulator::new();

    let mut action = Action::new_custom(
        "MyCustomAfter",
        CustomActionType::DllBinary {
            dll_entry: "Entry".into(),
            binary_ref: "Bin".into(),
        },
    );
    action.position = Some(SequencePosition::After("InstallFinalize".into()));
    sim.add_custom_action(action);

    let pdb = test_support::pdb::pdb_with_symbols(vec![test_support::pdb::custom_action_symbol(
        "MyCustomAfter",
        vec![json!({"After": "InstallFinalize"})],
    )]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(
        !warnings
            .iter()
            .any(|w| w.kind == WarningKind::SchedulingMismatch)
    );
}

#[test]
fn pdb_validation_can_parse_wix_action_symbol_with_string_id() {
    let sim = Simulator::new();

    let pdb = test_support::pdb::pdb_with_symbols(vec![test_support::pdb::wix_action_symbol(
        json!("MyWixAction"),
        vec![
            json!({"SequenceTable": "InstallExecuteSequence"}),
            json!({"Sequence": 123}),
        ],
    )]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(warnings.iter().any(|w| w.kind == WarningKind::ExtraAction));
}

#[test]
fn pdb_validation_ignores_symbols_that_are_not_actions() {
    let sim = Simulator::new();
    let pdb = test_support::pdb::pdb_with_symbols(vec![wixer::wixpdb::Symbol {
        symbol_type: "NotAnAction".to_string(),
        id: json!({"id": "X"}),
        ln: None,
        fields: vec![],
    }]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(warnings.is_empty());
}
