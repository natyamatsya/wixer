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
