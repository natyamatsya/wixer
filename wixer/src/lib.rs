//! Wixer library - WiX YAML authoring toolkit
//!
//! This library provides programmatic access to wixer's functionality:
//! - YAML ↔ WiX XML conversion
//! - WiX v3 → v4 upgrade
//! - MSIX migration
//! - Schema validation
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use wixer::commands::{upgrade, import, export};
//! use wixer::common::AttributeStrategy;
//!
//! // Upgrade WiX v3 to v4
//! upgrade::run(Path::new("Product.wxs").into(), None).unwrap();
//!
//! // Convert XML to YAML
//! import::run(
//!     Path::new("./wix"),
//!     Path::new("./yaml"),
//!     AttributeStrategy::Inline,
//! ).unwrap();
//! ```

pub mod analysis;
pub mod commands;
pub mod common;
pub mod config;
pub mod constants;
pub mod describe_db;
pub mod dirs;
pub mod project;
pub mod schema_mapping;
pub mod schema_mapping_v3v4;
pub mod utils;
pub mod validate;
pub mod wix_exit_codes;
pub mod wix_to_yaml;
pub mod wixpdb;
pub mod xsd_to_json;
pub mod yaml_to_wix;

// Re-export commonly used types
pub use common::AttributeStrategy;
pub use config::WixerConfig;
