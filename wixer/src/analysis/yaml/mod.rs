//! YAML Source Analysis
//!
//! Static analysis modules for WiX YAML source files.
//! These generate diagnostics for the LSP and analyze command.

pub mod condition_validation;
pub mod orphan_detection;
pub mod publish_conflicts;
pub mod scheduling_warnings;
