use serde_yaml::Value;

fn yaml(s: &str) -> Value {
    serde_yaml::from_str(s).unwrap()
}

use wixer::analysis::yaml::orphan_detection::{OrphanKind, find_orphans};

#[test]
fn orphan_kind_display_formats_as_expected() {
    assert_eq!(format!("{}", OrphanKind::Component), "Component");
    assert_eq!(format!("{}", OrphanKind::ComponentGroup), "ComponentGroup");
}

#[test]
fn orphan_detection_handles_components_in_unreferenced_groups_without_emitting_component_warning() {
    let yaml: Value = yaml(
        r#"
Wix:
  Package:
    ComponentGroup:
      - Id: G1
        Component:
          - Id: C1
    Feature:
      Id: Main
"#,
    );

    let orphans = find_orphans(&yaml);

    assert!(
        orphans
            .iter()
            .any(|o| o.kind == OrphanKind::ComponentGroup && o.id == "G1")
    );
    assert!(
        !orphans
            .iter()
            .any(|o| o.kind == OrphanKind::Component && o.id == "C1")
    );
}

#[test]
fn orphan_detection_supports_component_refs_as_strings_and_mappings() {
    let yaml: Value = yaml(
        r#"
Wix:
  Package:
    ComponentGroup:
      - Id: G1
        Component:
          - Id: C1
          - Id: C2
    Feature:
      Id: Main
      ComponentGroupRef:
        Id: G1
      ComponentRef:
        - "C1"
        - Id: C2
"#,
    );

    let orphans = find_orphans(&yaml);
    assert!(orphans.is_empty(), "Unexpected orphans: {:?}", orphans);
}

fn with_debug_tracing<T>(f: impl FnOnce() -> T) -> T {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    let dispatch = tracing::dispatcher::Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, f)
}

#[test]
fn orphan_detection_emits_debug_and_warn_logs_when_tracing_is_enabled() {
    let yaml: Value = yaml(
        r#"
Wix:
  Package:
    Component:
      - Id: C1
    ComponentGroup:
      - Id: G1
        Component:
          - Id: GC1
    Feature:
      Id: Main
      ComponentRef:
        - Id: C1
"#,
    );

    with_debug_tracing(|| {
        let orphans = find_orphans(&yaml);
        assert!(
            orphans
                .iter()
                .any(|o| o.kind == OrphanKind::ComponentGroup && o.id == "G1")
        );
    });
}

#[test]
fn orphan_detection_handles_component_ref_inside_group() {
    let yaml: Value = yaml(
        r#"
Wix:
  Package:
    Component:
      - Id: C1
    ComponentGroup:
      - Id: G1
        ComponentRef:
          Id: C1
    Feature:
      Id: Main
      ComponentGroupRef:
        Id: G1
"#,
    );

    let orphans = find_orphans(&yaml);
    assert!(orphans.is_empty(), "Unexpected orphans: {:?}", orphans);
}

#[test]
fn orphan_detection_handles_groups_without_id_gracefully() {
    let yaml: Value = yaml(
        r#"
Wix:
  Package:
    Component:
      - Id: C1
    ComponentGroup:
      - ComponentRef:
          Id: C1
    Feature:
      Id: Main
"#,
    );

    let orphans = find_orphans(&yaml);
    assert!(
        orphans
            .iter()
            .any(|o| o.kind == OrphanKind::Component && o.id == "C1")
    );
}

#[test]
fn orphan_detection_supports_component_group_ref_as_string() {
    let yaml: Value = yaml(
        r#"
Wix:
  Package:
    ComponentGroup:
      - Id: G1
        Component:
          - Id: C1
    Feature:
      Id: Main
      ComponentGroupRef: G1
"#,
    );

    let orphans = find_orphans(&yaml);
    assert!(orphans.is_empty(), "Unexpected orphans: {:?}", orphans);
}

#[test]
fn orphan_detection_supports_component_ref_sequences_inside_group() {
    let yaml: Value = yaml(
        r#"
Wix:
  Package:
    Component:
      - Id: C1
      - Id: C2
    ComponentGroup:
      - Id: G1
        ComponentRef:
          - Id: C1
          - Id: C2
    Feature:
      Id: Main
      ComponentGroupRef:
        Id: G1
"#,
    );

    let orphans = find_orphans(&yaml);
    assert!(orphans.is_empty(), "Unexpected orphans: {:?}", orphans);
}
