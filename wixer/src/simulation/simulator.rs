//! MSI Installer execution simulator

use super::condition::ConditionEvaluator;
use super::model::*;
use super::standard_actions::{default_execute_sequence, is_standard_action};
use std::collections::HashMap;

/// Simulation options
#[derive(Debug, Clone)]
pub struct SimulationOptions {
    /// Include UI sequence (false = silent install)
    pub include_ui: bool,
    /// Stop on first error
    pub stop_on_error: bool,
    /// Verbose logging
    pub verbose: bool,
    /// Custom property overrides
    pub property_overrides: HashMap<String, String>,
}

impl Default for SimulationOptions {
    fn default() -> Self {
        Self {
            include_ui: false,
            stop_on_error: true,
            verbose: false,
            property_overrides: HashMap::new(),
        }
    }
}

/// MSI Installer execution simulator
#[derive(Debug)]
pub struct Simulator {
    /// Custom actions defined in the package
    custom_actions: HashMap<String, Action>,
    /// InstallExecuteSequence
    execute_sequence: Sequence,
    /// InstallUISequence
    ui_sequence: Sequence,
    /// Properties defined in the package
    properties: HashMap<String, PropertyValue>,
    /// SetProperty actions (Type 51)
    set_property_actions: Vec<SetPropertyAction>,
}

/// SetProperty action definition
#[derive(Debug, Clone)]
struct SetPropertyAction {
    id: String,
    property: String,
    value: String,
    condition: Option<String>,
    sequence: SequenceTable,
    sequence_number: i32,
}

impl Default for Simulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Simulator {
    /// Create a new simulator with default sequences
    pub fn new() -> Self {
        Self {
            custom_actions: HashMap::new(),
            execute_sequence: default_execute_sequence(),
            ui_sequence: Sequence::new(SequenceTable::InstallUISequence),
            properties: HashMap::new(),
            set_property_actions: Vec::new(),
        }
    }

    /// Add a custom action
    pub fn add_custom_action(&mut self, action: Action) {
        self.custom_actions.insert(action.id.clone(), action);
    }

    /// Add a SetProperty action
    pub fn add_set_property(
        &mut self,
        id: impl Into<String>,
        property: impl Into<String>,
        value: impl Into<String>,
        condition: Option<String>,
        sequence: SequenceTable,
        sequence_number: i32,
    ) {
        self.set_property_actions.push(SetPropertyAction {
            id: id.into(),
            property: property.into(),
            value: value.into(),
            condition,
            sequence,
            sequence_number,
        });
    }

    /// Add an action to the execute sequence
    pub fn add_to_execute_sequence(&mut self, action: Action) {
        self.execute_sequence.add_action(action);
    }

    /// Add an action to the UI sequence
    pub fn add_to_ui_sequence(&mut self, action: Action) {
        self.ui_sequence.add_action(action);
    }

    /// Set initial property value
    pub fn set_property(&mut self, name: impl Into<String>, value: PropertyValue) {
        self.properties.insert(name.into(), value);
    }

    /// Run the simulation
    pub fn run(&self, mode: InstallMode, options: &SimulationOptions) -> ExecutionTrace {
        let mut trace = ExecutionTrace::new(mode);
        let mut props = mode.default_properties();

        // Apply package-defined properties
        for (k, v) in &self.properties {
            props.insert(k.clone(), v.clone());
        }

        // Apply command-line overrides
        for (k, v) in &options.property_overrides {
            props.insert(k.clone(), PropertyValue::String(v.clone()));
        }

        let mut timestamp = 0u64;

        // Run UI sequence if enabled
        if options.include_ui {
            self.run_sequence(
                &self.ui_sequence,
                &mut props,
                &mut trace,
                &mut timestamp,
                options,
            );

            if !trace.success && options.stop_on_error {
                return trace;
            }
        }

        // Run Execute sequence
        self.run_sequence(
            &self.execute_sequence,
            &mut props,
            &mut trace,
            &mut timestamp,
            options,
        );

        trace.final_properties = props;
        trace
    }

