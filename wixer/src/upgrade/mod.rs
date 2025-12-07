//! WiX v3 to v4 upgrade module
//!
//! This module provides a transformation pipeline for upgrading WiX v3 XML files to v4.
//! It handles preprocessor directives by extracting them before parsing and restoring after.

mod preprocessor;
mod transformations;
mod pipeline;

pub use pipeline::upgrade_file;
pub use preprocessor::PreprocessorPreserver;
pub use transformations::Transformation;
