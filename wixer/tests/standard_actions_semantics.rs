use wixer::simulation::{
    SequenceTable, default_execute_sequence, default_ui_sequence, get_standard_action,
    is_standard_action,
};

#[test]
fn standard_actions_lookup_and_sequences_contain_expected_tables() {
    assert!(is_standard_action("InstallFiles"));
    assert!(!is_standard_action("DefinitelyNotAStandardAction"));

    let sa = get_standard_action("InstallFiles").unwrap();
    assert_eq!(sa.name, "InstallFiles");
    assert!(sa.execute_sequence.is_some());

    let exec = default_execute_sequence();
    assert_eq!(exec.table, SequenceTable::InstallExecuteSequence);
    assert!(!exec.actions.is_empty());

    let ui = default_ui_sequence();
    assert_eq!(ui.table, SequenceTable::InstallUISequence);
    assert!(!ui.actions.is_empty());
}

#[test]
fn standard_actions_sequences_are_sorted_by_sequence_number() {
    let exec = default_execute_sequence();
    let sorted = exec.sorted_actions();

    assert!(!sorted.is_empty());

    for pair in sorted.windows(2) {
        let a = pair[0].sequence_number.unwrap_or(0);
        let b = pair[1].sequence_number.unwrap_or(0);
        assert!(a <= b);
    }
}
