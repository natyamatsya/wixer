mod test_support;

use wixer::simulation::ExtensionRegistry;

#[test]
fn extension_registry_registers_and_normalizes_names() {
    let mut registry = ExtensionRegistry::new();

    let ext = test_support::extension::dummy_extension("WixToolset.MyExt.wixext");

    registry.register(ext);

    assert!(registry.get("WixToolset.MyExt.wixext").is_some());
    assert!(registry.get("wixtoolset.myext.wixext").is_some());
    assert!(registry.get("myext").is_some());
    assert!(registry.get("MyExt").is_some());

    assert!(registry.get("WixToolset.Unknown.wixext").is_none());
}

#[test]
fn extension_registry_available_is_sorted_and_deduped() {
    let registry = test_support::registry::bundled();
    let names = registry.available();

    assert!(!names.is_empty());

    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}
