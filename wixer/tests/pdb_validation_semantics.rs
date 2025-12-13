use serde_json::json;

use wixer::analysis::pdb::pdb_validation;
use wixer::analysis::pdb::pdb_validation::WarningKind;
use wixer::simulation::{Action, CustomActionType, SequencePosition, Simulator};
use wixer::wixpdb::{Section, Symbol, WixPdb};

fn symbol(id: &str, fields: Vec<serde_json::Value>) -> Symbol {
    Symbol {
        symbol_type: "CustomAction".to_string(),
        id: json!({"id": id}),
        ln: None,
        fields,
    }
}

fn pdb_with_symbols(symbols: Vec<Symbol>) -> WixPdb {
    WixPdb {
        sections: vec![Section { symbols }],
    }
}

fn wix_action_symbol(id: serde_json::Value, fields: Vec<serde_json::Value>) -> Symbol {
    Symbol {
        symbol_type: "WixAction".to_string(),
        id,
        ln: None,
        fields,
    }
}

#[test]
fn pdb_validation_reports_missing_action_for_non_standard_actions() {
    let mut sim = Simulator::new();
    sim.add_custom_action(Action::new_custom(
        "MyCustomAction",
        CustomActionType::DllBinary {
            dll_entry: "Entry".into(),
            binary_ref: "Bin".into(),
        },
    ));

    let pdb = pdb_with_symbols(vec![]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(
        warnings
            .iter()
            .any(|w| w.kind == WarningKind::MissingAction)
    );
}

#[test]
fn pdb_validation_ignores_missing_standard_actions() {
    let mut sim = Simulator::new();
    sim.add_custom_action(Action::new_custom(
        "InstallFiles",
        CustomActionType::DllBinary {
            dll_entry: "Entry".into(),
            binary_ref: "Bin".into(),
        },
    ));

    let pdb = pdb_with_symbols(vec![]);

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

    let pdb = pdb_with_symbols(vec![
        symbol("ExtraCustom", vec![]),
        symbol("Wix4CloseApplications_X86", vec![]),
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
    let mut sim = Simulator::new();

    let mut action = Action::new_custom(
        "MyCustom",
        CustomActionType::DllBinary {
            dll_entry: "Entry".into(),
            binary_ref: "Bin".into(),
        },
    )
    .with_condition("A AND B");
    action.position = Some(SequencePosition::Before("InstallFiles".into()));

    sim.add_custom_action(action);

    let pdb = pdb_with_symbols(vec![symbol(
        "MyCustom",
        vec![
            json!({"Condition": "A OR B"}),
            json!({"After": "InstallFinalize"}),
        ],
    )]);

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
    let pdb = pdb_with_symbols(vec![]);

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

    let pdb = pdb_with_symbols(vec![symbol(
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

    let pdb = pdb_with_symbols(vec![symbol(
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

    let pdb = pdb_with_symbols(vec![wix_action_symbol(
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
    let pdb = pdb_with_symbols(vec![Symbol {
        symbol_type: "NotAnAction".to_string(),
        id: json!({"id": "X"}),
        ln: None,
        fields: vec![],
    }]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(warnings.is_empty());
}
