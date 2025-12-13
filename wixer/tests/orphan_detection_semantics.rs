use serde_yaml::Value;

mod test_support;

use wixer::analysis::yaml::orphan_detection::{OrphanKind, find_orphans};

#[test]
fn orphan_kind_display_formats_as_expected() {
    assert_eq!(format!("{}", OrphanKind::Component), "Component");
    assert_eq!(format!("{}", OrphanKind::ComponentGroup), "ComponentGroup");
}

#[test]
fn orphan_detection_handles_components_in_unreferenced_groups_without_emitting_component_warning() {
    let yaml: Value = test_support::yaml(
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
    let yaml: Value = test_support::yaml(
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

#[test]
fn orphan_detection_handles_component_ref_inside_group() {
    let yaml: Value = test_support::yaml(
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
    let yaml: Value = test_support::yaml(
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
