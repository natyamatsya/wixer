//! Code Analysis
//!
//! Static analysis modules organized by target:
//! - `yaml`: YAML source file analyzers (for LSP diagnostics)
//! - `pdb`: PDB inspection utilities (for `wixer inspect-pdb`)

pub mod pdb;
pub mod yaml;

// Re-export for convenience
pub use pdb::{custom_action, pdb_validation, util_extension, wix_action};
pub use yaml::{condition_validation, orphan_detection, publish_conflicts, scheduling_warnings};
