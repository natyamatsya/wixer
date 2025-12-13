use wixer::simulation::{ExtensionRegistry, ExtractedExtension, ExtractedLicense};

fn dummy_extension(id: &str) -> ExtractedExtension {
    ExtractedExtension {
        license: ExtractedLicense {
            notice: "n".into(),
            source: "s".into(),
            spdx_id: "MIT".into(),
        },
        id: id.into(),
        namespace: None,
        custom_actions: vec![],
        sequence_entries: vec![],
        publish_entries: vec![],
        properties: vec![],
        binaries: vec![],
    }
}

#[test]
fn extension_registry_registers_and_normalizes_names() {
    let mut registry = ExtensionRegistry::new();

    let ext = dummy_extension("WixToolset.MyExt.wixext");

    registry.register(ext);

    assert!(registry.get("WixToolset.MyExt.wixext").is_some());
    assert!(registry.get("wixtoolset.myext.wixext").is_some());
    assert!(registry.get("myext").is_some());
    assert!(registry.get("MyExt").is_some());

    assert!(registry.get("WixToolset.Unknown.wixext").is_none());
}

#[test]
fn extension_registry_available_is_sorted_and_deduped() {
    let registry = ExtensionRegistry::load_bundled();
    let names = registry.available();

    assert!(!names.is_empty());

    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}
