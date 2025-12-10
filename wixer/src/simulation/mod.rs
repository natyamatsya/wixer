//! MSI Installer Execution Simulation
//!
//! This module provides a data model for simulating the execution flow of
//! Windows Installer (MSI) packages. It models:
//!
//! - **Sequences**: InstallUISequence, InstallExecuteSequence, etc.
//! - **Actions**: Standard and custom actions with conditions
//! - **Properties**: Installer state that conditions evaluate against
//! - **Installation Modes**: FirstInstall, Upgrade, Uninstall, Maintenance, Repair
//! - **Extensions**: WiX extension actions loaded from bundled JSON data
//!
//! # Example
//!
//! ```ignore
//! use wixer::simulation::{Simulator, InstallMode, SimulationOptions};
//!
//! let mut sim = Simulator::from_wix_project(&project)?;
//! let trace = sim.run(InstallMode::FirstInstall, &SimulationOptions::default())?;
//!
//! for event in trace.events() {
//!     println!("{}: {} ({})", event.sequence_number, event.action, event.result);
//! }
//! ```

mod condition;
pub mod extensions;
mod model;
mod simulator;
mod standard_actions;

pub use condition::*;
pub use extensions::*;
pub use model::*;
pub use simulator::*;
pub use standard_actions::*;
