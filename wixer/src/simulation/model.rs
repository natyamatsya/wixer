//! Core data model for MSI execution simulation

use std::collections::HashMap;

/// Installation mode determines which actions run based on conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstallMode {
    /// Fresh installation (no previous version)
    FirstInstall,
    /// Upgrading from an older version
    Upgrade,
    /// Being removed as part of an upgrade (old version)
    RemovingForUpgrade,
    /// Complete uninstallation
    Uninstall,
    /// Repair existing installation
    Repair,
    /// Modify features of existing installation
    Maintenance,
}

impl InstallMode {
    /// Get the standard properties set for this install mode
    pub fn default_properties(&self) -> HashMap<String, PropertyValue> {
        let mut props = HashMap::new();

        match self {
            InstallMode::FirstInstall => {
                props.insert("Installed".into(), PropertyValue::Null);
                props.insert("WIX_UPGRADE_DETECTED".into(), PropertyValue::Null);
                props.insert("WIX_DOWNGRADE_DETECTED".into(), PropertyValue::Null);
                props.insert("UPGRADINGPRODUCTCODE".into(), PropertyValue::Null);
                props.insert("REMOVE".into(), PropertyValue::Null);
            }
            InstallMode::Upgrade => {
                props.insert("Installed".into(), PropertyValue::Null);
                props.insert(
                    "WIX_UPGRADE_DETECTED".into(),
                    PropertyValue::String("1".into()),
                );
                props.insert("WIX_DOWNGRADE_DETECTED".into(), PropertyValue::Null);
                props.insert("UPGRADINGPRODUCTCODE".into(), PropertyValue::Null);
                props.insert("REMOVE".into(), PropertyValue::Null);
            }
            InstallMode::RemovingForUpgrade => {
                props.insert("Installed".into(), PropertyValue::String("1".into()));
                props.insert("WIX_UPGRADE_DETECTED".into(), PropertyValue::Null);
                props.insert(
                    "UPGRADINGPRODUCTCODE".into(),
                    PropertyValue::String("{GUID}".into()),
                );
                props.insert("REMOVE".into(), PropertyValue::String("ALL".into()));
            }
            InstallMode::Uninstall => {
                props.insert("Installed".into(), PropertyValue::String("1".into()));
                props.insert("WIX_UPGRADE_DETECTED".into(), PropertyValue::Null);
                props.insert("UPGRADINGPRODUCTCODE".into(), PropertyValue::Null);
                props.insert("REMOVE".into(), PropertyValue::String("ALL".into()));
            }
            InstallMode::Repair => {
                props.insert("Installed".into(), PropertyValue::String("1".into()));
                props.insert("WIX_UPGRADE_DETECTED".into(), PropertyValue::Null);
                props.insert("REINSTALL".into(), PropertyValue::String("ALL".into()));
                props.insert("REMOVE".into(), PropertyValue::Null);
            }
            InstallMode::Maintenance => {
                props.insert("Installed".into(), PropertyValue::String("1".into()));
                props.insert("WIX_UPGRADE_DETECTED".into(), PropertyValue::Null);
                props.insert("REMOVE".into(), PropertyValue::Null);
            }
        }

        props
    }
}

/// Property value in the installer state
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Null,
    String(String),
    Integer(i64),
}

impl PropertyValue {
    pub fn is_set(&self) -> bool {
        !matches!(self, PropertyValue::Null)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropertyValue::String(s) => Some(s),
            _ => None,
        }
    }
}

/// Sequence table type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SequenceTable {
    InstallUISequence,
    #[default]
    InstallExecuteSequence,
    AdminUISequence,
    AdminExecuteSequence,
    AdvertiseExecuteSequence,
}

impl SequenceTable {
    pub fn name(&self) -> &'static str {
        match self {
            SequenceTable::InstallUISequence => "InstallUISequence",
            SequenceTable::InstallExecuteSequence => "InstallExecuteSequence",
            SequenceTable::AdminUISequence => "AdminUISequence",
            SequenceTable::AdminExecuteSequence => "AdminExecuteSequence",
            SequenceTable::AdvertiseExecuteSequence => "AdvertiseExecuteSequence",
        }
    }
}

/// Action type classification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionType {
    /// Built-in Windows Installer action
    Standard,
    /// Custom action defined in the package
    Custom(CustomActionType),
    /// Dialog action (UI only)
    Dialog,
    /// Nested action sequence
    NestedSequence,
}

/// Custom action types (Type column in CustomAction table)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomActionType {
    /// Type 1: DLL in Binary table
    DllBinary {
        dll_entry: String,
        binary_ref: String,
    },
    /// Type 2: EXE in Binary table
    ExeBinary {
        binary_ref: String,
        command_line: String,
    },
    /// Type 5: JScript in Binary table
    JScriptBinary {
        binary_ref: String,
        function: Option<String>,
    },
    /// Type 6: VBScript in Binary table
    VBScriptBinary {
        binary_ref: String,
        function: Option<String>,
    },
    /// Type 17: DLL in installed file
    DllFile { dll_entry: String, file_ref: String },
    /// Type 18: EXE in installed file
    ExeFile {
        file_ref: String,
        command_line: String,
    },
    /// Type 19: Display error and abort
    Error { message: String },
    /// Type 34: EXE in directory
    ExeDirectory {
        directory: String,
        command_line: String,
    },
    /// Type 35: Set directory
    SetDirectory { directory: String, value: String },
    /// Type 37: JScript inline
    JScriptInline { code: String },
    /// Type 38: VBScript inline
    VBScriptInline { code: String },
    /// Type 50: EXE with property
    ExeProperty {
        property: String,
        command_line: String,
    },
    /// Type 51: Set property
    SetProperty { property: String, value: String },
    /// Type 53: JScript in property
    JScriptProperty { property: String },
    /// Type 54: VBScript in property
    VBScriptProperty { property: String },
}

