//! WiX v3 to v4 upgrade module
//!
//! This module provides a transformation pipeline for upgrading WiX v3 XML files to v4.
//! It handles preprocessor directives by extracting them before parsing and restoring after.
//!
//! For multi-file projects, use `upgrade_files` which collects cross-file context
//! (e.g., WIXUI_INSTALLDIR property in one file, UIRef in another).

mod context;
mod pipeline;
mod preprocessor;
mod transformations;

pub use context::{UpgradeContext, analyze_files};
pub use pipeline::{upgrade_content_with_context, upgrade_file, upgrade_files};
pub use preprocessor::PreprocessorPreserver;
pub use transformations::Transformation;
pub use transformations::{
    ConditionToLaunch, ControlConditionToAttribute, ControlTextToAttribute, CustomActionTransform,
    InnerTextToCondition, RemoveDeprecatedAttributes, SkipControlTextAndCondition,
    SkipWixUIInstallDirProperty, UIRefToWixUI, V4_NAMESPACE, V4_UI_NAMESPACE,
    standard_transformations,
};
