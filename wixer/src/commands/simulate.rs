//! Simulate MSI installer execution flow
//!
//! This command parses a WiX project and simulates its execution to show
//! what actions would run in different installation scenarios.

use crate::includes::load_and_process_yaml;
use crate::simulation::{
    Action, ActionResult, CustomActionType, ExecutionTrace, InstallMode, SequencePosition,
    SequenceTable, SimulationOptions, Simulator,
};
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::path::Path;
use tabled::{Table, Tabled, settings::Style};
use tracing::info;

/// Installation mode for simulation
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum SimulateMode {
    /// Fresh installation
    FirstInstall,
    /// Upgrade from older version
    Upgrade,
    /// Complete uninstallation
    Uninstall,
    /// Repair installation
    Repair,
    /// Modify features
    Maintenance,
}

impl From<SimulateMode> for InstallMode {
    fn from(mode: SimulateMode) -> Self {
        match mode {
            SimulateMode::FirstInstall => InstallMode::FirstInstall,
            SimulateMode::Upgrade => InstallMode::Upgrade,
            SimulateMode::Uninstall => InstallMode::Uninstall,
            SimulateMode::Repair => InstallMode::Repair,
            SimulateMode::Maintenance => InstallMode::Maintenance,
        }
    }
}

#[derive(Tabled)]
struct EventRow {
    #[tabled(rename = "Seq")]
    sequence: String,
    #[tabled(rename = "Action")]
    action: String,
    #[tabled(rename = "Result")]
    result: String,
    #[tabled(rename = "Condition")]
    condition: String,
}

pub fn run(input: &Path, mode: SimulateMode, verbose: bool) -> Result<()> {
    info!(file = %input.display(), mode = ?mode, "Simulating installer execution");

    let simulator = build_simulator_from_file(input)?;
    let options = SimulationOptions {
        verbose,
        ..Default::default()
    };

    let trace = simulator.run(mode.into(), &options);
    print_trace(&trace, verbose);

    Ok(())
}

fn build_simulator_from_file(path: &Path) -> Result<Simulator> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    if ext == "yaml" || path.to_string_lossy().ends_with(".wxs.yaml") {
        build_simulator_from_yaml(path)
    } else if ext == "wxs" || ext == "xml" {
        build_simulator_from_xml(path)
    } else {
        anyhow::bail!("Unsupported file type: {}", path.display());
    }
}

