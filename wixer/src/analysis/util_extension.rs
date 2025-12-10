use serde_json::Value;

pub struct CloseAppAttributes;

impl CloseAppAttributes {
    pub const CLOSE_MESSAGE: i32 = 0x1;
    pub const REBOOT_PROMPT: i32 = 0x2;
    pub const ELEVATED_CLOSE_MESSAGE: i32 = 0x4;
    pub const END_SESSION_MESSAGE: i32 = 0x8;
    pub const ELEVATED_END_SESSION_MESSAGE: i32 = 0x10;
    pub const TERMINATE_PROCESS: i32 = 0x20;
    pub const PROMPT_TO_CONTINUE: i32 = 0x40;
}

pub fn analyze_close_app(fields: &[Value]) -> String {
    // Fields from WixCloseApplicationSymbol.cs:
    // [0] Target (String)
    // [1] Description (String)
    // [2] Condition (String)
    // [3] Attributes (Number)
    // [4] Sequence (Number)
    // [5] Property (String)
    // [6] TerminateExitCode (Number)
    // [7] Timeout (Number)

    let get_val = |idx: usize| fields.get(idx).and_then(|f| f.get("data"));

    let target = get_val(0).and_then(|v| v.as_str()).unwrap_or("?");
    let attr_val = get_val(3).and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    let mut report = format!("Target Process: {}\nStrategy:\n", target);

    if (attr_val & CloseAppAttributes::PROMPT_TO_CONTINUE) != 0 {
        report.push_str("  1. PROMPT user to close it manually (Abort/Retry/Ignore).\n");
    }

    // Immediate actions
    if (attr_val & CloseAppAttributes::CLOSE_MESSAGE) != 0 {
        report.push_str("  2. Try to close via WM_CLOSE (User Context).\n");
    }
    if (attr_val & CloseAppAttributes::END_SESSION_MESSAGE) != 0 {
        report.push_str("  2. Try to close via WM_QUERYENDSESSION (User Context).\n");
    }

    // Deferred actions
    if (attr_val
        & (CloseAppAttributes::ELEVATED_CLOSE_MESSAGE
            | CloseAppAttributes::ELEVATED_END_SESSION_MESSAGE
            | CloseAppAttributes::TERMINATE_PROCESS))
        != 0
    {
        report.push_str("  3. Schedule DEFERRED action (System Context):\n");

        if (attr_val & CloseAppAttributes::ELEVATED_CLOSE_MESSAGE) != 0 {
            report.push_str("     - Retry closing via WM_CLOSE (Elevated).\n");
        }
        if (attr_val & CloseAppAttributes::ELEVATED_END_SESSION_MESSAGE) != 0 {
            report.push_str("     - Retry closing via WM_QUERYENDSESSION (Elevated).\n");
        }
        if (attr_val & CloseAppAttributes::TERMINATE_PROCESS) != 0 {
            report.push_str("     - FORCE KILL process if still running.\n");
        }
    }

    if (attr_val & CloseAppAttributes::REBOOT_PROMPT) != 0 {
        report.push_str("  4. If still running, require REBOOT at end of install.\n");
    }

    report
}