    fn run_sequence(
        &self,
        sequence: &Sequence,
        props: &mut HashMap<String, PropertyValue>,
        trace: &mut ExecutionTrace,
        timestamp: &mut u64,
        options: &SimulationOptions,
    ) {
        // Collect all actions including SetProperty actions for this sequence
        let mut all_actions: Vec<(i32, ActionEntry)> = Vec::new();

        // Add standard/custom actions from sequence
        for action in &sequence.actions {
            if let Some(seq) = action.sequence_number {
                all_actions.push((seq, ActionEntry::Action(action)));
            }
        }

        // Add SetProperty actions for this sequence
        for spa in &self.set_property_actions {
            if spa.sequence == sequence.table {
                all_actions.push((spa.sequence_number, ActionEntry::SetProperty(spa)));
            }
        }

        // Sort by sequence number
        all_actions.sort_by_key(|(seq, _)| *seq);

        // Execute in order
        for (seq_num, entry) in all_actions {
            let event = match entry {
                ActionEntry::Action(action) => {
                    self.execute_action(action, seq_num, sequence.table, props, *timestamp)
                }
                ActionEntry::SetProperty(spa) => self.execute_set_property(spa, props, *timestamp),
            };

            *timestamp += 10; // Simulate time passing

            let failed = matches!(event.result, ActionResult::Failed(_));
            trace.add_event(event);

            if failed && options.stop_on_error {
                break;
            }
        }
    }

    fn execute_action(
        &self,
        action: &Action,
        seq_num: i32,
        table: SequenceTable,
        props: &mut HashMap<String, PropertyValue>,
        timestamp: u64,
    ) -> ExecutionEvent {
        // Evaluate condition
        let condition_result = action
            .condition
            .as_ref()
            .map(|c| ConditionEvaluator::evaluate(c, props).is_true())
            .unwrap_or(true);

        let result = if !condition_result {
            ActionResult::Skipped
        } else {
            // Simulate action execution
            self.simulate_action(action, props)
        };

        ExecutionEvent {
            sequence_table: table,
            action_id: action.id.clone(),
            action_type: action.action_type.clone(),
            sequence_number: seq_num,
            condition: action.condition.clone(),
            condition_result,
            result,
            properties_modified: Vec::new(),
            timestamp_ms: timestamp,
        }
    }

    fn execute_set_property(
        &self,
        spa: &SetPropertyAction,
        props: &mut HashMap<String, PropertyValue>,
        timestamp: u64,
    ) -> ExecutionEvent {
        // Evaluate condition
        let condition_result = spa
            .condition
            .as_ref()
            .map(|c| ConditionEvaluator::evaluate(c, props).is_true())
            .unwrap_or(true);

        let mut properties_modified = Vec::new();

        let result = if !condition_result {
            ActionResult::Skipped
        } else {
            // Set the property
            let expanded_value = self.expand_properties(&spa.value, props);
            let new_value = PropertyValue::String(expanded_value);
            props.insert(spa.property.clone(), new_value.clone());
            properties_modified.push((spa.property.clone(), new_value));
            ActionResult::Success
        };

        ExecutionEvent {
            sequence_table: spa.sequence,
            action_id: spa.id.clone(),
            action_type: ActionType::Custom(CustomActionType::SetProperty {
                property: spa.property.clone(),
                value: spa.value.clone(),
            }),
            sequence_number: spa.sequence_number,
            condition: spa.condition.clone(),
            condition_result,
            result,
            properties_modified,
            timestamp_ms: timestamp,
        }
    }

    fn simulate_action(
        &self,
        action: &Action,
        _props: &mut HashMap<String, PropertyValue>,
    ) -> ActionResult {
        // For standard actions, just return success (simulation)
        if is_standard_action(&action.id) {
            return ActionResult::Success;
        }

        // For custom actions, simulate based on type
        match &action.action_type {
            ActionType::Standard => ActionResult::Success,
            ActionType::Custom(ca_type) => match ca_type {
                CustomActionType::SetProperty { .. } => ActionResult::Success,
                CustomActionType::Error { message } => ActionResult::Failed(message.clone()),
                _ => {
                    // Simulate execution (in real scenario, we can't run DLLs/EXEs)
                    ActionResult::Success
                }
            },
            ActionType::Dialog => ActionResult::NotApplicable,
            ActionType::NestedSequence => ActionResult::Success,
        }
    }

    fn expand_properties(&self, value: &str, props: &HashMap<String, PropertyValue>) -> String {
        let mut result = value.to_string();

        // Simple property expansion: [PropertyName]
        for (name, val) in props {
            let pattern = format!("[{}]", name);
            let replacement = match val {
                PropertyValue::String(s) => s.clone(),
                PropertyValue::Integer(n) => n.to_string(),
                PropertyValue::Null => String::new(),
            };
            result = result.replace(&pattern, &replacement);
        }

        result
    }
}

