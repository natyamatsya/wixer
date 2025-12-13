mod test_support;

use serde_yaml::Value;

use wixer::analysis::yaml::publish_conflicts::{self, ConflictSuggestion};
use wixer::constants::ui_events::{ACTION_EVENTS, DO_ACTION, NAVIGATION_EVENTS, SPAWN_DIALOG};

fn first_ui_publish_entry() -> (String, String, String, Option<String>, i32) {
    let ui = test_support::registry::ui();

    let entry = ui
        .publish_entries
        .iter()
        .find(|e| e.event.is_some())
        .expect("ui extension must have publish entries with events");

    (
        entry.dialog.clone(),
        entry.control.clone(),
        entry.event.clone().unwrap_or_default(),
        entry.value.clone(),
        entry.order.unwrap_or(1),
    )
}

fn first_ui_navigation_publish_entry_with_value() -> (String, String, String, String, i32) {
    let ui = test_support::registry::ui();

    let entry = ui
        .publish_entries
        .iter()
        .find(|e| {
            e.event
                .as_deref()
                .is_some_and(|ev| NAVIGATION_EVENTS.contains(&ev))
                && e.value.is_some()
        })
        .expect("ui extension must have navigation publish entries with values");

    (
        entry.dialog.clone(),
        entry.control.clone(),
        entry.event.clone().unwrap_or_default(),
        entry.value.clone().unwrap_or_default(),
        entry.order.unwrap_or(1),
    )
}

fn max_ui_order_for(dialog: &str, control: &str) -> i32 {
    let ui = test_support::registry::ui();

    ui.publish_entries
        .iter()
        .filter(|e| e.dialog == dialog && e.control == control)
        .map(|e| e.order.unwrap_or(1))
        .max()
        .unwrap_or(1)
}

fn first_ui_publish_entry_with_value() -> (String, String, String, String) {
    let ui = test_support::registry::ui();

    let entry = ui
        .publish_entries
        .iter()
        .find(|e| e.event.is_some() && e.value.is_some())
        .expect("ui extension must have publish entries with values");

    (
        entry.dialog.clone(),
        entry.control.clone(),
        entry.event.clone().unwrap_or_default(),
        entry.value.clone().unwrap_or_default(),
    )
}

#[test]
fn publish_conflicts_suggests_override_when_user_matches_extension_event() {
    let (dialog, control, event, value, order) = first_ui_publish_entry();

    let yaml: Value = test_support::yaml(&format!(
        r#"
Wix:
  UI:
    Publish:
      Dialog: {dialog}
      Control: {control}
      Event: {event}
      Value: {value}
      Order: {order}
"#,
        value = value.clone().unwrap_or_else(|| "".to_string())
    ));

    let conflicts = publish_conflicts::find_conflicts(&yaml);
    assert!(!conflicts.is_empty());

    if let Some(v) = value {
        let s = conflicts
            .iter()
            .find_map(|c| match &c.suggestion {
                ConflictSuggestion::UseOverride { target_dialog, .. } => Some(target_dialog),
                _ => None,
            })
            .expect("expected UseOverride suggestion");
        assert_eq!(s, &v);
    }
}

#[test]
fn publish_conflicts_reports_invalid_overrides_and_lists_available_targets() {
    let (dialog, control, _event, existing_target) = first_ui_publish_entry_with_value();

    let yaml: Value = test_support::yaml(&format!(
        r#"
Wix:
  UI:
    Publish:
      - Dialog: {dialog}
        Control: {control}
        Event: "Override TotallyNotARealTarget"
      - Dialog: {dialog}
        Control: {control}
        Event: "override {existing_target}"
"#,
    ));

    let invalid = publish_conflicts::find_invalid_overrides(&yaml);
    let i = invalid
        .iter()
        .find(|i| i.dialog == dialog && i.control == control)
        .expect("expected invalid override for dialog/control");
    assert!(i.available_targets.contains(&existing_target));
}

