use wixer::simulation::{
    Action, ActionResult, CustomActionType, InstallMode, PropertyValue, SequencePosition,
    SequenceTable, SimulationOptions,
};

#[test]
fn simulator_runs_ui_and_execute_sequences_and_tracks_skips_and_failures() {
    let mut sim = wixer::simulation::Simulator::default();

    // UI sequence action should run when include_ui=true.
    sim.add_to_ui_sequence(
        Action::new_custom(
            "UI_OK",
            CustomActionType::SetProperty {
                property: "UI_RAN".into(),
                value: "1".into(),
            },
        )
        .with_sequence(1),
    );

    // Execute sequence action that should be skipped due to false condition.
    // Place it before FAIL_ME so stop_on_error still allows observing it.
    sim.add_to_execute_sequence(
        Action::new_custom(
            "SKIP_ME",
            CustomActionType::SetProperty {
                property: "NEVER".into(),
                value: "1".into(),
            },
        )
        .with_sequence(1499)
        .with_condition("0"),
    );

    // Execute sequence action that fails.
    sim.add_to_execute_sequence(
        Action::new_custom(
            "FAIL_ME",
            CustomActionType::Error {
                message: "boom".into(),
            },
        )
        .with_sequence(1500),
    );

    let options = SimulationOptions {
        include_ui: true,
        stop_on_error: true,
        ..Default::default()
    };

    let trace = sim.run(InstallMode::FirstInstall, &options);

    // UI action should execute successfully.
    assert!(
        trace
            .events
            .iter()
            .any(|e| e.sequence_table == SequenceTable::InstallUISequence
                && e.action_id == "UI_OK"
                && e.result == ActionResult::Success)
    );

    // Failure should be recorded and stop the run.
    assert!(!trace.success);
    assert!(
        trace
            .events
            .iter()
            .any(|e| e.action_id == "FAIL_ME" && matches!(e.result, ActionResult::Failed(_)))
    );

    // Skipped action should not execute (condition false).
    assert!(
        trace
            .events
            .iter()
            .any(|e| e.action_id == "SKIP_ME" && e.result == ActionResult::Skipped)
    );
}

#[test]
fn simulator_applies_overrides_suppression_and_set_property_expansion() {
    let mut sim = wixer::simulation::Simulator::new();

    // Add a base action with a known sequence number.
    sim.add_to_execute_sequence(Action::new_standard("Base", 100));

    // Add an action we will override/suppress.
    sim.add_to_execute_sequence(
        Action::new_custom(
            "ToSuppress",
            CustomActionType::SetProperty {
                property: "SHOULD_NOT_SET".into(),
                value: "1".into(),
            },
        )
        .with_sequence(110),
    );

    // Override that suppresses the action.
    sim.add_to_execute_sequence(Action::override_action("ToSuppress").suppressed());

    // Action with Before/After positioning.
    let mut before_base = Action::new_custom(
        "BeforeBase",
        CustomActionType::SetProperty {
            property: "BEFORE".into(),
            value: "1".into(),
        },
    );
    before_base.position = Some(SequencePosition::Before("Base".into()));
    sim.add_to_execute_sequence(before_base);

    let mut after_base = Action::new_custom(
        "AfterBase",
        CustomActionType::SetProperty {
            property: "AFTER".into(),
            value: "1".into(),
        },
    );
    after_base.position = Some(SequencePosition::After("Base".into()));
    sim.add_to_execute_sequence(after_base);

    // A SetProperty action executed via the simulator's SetProperty action path.
    sim.add_set_property(
        "SetExpanded",
        "EXPANDED",
        "prefix_[P]_suffix",
        None,
        SequenceTable::InstallExecuteSequence,
        105,
    );

    let mut options = SimulationOptions {
        include_ui: false,
        ..Default::default()
    };

    // Ensure expansion has something to expand.
    options
        .property_overrides
        .insert("P".to_string(), "X".to_string());

    let trace = sim.run(InstallMode::FirstInstall, &options);

    // Suppressed override should remove the base action from execution.
    assert!(!trace.events.iter().any(|e| e.action_id == "ToSuppress"));

    // Ensure Before/After were resolved around Base by sequence number.
    let idx = |id: &str| {
        trace
            .events
            .iter()
            .position(|e| e.action_id == id)
            .expect("expected event")
    };

    assert!(idx("BeforeBase") < idx("Base"));
    assert!(idx("Base") < idx("AfterBase"));

    // Ensure SetProperty expansion happened.
    assert_eq!(
        trace.final_properties.get("EXPANDED"),
        Some(&PropertyValue::String("prefix_X_suffix".into()))
    );

    // Sanity: we touched override path.
    assert!(trace.events.iter().any(|e| e.action_id == "Base"));
}

#[test]
fn simulator_resolves_absolute_and_unknown_before_after_targets() {
    let mut sim = wixer::simulation::Simulator::new();

    sim.add_to_execute_sequence(Action::new_standard("Base", 100));

    // Absolute positioning.
    sim.add_to_execute_sequence(
        Action::new_custom(
            "Abs",
            CustomActionType::SetProperty {
                property: "ABS".into(),
                value: "1".into(),
            },
        )
        .with_sequence(10),
    );

    // Before/After targets that do not exist should result in no sequence number and thus no event.
    let mut before_missing = Action::new_custom(
        "BeforeMissing",
        CustomActionType::SetProperty {
            property: "NO".into(),
            value: "1".into(),
        },
    );
    before_missing.position = Some(SequencePosition::Before("TotallyMissing".into()));
    sim.add_to_execute_sequence(before_missing);

    let mut after_missing = Action::new_custom(
        "AfterMissing",
        CustomActionType::SetProperty {
            property: "NO".into(),
            value: "1".into(),
        },
    );
    after_missing.position = Some(SequencePosition::After("AlsoMissing".into()));
    sim.add_to_execute_sequence(after_missing);

    let options = SimulationOptions::default();
    let trace = sim.run(InstallMode::FirstInstall, &options);

    assert!(trace.events.iter().any(|e| e.action_id == "Abs"));
    assert!(!trace.events.iter().any(|e| e.action_id == "BeforeMissing"));
    assert!(!trace.events.iter().any(|e| e.action_id == "AfterMissing"));
}