enum ActionEntry<'a> {
    Action(&'a Action),
    SetProperty(&'a SetPropertyAction),
}

/// Builder for creating simulators from parsed WiX data
pub struct SimulatorBuilder {
    simulator: Simulator,
}

impl Default for SimulatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulatorBuilder {
    pub fn new() -> Self {
        Self {
            simulator: Simulator::new(),
        }
    }

    /// Add installation stage properties (FirstInstall, Upgrading, etc.)
    pub fn with_installation_stages(mut self) -> Self {
        // FirstInstall
        self.simulator.add_set_property(
            "SetFirstInstall",
            "FirstInstall",
            "true",
            Some(
                "NOT Installed AND NOT WIX_UPGRADE_DETECTED AND NOT WIX_DOWNGRADE_DETECTED".into(),
            ),
            SequenceTable::InstallExecuteSequence,
            30, // After FindRelatedProducts (25)
        );

        // Upgrading
        self.simulator.add_set_property(
            "SetUpgrading",
            "Upgrading",
            "true",
            Some("WIX_UPGRADE_DETECTED AND NOT (REMOVE=\"ALL\")".into()),
            SequenceTable::InstallExecuteSequence,
            31,
        );

        // RemovingForUpgrade
        self.simulator.add_set_property(
            "SetRemovingForUpgrade",
            "RemovingForUpgrade",
            "true",
            Some("(REMOVE=\"ALL\") AND UPGRADINGPRODUCTCODE".into()),
            SequenceTable::InstallExecuteSequence,
            6710, // After RemoveExistingProducts
        );

        // Uninstalling
        self.simulator.add_set_property(
            "SetUninstalling",
            "Uninstalling",
            "true",
            Some("Installed AND (REMOVE=\"ALL\") AND NOT (WIX_UPGRADE_DETECTED OR UPGRADINGPRODUCTCODE)".into()),
            SequenceTable::InstallExecuteSequence,
            32,
        );

        // Maintenance
        self.simulator.add_set_property(
            "SetMaintenance",
            "Maintenance",
            "true",
            Some(
                "Installed AND NOT Upgrading AND NOT Uninstalling AND NOT UPGRADINGPRODUCTCODE"
                    .into(),
            ),
            SequenceTable::InstallExecuteSequence,
            33,
        );

        self
    }

    /// Add a custom action to the execute sequence
    pub fn with_custom_action(
        mut self,
        id: impl Into<String>,
        condition: Option<String>,
        sequence_number: i32,
    ) -> Self {
        let mut action = Action::new_custom(
            id,
            CustomActionType::DllBinary {
                dll_entry: "CustomEntry".into(),
                binary_ref: "CustomBinary".into(),
            },
        );
        action.sequence_number = Some(sequence_number);
        action.condition = condition;
        self.simulator.add_to_execute_sequence(action);
        self
    }

    /// Build the simulator
    pub fn build(self) -> Simulator {
        self.simulator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_install_simulation() {
        let sim = SimulatorBuilder::new()
            .with_installation_stages()
            .with_custom_action("MyCustomAction", Some("FirstInstall".into()), 6500)
            .build();

        let trace = sim.run(InstallMode::FirstInstall, &SimulationOptions::default());

        assert!(trace.success);

        // Check that FirstInstall property was set
        let first_install = trace.final_properties.get("FirstInstall");
        assert!(first_install.is_some());
        assert!(first_install.unwrap().is_set());

        // Check that custom action ran (condition was true)
        let ca_event = trace
            .events
            .iter()
            .find(|e| e.action_id == "MyCustomAction");
        assert!(ca_event.is_some());
        assert!(matches!(ca_event.unwrap().result, ActionResult::Success));
    }

    #[test]
    fn test_upgrade_simulation() {
        let sim = SimulatorBuilder::new()
            .with_installation_stages()
            .with_custom_action("OnUpgrade", Some("Upgrading".into()), 6500)
            .with_custom_action("OnFirstInstall", Some("FirstInstall".into()), 6501)
            .build();

        let trace = sim.run(InstallMode::Upgrade, &SimulationOptions::default());

        assert!(trace.success);

        // OnUpgrade should run
        let upgrade_event = trace.events.iter().find(|e| e.action_id == "OnUpgrade");
        assert!(matches!(
            upgrade_event.unwrap().result,
            ActionResult::Success
        ));

        // OnFirstInstall should be skipped
        let first_event = trace
            .events
            .iter()
            .find(|e| e.action_id == "OnFirstInstall");
        assert!(matches!(first_event.unwrap().result, ActionResult::Skipped));
    }

    #[test]
    fn test_uninstall_simulation() {
        let sim = SimulatorBuilder::new()
            .with_installation_stages()
            .with_custom_action("OnUninstall", Some("Uninstalling".into()), 3400)
            .build();

        let trace = sim.run(InstallMode::Uninstall, &SimulationOptions::default());

        assert!(trace.success);

        // OnUninstall should run
        let uninstall_event = trace.events.iter().find(|e| e.action_id == "OnUninstall");
        assert!(matches!(
            uninstall_event.unwrap().result,
            ActionResult::Success
        ));
    }
}
