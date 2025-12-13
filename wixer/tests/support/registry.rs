use std::sync::OnceLock;

use wixer::simulation::{ExtensionRegistry, ExtractedExtension};

static BUNDLED: OnceLock<ExtensionRegistry> = OnceLock::new();

pub fn bundled() -> &'static ExtensionRegistry {
    BUNDLED.get_or_init(ExtensionRegistry::load_bundled)
}

pub fn ui() -> &'static ExtractedExtension {
    bundled().get("ui").expect("ui extension must be bundled")
}