fn build_simulator_from_yaml(path: &Path) -> Result<Simulator> {
    let yaml = load_and_process_yaml(path).context("Failed to load YAML")?;
    let mut simulator = Simulator::new();

    // Navigate to Package element
    let package = yaml
        .get("Wix")
        .and_then(|w| w.get("Package"))
        .context("No Wix/Package element found")?;

    // Extract SetProperty actions
    if let Some(Value::Sequence(props)) = package.get("SetProperty") {
        for prop in props {
            if let Some(id) = prop.get("Id").and_then(|v| v.as_str()) {
                let condition = prop.get("Condition").and_then(|v| v.as_str());
                let sequence = prop
                    .get("Sequence")
                    .and_then(|v| v.as_str())
                    .unwrap_or("execute");
                let before = prop.get("Before").and_then(|v| v.as_str());
                let after = prop.get("After").and_then(|v| v.as_str());

                let seq_table = if sequence == "ui" {
                    SequenceTable::InstallUISequence
                } else {
                    SequenceTable::InstallExecuteSequence
                };

                // Determine position
                let position = if let Some(before_action) = before {
                    Some(SequencePosition::Before(before_action.to_string()))
                } else {
                    after.map(|after_action| SequencePosition::After(after_action.to_string()))
                };

                // Resolve to a sequence number (simplified - use standard boundaries)
                let seq_num = resolve_position_to_number(&position, before, after);

                simulator.add_set_property(
                    format!("Set{}", id),
                    id,
                    prop.get("Value").and_then(|v| v.as_str()).unwrap_or("true"),
                    condition.map(String::from),
                    seq_table,
                    seq_num,
                );
            }
        }
    }

    // Extract CustomAction definitions
    if let Some(Value::Sequence(actions)) = package.get("CustomAction") {
        for ca in actions {
            if let Some(id) = ca.get("Id").and_then(|v| v.as_str()) {
                let ca_type = if ca.get("Property").is_some() {
                    CustomActionType::SetProperty {
                        property: ca
                            .get("Property")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        value: ca
                            .get("Value")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    }
                } else if ca.get("BinaryRef").is_some() || ca.get("BinaryKey").is_some() {
                    CustomActionType::DllBinary {
                        dll_entry: ca
                            .get("DllEntry")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        binary_ref: ca
                            .get("BinaryRef")
                            .or_else(|| ca.get("BinaryKey"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    }
                } else {
                    CustomActionType::SetProperty {
                        property: String::new(),
                        value: String::new(),
                    }
                };

                let action = Action::new_custom(id, ca_type);
                simulator.add_custom_action(action);
            }
        }
    }

    // Extract InstallExecuteSequence Custom actions
    extract_sequence_actions(package, "InstallExecuteSequence", &mut simulator)?;
    extract_sequence_actions(package, "InstallUISequence", &mut simulator)?;

    Ok(simulator)
}

fn extract_sequence_actions(
    package: &Value,
    seq_name: &str,
    simulator: &mut Simulator,
) -> Result<()> {
    let seq_table = match seq_name {
        "InstallExecuteSequence" => SequenceTable::InstallExecuteSequence,
        "InstallUISequence" => SequenceTable::InstallUISequence,
        _ => return Ok(()),
    };

    if let Some(seq) = package.get(seq_name) {
        // Handle array of Custom entries
        if let Some(Value::Sequence(customs)) = seq.get("Custom") {
            for custom in customs {
                extract_custom_entry(custom, seq_table, simulator);
            }
        }
        // Handle single Custom entry
        if let Some(custom) = seq.get("Custom")
            && custom.is_mapping()
        {
            extract_custom_entry(custom, seq_table, simulator);
        }
        // Handle array format [{Custom: [...]}]
        if let Value::Sequence(items) = seq {
            for item in items {
                if let Some(Value::Sequence(customs)) = item.get("Custom") {
                    for custom in customs {
                        extract_custom_entry(custom, seq_table, simulator);
                    }
                }
            }
        }
    }

    Ok(())
}

fn extract_custom_entry(custom: &Value, seq_table: SequenceTable, simulator: &mut Simulator) {
    let action_id = custom.get("Action").and_then(|v| v.as_str());
    let condition = custom.get("Condition").and_then(|v| v.as_str());
    let before = custom.get("Before").and_then(|v| v.as_str());
    let after = custom.get("After").and_then(|v| v.as_str());

    if let Some(id) = action_id {
        let position = if let Some(before_action) = before {
            Some(SequencePosition::Before(before_action.to_string()))
        } else {
            after.map(|after_action| SequencePosition::After(after_action.to_string()))
        };

        let seq_num = resolve_position_to_number(&position, before, after);

        let mut action = Action::new_custom(
            id,
            CustomActionType::DllBinary {
                dll_entry: String::new(),
                binary_ref: String::new(),
            },
        );
        action.sequence_number = Some(seq_num);
        action.position = position;
        action.condition = condition.map(String::from);

        match seq_table {
            SequenceTable::InstallExecuteSequence => simulator.add_to_execute_sequence(action),
            SequenceTable::InstallUISequence => simulator.add_to_ui_sequence(action),
            _ => {}
        }
    }
}

fn resolve_position_to_number(
    position: &Option<SequencePosition>,
    before: Option<&str>,
    after: Option<&str>,
) -> i32 {
    // Use well-known sequence boundaries
    match position {
        Some(SequencePosition::Before(target)) => match target.as_str() {
            "InstallFinalize" => 6590,
            "InstallInitialize" => 1490,
            "RemoveFiles" => 3490,
            "InstallFiles" => 3990,
            "AppSearch" => 40,
            _ => 6500, // Default: before InstallFinalize
        },
        Some(SequencePosition::After(target)) => match target.as_str() {
            "FindRelatedProducts" => 30,
            "AppSearch" => 60,
            "CostFinalize" => 1010,
            "InstallInitialize" => 1510,
            "InstallFiles" => 4010,
            "RemoveExistingProducts" => 6710,
            _ => 6500,
        },
        _ => {
            // Fallback based on string values
            if let Some(b) = before {
                match b {
                    "InstallFinalize" => 6590,
                    "RemoveFiles" => 3490,
                    _ => 6500,
                }
            } else if let Some(a) = after {
                match a {
                    "FindRelatedProducts" => 30,
                    "AppSearch" => 60,
                    _ => 6500,
                }
            } else {
                6500
            }
        }
    }
}

fn build_simulator_from_xml(_path: &Path) -> Result<Simulator> {
    // TODO: Implement XML parsing
    anyhow::bail!("XML parsing not yet implemented. Please use YAML files.")
}

fn print_trace(trace: &ExecutionTrace, verbose: bool) {
    println!();
    println!(
        "=== Simulation: {:?} ===",
        trace.install_mode.unwrap_or(InstallMode::FirstInstall)
    );
    println!();

    let rows: Vec<EventRow> = trace
        .events
        .iter()
        .filter(|e| verbose || !matches!(e.result, ActionResult::Skipped))
        .map(|e| EventRow {
            sequence: e.sequence_number.to_string(),
            action: e.action_id.clone(),
            result: match &e.result {
                ActionResult::Success => "✓".to_string(),
                ActionResult::Skipped => "○ skip".to_string(),
                ActionResult::Failed(msg) => format!("✗ {}", msg),
                ActionResult::NotApplicable => "- N/A".to_string(),
            },
            condition: e.condition.clone().unwrap_or_default(),
        })
        .collect();

    if rows.is_empty() {
        println!("No actions to display.");
    } else {
        let table = Table::new(&rows).with(Style::modern()).to_string();
        println!("{}", table);
    }

    println!();
    println!("{}", trace.summary());

    // Show final properties of interest
    if verbose {
        println!();
        println!("=== Final Properties ===");
        let interesting = [
            "FirstInstall",
            "Upgrading",
            "Uninstalling",
            "Maintenance",
            "Installed",
        ];
        for prop in interesting {
            if let Some(val) = trace.final_properties.get(prop) {
                println!("  {} = {:?}", prop, val);
            }
        }
    }
}
