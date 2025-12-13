//! Custom Action Type Analysis
//!
//! Utilities for interpreting MSI custom action type values from PDB data.

use serde_json::Value;
use tracing::debug;

pub struct CustomActionType;

impl CustomActionType {
    // Executable Types
    pub const DLL: i32 = 0x001;
    pub const EXE: i32 = 0x002;
    pub const TEXT_DATA: i32 = 0x003;
    pub const J_SCRIPT: i32 = 0x005;
    pub const VB_SCRIPT: i32 = 0x006;
    pub const INSTALL: i32 = 0x007;

    // Source Types
    pub const SOURCE_FILE: i32 = 0x010;
    pub const DIRECTORY: i32 = 0x020;
    pub const PROPERTY: i32 = 0x030;

    // Return Processing
    pub const CONTINUE: i32 = 0x040;
    pub const ASYNC: i32 = 0x080;

    // Scheduling
    pub const FIRST_SEQUENCE: i32 = 0x100;
    pub const ONCE_PER_PROCESS: i32 = 0x200;
    pub const CLIENT_REPEAT: i32 = 0x300;
    pub const IN_SCRIPT: i32 = 0x400;
    pub const ROLLBACK: i32 = 0x100;
    pub const COMMIT: i32 = 0x200;

    // Security & Other
    pub const NO_IMPERSONATE: i32 = 0x800;
    pub const TS_AWARE: i32 = 0x4000;
    pub const SIXTY_FOUR_BIT: i32 = 0x1000;
    pub const HIDE_TARGET: i32 = 0x2000;
    pub const PATCH_UNINSTALL: i32 = 0x8000;

    // MASKS
    pub const MASK_TARGET: i32 = 0x007;
    pub const MASK_SOURCE: i32 = 0x030;
    pub const MASK_EXECUTION: i32 = 0x700;
}

/// Analyze custom action fields from PDB and return human-readable description
pub fn analyze_launch_app(fields: &[Value]) -> String {
    debug!(field_count = fields.len(), "Analyzing custom action");
    // Fields indices from CustomActionSymbol.cs:
    // [0] ExecutionType (Number)
    // [1] Source (String)
    // [2] SourceType (Number)
    // [3] Target (String)
    // [4] TargetType (Number)
    // [5] Async (Bool)
    // [6] Hidden (Bool)
    // [7] IgnoreResult (Bool)
    // [8] Impersonate (Bool)
    // [9] PatchUninstall (Bool)
    // [10] TSAware (Bool)
    // [11] Win64 (Bool)
    // [12] ScriptFile (Path)

    let get_val = |idx: usize| fields.get(idx).and_then(|f| f.get("data"));

    let source_type = get_val(2).and_then(|v| v.as_i64()).unwrap_or(0);
    let target_type = get_val(4).and_then(|v| v.as_i64()).unwrap_or(0);
    let async_exec = get_val(5).and_then(|v| v.as_bool()).unwrap_or(false);
    let hidden = get_val(6).and_then(|v| v.as_bool()).unwrap_or(false);
    let ignore_result = get_val(7).and_then(|v| v.as_bool()).unwrap_or(false);
    let impersonate = get_val(8).and_then(|v| v.as_bool()).unwrap_or(true);
    let execution_type = get_val(0).and_then(|v| v.as_i64()).unwrap_or(0);
    let win64 = get_val(11).and_then(|v| v.as_bool()).unwrap_or(false);
    let patch_uninstall = get_val(9).and_then(|v| v.as_bool()).unwrap_or(false);

    let mut type_val = 0;

    // Map SourceType
    match source_type {
        16 => type_val |= CustomActionType::SOURCE_FILE,
        32 => type_val |= CustomActionType::DIRECTORY,
        48 => type_val |= CustomActionType::PROPERTY,
        _ => {}
    }

    // Map TargetType
    match target_type {
        1 => type_val |= CustomActionType::DLL,
        2 => type_val |= CustomActionType::EXE,
        3 => type_val |= CustomActionType::TEXT_DATA,
        5 => type_val |= CustomActionType::J_SCRIPT,
        6 => type_val |= CustomActionType::VB_SCRIPT,
        _ => {}
    }

    if async_exec {
        type_val |= CustomActionType::ASYNC;
    }
    if ignore_result {
        type_val |= CustomActionType::CONTINUE;
    }
    if hidden {
        type_val |= CustomActionType::HIDE_TARGET;
    }
    if win64 {
        type_val |= CustomActionType::SIXTY_FOUR_BIT;
    }
    if patch_uninstall {
        type_val |= CustomActionType::PATCH_UNINSTALL;
    }

    // Execution Type
    // ExecutionType enum: FirstSequence=256, OncePerProcess=512, ClientRepeat=768, Deferred=1024, Rollback=1280, Commit=1536
    match execution_type {
        256 => type_val |= CustomActionType::FIRST_SEQUENCE,
        512 => type_val |= CustomActionType::ONCE_PER_PROCESS,
        768 => type_val |= CustomActionType::CLIENT_REPEAT,
        1024 => type_val |= CustomActionType::IN_SCRIPT,
        1280 => type_val |= CustomActionType::IN_SCRIPT | CustomActionType::ROLLBACK,
        1536 => type_val |= CustomActionType::IN_SCRIPT | CustomActionType::COMMIT,
        _ => {} // Immediate
    }

    // Impersonation (only applies if InScript)
    if (type_val & CustomActionType::IN_SCRIPT) != 0 && !impersonate {
        type_val |= CustomActionType::NO_IMPERSONATE;
    }

    // Generate description
    let mut flags = Vec::new();

    match type_val & CustomActionType::MASK_TARGET {
        CustomActionType::EXE => flags.push("EXE"),
        CustomActionType::DLL => flags.push("DLL"),
        CustomActionType::J_SCRIPT => flags.push("JScript"),
        CustomActionType::VB_SCRIPT => flags.push("VBScript"),
        CustomActionType::TEXT_DATA => flags.push("TextData"),
        _ => {}
    }

    match type_val & CustomActionType::MASK_SOURCE {
        CustomActionType::SOURCE_FILE => flags.push("Source:File"),
        CustomActionType::DIRECTORY => flags.push("Source:Directory"),
        CustomActionType::PROPERTY => flags.push("Source:Property"),
        _ => {}
    }

    if (type_val & CustomActionType::ASYNC) != 0 {
        flags.push("Async");
    }
    if (type_val & CustomActionType::CONTINUE) != 0 {
        flags.push("IgnoreResult");
    }

    if (type_val & CustomActionType::IN_SCRIPT) != 0 {
        flags.push("Deferred");
        if (type_val & CustomActionType::NO_IMPERSONATE) != 0 {
            flags.push("NoImpersonate");
        }
    }

    if (type_val & CustomActionType::ROLLBACK) != 0 {
        flags.push("Rollback");
    }
    if (type_val & CustomActionType::COMMIT) != 0 {
        flags.push("Commit");
    }

    format!(
        "Calculated MSI Type: {} ({:#x})\nBehavior: {}",
        type_val,
        type_val,
        flags.join(" | ")
    )
}