/// Execution options for custom actions
#[derive(Debug, Clone, Default)]
pub struct ExecutionOptions {
    /// Deferred execution (runs during script execution)
    pub deferred: bool,
    /// Rollback action (runs on failure)
    pub rollback: bool,
    /// Commit action (runs on success)
    pub commit: bool,
    /// Run with elevated privileges
    pub no_impersonate: bool,
    /// Run asynchronously
    pub r#async: bool,
    /// Continue on error
    pub continue_on_error: bool,
}

/// An action in a sequence
#[derive(Debug, Clone)]
pub struct Action {
    /// Action identifier
    pub id: String,
    /// Action type
    pub action_type: ActionType,
    /// Sequence number (determines order)
    pub sequence_number: Option<i32>,
    /// Condition expression (action runs if true or empty)
    pub condition: Option<String>,
    /// Execution options
    pub execution: ExecutionOptions,
    /// Description for logging
    pub description: Option<String>,
}

impl Action {
    pub fn new_standard(id: impl Into<String>, sequence: i32) -> Self {
        Self {
            id: id.into(),
            action_type: ActionType::Standard,
            sequence_number: Some(sequence),
            condition: None,
            execution: ExecutionOptions::default(),
            description: None,
        }
    }

    pub fn new_custom(id: impl Into<String>, ca_type: CustomActionType) -> Self {
        Self {
            id: id.into(),
            action_type: ActionType::Custom(ca_type),
            sequence_number: None,
            condition: None,
            execution: ExecutionOptions::default(),
            description: None,
        }
    }

    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }

    pub fn with_sequence(mut self, seq: i32) -> Self {
        self.sequence_number = Some(seq);
        self
    }

    pub fn deferred(mut self) -> Self {
        self.execution.deferred = true;
        self
    }
}

/// A complete sequence table
#[derive(Debug, Clone, Default)]
pub struct Sequence {
    pub table: SequenceTable,
    pub actions: Vec<Action>,
}

impl Sequence {
    pub fn new(table: SequenceTable) -> Self {
        Self {
            table,
            actions: Vec::new(),
        }
    }

    pub fn add_action(&mut self, action: Action) {
        self.actions.push(action);
    }

    /// Get actions sorted by sequence number
    pub fn sorted_actions(&self) -> Vec<&Action> {
        let mut actions: Vec<_> = self.actions.iter().collect();
        actions.sort_by_key(|a| a.sequence_number.unwrap_or(0));
        actions
    }
}

/// Result of executing an action
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionResult {
    /// Action executed successfully
    Success,
    /// Action was skipped (condition false)
    Skipped,
    /// Action failed
    Failed(String),
    /// Action not applicable (e.g., UI action in silent mode)
    NotApplicable,
}

/// Event in the execution trace
#[derive(Debug, Clone)]
pub struct ExecutionEvent {
    /// Sequence table this event belongs to
    pub sequence_table: SequenceTable,
    /// Action that was executed
    pub action_id: String,
    /// Action type
    pub action_type: ActionType,
    /// Sequence number
    pub sequence_number: i32,
    /// Condition that was evaluated
    pub condition: Option<String>,
    /// Result of condition evaluation
    pub condition_result: bool,
    /// Result of action execution
    pub result: ActionResult,
    /// Properties modified by this action
    pub properties_modified: Vec<(String, PropertyValue)>,
    /// Timestamp (relative to simulation start)
    pub timestamp_ms: u64,
}

/// Complete execution trace
#[derive(Debug, Clone, Default)]
pub struct ExecutionTrace {
    pub install_mode: Option<InstallMode>,
    pub events: Vec<ExecutionEvent>,
    pub final_properties: HashMap<String, PropertyValue>,
    pub success: bool,
    pub error_message: Option<String>,
}

impl ExecutionTrace {
    pub fn new(mode: InstallMode) -> Self {
        Self {
            install_mode: Some(mode),
            events: Vec::new(),
            final_properties: HashMap::new(),
            success: true,
            error_message: None,
        }
    }

    pub fn add_event(&mut self, event: ExecutionEvent) {
        if matches!(event.result, ActionResult::Failed(_)) {
            self.success = false;
        }
        self.events.push(event);
    }

    /// Get events that actually executed (not skipped)
    pub fn executed_events(&self) -> impl Iterator<Item = &ExecutionEvent> {
        self.events
            .iter()
            .filter(|e| matches!(e.result, ActionResult::Success))
    }

    /// Get events that were skipped
    pub fn skipped_events(&self) -> impl Iterator<Item = &ExecutionEvent> {
        self.events
            .iter()
            .filter(|e| matches!(e.result, ActionResult::Skipped))
    }

    /// Print a summary of the execution
    pub fn summary(&self) -> String {
        let executed = self.executed_events().count();
        let skipped = self.skipped_events().count();
        let failed = self
            .events
            .iter()
            .filter(|e| matches!(e.result, ActionResult::Failed(_)))
            .count();

        format!(
            "Execution: {} ({} executed, {} skipped, {} failed)",
            if self.success { "SUCCESS" } else { "FAILED" },
            executed,
            skipped,
            failed
        )
    }
}
