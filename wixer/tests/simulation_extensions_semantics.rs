use wixer::simulation::{
    ExtensionRegistry, ExtractedCustomAction, ExtractedExtension, ExtractedLicense,
    ExtractedSequenceEntry, SequencePosition, SequenceTable,
};

#[test]
fn extracted_extension_to_actions_applies_execute_impersonate_return_and_scheduling() {
    let ext = ExtractedExtension {
        license: ExtractedLicense {
            notice: "n".into(),
            source: "s".into(),
            spdx_id: "MIT".into(),
        },
        id: "WixToolset.Test.wixext".into(),
        namespace: None,
        custom_actions: vec![
            ExtractedCustomAction {
                id: "CA_Deferred".into(),
                dll_entry: Some("Entry".into()),
                execute: Some("deferred".into()),
                r#return: Some("ignore".into()),
                impersonate: Some("no".into()),
                binary_ref: Some("Bin".into()),
                suppress_modularization: false,
                hide_target: false,
            },
            ExtractedCustomAction {
                id: "CA_Rollback".into(),
                dll_entry: Some("Entry".into()),
                execute: Some("rollback".into()),
                r#return: None,
                impersonate: None,
                binary_ref: Some("Bin".into()),
                suppress_modularization: false,
                hide_target: false,
            },
            ExtractedCustomAction {
                id: "CA_Commit".into(),
                dll_entry: Some("Entry".into()),
                execute: Some("commit".into()),
                r#return: None,
                impersonate: None,
                binary_ref: Some("Bin".into()),
                suppress_modularization: false,
                hide_target: false,
            },
        ],
        sequence_entries: vec![
            ExtractedSequenceEntry {
                action: "CA_Deferred".into(),
                sequence_table: SequenceTable::InstallExecuteSequence.name().into(),
                before: Some("InstallFinalize".into()),
                after: None,
                condition: Some("1".into()),
                overridable: false,
            },
            ExtractedSequenceEntry {
                action: "CA_Rollback".into(),
                sequence_table: SequenceTable::InstallExecuteSequence.name().into(),
                before: None,
                after: Some("InstallFiles".into()),
                condition: None,
                overridable: false,
            },
        ],
        publish_entries: vec![],
        properties: vec![],
        binaries: vec![],
    };

    let actions = ext.to_actions();

    let deferred = actions.iter().find(|a| a.id == "CA_Deferred").unwrap();
    assert!(deferred.execution.deferred);
    assert!(deferred.execution.no_impersonate);
    assert!(deferred.execution.continue_on_error);
    assert_eq!(deferred.condition.as_deref(), Some("1"));
    assert!(matches!(
        deferred.position,
        Some(SequencePosition::Before(ref s)) if s == "InstallFinalize"
    ));

    let rollback = actions.iter().find(|a| a.id == "CA_Rollback").unwrap();
    assert!(rollback.execution.rollback);
    assert!(matches!(
        rollback.position,
        Some(SequencePosition::After(ref s)) if s == "InstallFiles"
    ));

    let commit = actions.iter().find(|a| a.id == "CA_Commit").unwrap();
    assert!(commit.execution.commit);
    assert!(commit.position.is_none());
}

#[test]
fn extracted_extension_actions_for_table_filters_by_sequence_entries() {
    let ext = ExtractedExtension {
        license: ExtractedLicense {
            notice: "n".into(),
            source: "s".into(),
            spdx_id: "MIT".into(),
        },
        id: "WixToolset.Test.wixext".into(),
        namespace: None,
        custom_actions: vec![
            ExtractedCustomAction {
                id: "CA_Exec".into(),
                dll_entry: Some("Entry".into()),
                execute: Some("deferred".into()),
                r#return: None,
                impersonate: None,
                binary_ref: Some("Bin".into()),
                suppress_modularization: false,
                hide_target: false,
            },
            ExtractedCustomAction {
                id: "CA_UI".into(),
                dll_entry: Some("Entry".into()),
                execute: None,
                r#return: None,
                impersonate: None,
                binary_ref: Some("Bin".into()),
                suppress_modularization: false,
                hide_target: false,
            },
        ],
        sequence_entries: vec![
            ExtractedSequenceEntry {
                action: "CA_Exec".into(),
                sequence_table: SequenceTable::InstallExecuteSequence.name().into(),
                before: Some("InstallFinalize".into()),
                after: None,
                condition: None,
                overridable: false,
            },
            ExtractedSequenceEntry {
                action: "CA_UI".into(),
                sequence_table: SequenceTable::InstallUISequence.name().into(),
                before: None,
                after: Some("WelcomeDlg".into()),
                condition: None,
                overridable: false,
            },
        ],
        publish_entries: vec![],
        properties: vec![],
        binaries: vec![],
    };

    let exec = ext.actions_for_table(SequenceTable::InstallExecuteSequence);
    assert!(exec.iter().any(|a| a.id == "CA_Exec"));
    assert!(!exec.iter().any(|a| a.id == "CA_UI"));

    let ui = ext.actions_for_table(SequenceTable::InstallUISequence);
    assert!(ui.iter().any(|a| a.id == "CA_UI"));
    assert!(!ui.iter().any(|a| a.id == "CA_Exec"));
}

#[test]
fn extension_registry_available_is_sorted_and_deduped_with_registering_duplicates() {
    let mut r = ExtensionRegistry::new();

    let ext = ExtractedExtension {
        license: ExtractedLicense {
            notice: "n".into(),
            source: "s".into(),
            spdx_id: "MIT".into(),
        },
        id: "WixToolset.Util.wixext".into(),
        namespace: None,
        custom_actions: vec![],
        sequence_entries: vec![],
        publish_entries: vec![],
        properties: vec![],
        binaries: vec![],
    };

    r.register(ext);

    let names = r.available();
    assert_eq!(names, vec!["WixToolset.Util.wixext"]);
}