#[test]
fn publish_conflicts_suggests_change_order_for_action_before_navigation() {
    let (dialog, control, ext_event, _target, order) =
        first_ui_navigation_publish_entry_with_value();
    assert!(NAVIGATION_EVENTS.contains(&ext_event.as_str()));

    let yaml: Value = test_support::yaml(&format!(
        r#"
Wix:
  UI:
    Publish:
      Dialog: {dialog}
      Control: {control}
      Event: {user_event}
      Value: MyCA
      Order: {order}
"#,
        user_event = DO_ACTION,
    ));

    let conflicts = publish_conflicts::find_conflicts(&yaml);
    assert!(!conflicts.is_empty());

    let suggested_order = conflicts
        .iter()
        .find_map(|c| match c.suggestion {
            ConflictSuggestion::ChangeOrder {
                suggested_order, ..
            } => Some(suggested_order),
            _ => None,
        })
        .expect("expected ChangeOrder suggestion");
    let expected = order.saturating_sub(1).max(1);
    assert_eq!(suggested_order, expected);
}

#[test]
fn publish_conflicts_suggests_override_for_navigation_event_mismatch() {
    let (dialog, control, ext_event, target, order) =
        first_ui_navigation_publish_entry_with_value();
    assert!(NAVIGATION_EVENTS.contains(&ext_event.as_str()));

    let yaml: Value = test_support::yaml(&format!(
        r#"
Wix:
  UI:
    Publish:
      Dialog: {dialog}
      Control: {control}
      Event: {user_event}
      Value: Something
      Order: {order}
"#,
        user_event = SPAWN_DIALOG,
    ));

    let conflicts = publish_conflicts::find_conflicts(&yaml);
    assert!(!conflicts.is_empty());

    let s = conflicts
        .iter()
        .find_map(|c| match &c.suggestion {
            ConflictSuggestion::UseOverride { target_dialog, .. } => Some(target_dialog),
            _ => None,
        })
        .expect("expected UseOverride suggestion");
    assert_eq!(s, &target);
}

#[test]
fn publish_conflicts_suggests_add_with_order_by_default() {
    let (dialog, control, _event, _target, order) = first_ui_navigation_publish_entry_with_value();
    let max_order = max_ui_order_for(&dialog, &control);

    let yaml: Value = test_support::yaml(&format!(
        r#"
Wix:
  UI:
    Publish:
      Dialog: {dialog}
      Control: {control}
      Event: SomeOtherEvent
      Order: {order}
"#,
    ));

    let conflicts = publish_conflicts::find_conflicts(&yaml);
    assert!(!conflicts.is_empty());

    let suggested_order = conflicts
        .iter()
        .find_map(|c| match c.suggestion {
            ConflictSuggestion::AddWithOrder { suggested_order } => Some(suggested_order),
            _ => None,
        })
        .expect("expected AddWithOrder suggestion");
    assert_eq!(suggested_order, max_order + 10);
}

#[test]
fn publish_conflicts_invalid_override_has_empty_available_targets_when_control_is_unknown() {
    let yaml: Value = test_support::yaml(
        r#"
Wix:
  UI:
    Publish:
      Dialog: TotallyUnknownDlg
      Control: TotallyUnknownControl
      Event: "override SomeDlg"
"#,
    );

    let invalid = publish_conflicts::find_invalid_overrides(&yaml);
    let i = invalid
        .iter()
        .find(|i| i.dialog == "TotallyUnknownDlg" && i.control == "TotallyUnknownControl")
        .expect("expected invalid override for unknown dialog/control");
    assert!(i.available_targets.is_empty());
}

#[test]
fn publish_conflicts_override_parsing_is_case_insensitive_and_ignores_empty_targets() {
    let (dialog, control, _event, existing_target) = first_ui_publish_entry_with_value();

    let yaml: Value = test_support::yaml(&format!(
        r#"
Wix:
  UI:
    Publish:
      - Dialog: {dialog}
        Control: {control}
        Event: "OvErRiDe {existing_target}"
      - Dialog: {dialog}
        Control: {control}
        Event: "override   "
"#
    ));

    let invalid = publish_conflicts::find_invalid_overrides(&yaml);
    assert!(invalid.is_empty());

    assert!(ACTION_EVENTS.contains(&DO_ACTION));
}
