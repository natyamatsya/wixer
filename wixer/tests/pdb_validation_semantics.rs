use serde_json::json;

use wixer::analysis::pdb::pdb_validation;
use wixer::analysis::pdb::pdb_validation::WarningKind;
use wixer::simulation::{Action, CustomActionType, SequencePosition, Simulator};
use wixer::wixpdb::{Section, Symbol, WixPdb};

fn with_debug_tracing<T>(f: impl FnOnce() -> T) -> T {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    let dispatch = tracing::dispatcher::Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, f)
}

fn pdb_with_symbols(symbols: Vec<Symbol>) -> WixPdb {
    WixPdb {
        sections: vec![Section { symbols }],
    }
}

fn custom_action_symbol(id: &str, fields: Vec<serde_json::Value>) -> Symbol {
    Symbol {
        symbol_type: "CustomAction".to_string(),
        id: json!({"id": id}),
        ln: None,
        fields,
    }
}

struct SimBuilder {
    sim: Simulator,
}

impl SimBuilder {
    fn new() -> Self {
        Self {
            sim: Simulator::new(),
        }
    }

    fn custom_action(mut self, action: Action) -> Self {
        self.sim.add_custom_action(action);
        self
    }

    fn build(self) -> Simulator {
        self.sim
    }
}

fn dll_binary_action(id: &str, dll_entry: &str, binary_ref: &str) -> Action {
    Action::new_custom(
        id,
        CustomActionType::DllBinary {
            dll_entry: dll_entry.into(),
            binary_ref: binary_ref.into(),
        },
    )
}

struct PdbBuilder {
    symbols: Vec<Symbol>,
}

impl PdbBuilder {
    fn new() -> Self {
        Self { symbols: vec![] }
    }

    fn custom_action(self, id: &str) -> PdbSymbolBuilder {
        let symbol = custom_action_symbol(id, vec![]);
        PdbSymbolBuilder { pdb: self, symbol }
    }

    fn build(self) -> WixPdb {
        pdb_with_symbols(self.symbols)
    }
}

struct PdbSymbolBuilder {
    pdb: PdbBuilder,
    symbol: Symbol,
}

impl PdbSymbolBuilder {
    fn condition(mut self, condition: &str) -> Self {
        self.symbol.fields.push(json!({"Condition": condition}));
        self
    }

    fn after_action(mut self, after: &str) -> Self {
        self.symbol.fields.push(json!({"After": after}));
        self
    }

    fn finish(mut self) -> PdbBuilder {
        self.pdb.symbols.push(self.symbol);
        self.pdb
    }
}

#[test]
fn pdb_validation_reports_missing_action_for_non_standard_actions() {
    let sim = SimBuilder::new()
        .custom_action(dll_binary_action("MyCustomAction", "Entry", "Bin"))
        .build();

    let pdb = PdbBuilder::new().build();

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(
        warnings
            .iter()
            .any(|w| w.kind == WarningKind::MissingAction)
    );
}

#[test]
fn pdb_validation_ignores_missing_standard_actions() {
    let sim = SimBuilder::new()
        .custom_action(dll_binary_action("InstallFiles", "Entry", "Bin"))
        .build();

    let pdb = PdbBuilder::new().build();

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
        custom_action_symbol("ExtraCustom", vec![]),
        custom_action_symbol("Wix4CloseApplications_X86", vec![]),
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
    let mut action = dll_binary_action("MyCustom", "Entry", "Bin");
    action.condition = Some("A AND B".into());
    action.position = Some(SequencePosition::Before("InstallFiles".into()));

    let sim = SimBuilder::new().custom_action(action).build();

    let pdb = PdbBuilder::new()
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
    let pdb = pdb_with_symbols(vec![]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(warnings.is_empty());
}

#[test]
fn pdb_validation_warning_kind_display_is_stable() {
    assert_eq!(format!("{}", WarningKind::MissingAction), "Missing Action");
    assert_eq!(format!("{}", WarningKind::ExtraAction), "Extra Action");
    assert_eq!(
        format!("{}", WarningKind::SchedulingMismatch),
        "Scheduling Mismatch"
    );
    assert_eq!(
        format!("{}", WarningKind::ConditionMismatch),
        "Condition Mismatch"
    );
    assert_eq!(
        format!("{}", WarningKind::WrongSequenceTable),
        "Wrong Sequence Table"
    );
    assert_eq!(format!("{}", WarningKind::TypeMismatch), "Type Mismatch");
}

#[test]
fn pdb_validation_emits_debug_and_info_logs_when_tracing_is_enabled() {
    let sim = SimBuilder::new()
        .custom_action(dll_binary_action("MyCustomAction", "Entry", "Bin"))
        .build();

    let pdb = pdb_with_symbols(vec![custom_action_symbol("ExtraCustom", vec![])]);

    with_debug_tracing(|| {
        let warnings = pdb_validation::validate(&pdb, &sim);
        assert!(!warnings.is_empty());
    });
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

    let pdb = pdb_with_symbols(vec![custom_action_symbol(
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

    let pdb = pdb_with_symbols(vec![custom_action_symbol(
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

    let pdb = pdb_with_symbols(vec![Symbol {
        symbol_type: "WixAction".to_string(),
        id: json!("MyWixAction"),
        ln: None,
        fields: vec![
            json!({"SequenceTable": "InstallExecuteSequence"}),
            json!({"Sequence": 123}),
        ],
    }]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(warnings.iter().any(|w| w.kind == WarningKind::ExtraAction));
}

#[test]
fn pdb_validation_ignores_symbols_that_are_not_actions() {
    let sim = Simulator::new();
    let pdb = pdb_with_symbols(vec![wixer::wixpdb::Symbol {
        symbol_type: "NotAnAction".to_string(),
        id: json!({"id": "X"}),
        ln: None,
        fields: vec![],
    }]);

    let warnings = pdb_validation::validate(&pdb, &sim);
    assert!(warnings.is_empty());
}
