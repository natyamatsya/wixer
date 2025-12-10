use serde_json::Value;

#[derive(Debug, Clone)]
pub struct WixActionData {
    pub table: String,
    pub action: String,
    pub sequence: Option<i64>,
    pub condition: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
}

pub fn parse_wix_action(id: &str, fields: &[Value]) -> WixActionData {
    let get_val = |idx: usize| fields.get(idx).and_then(|f| f.get("data"));

    // Parse ID to get Table
    let parts: Vec<&str> = id.split('/').collect();
    let table = parts.first().unwrap_or(&"?").to_string();

    let action = get_val(1)
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();
    let condition = get_val(2).and_then(|v| v.as_str()).map(|s| s.to_string());
    let sequence = get_val(3).and_then(|v| v.as_i64());
    let before = get_val(4).and_then(|v| v.as_str()).map(|s| s.to_string());
    let after = get_val(5).and_then(|v| v.as_str()).map(|s| s.to_string());

    WixActionData {
        table,
        action,
        sequence,
        condition,
        before,
        after,
    }
}

pub fn analyze_wix_action(id: &str, fields: &[Value]) -> String {
    let data = parse_wix_action(id, fields);

    let mut report = format!("Sequence Table: {}\nAction: {}\n", data.table, data.action);

    if let Some(seq) = data.sequence {
        report.push_str(&format!("Scheduled Sequence: {}\n", seq));
    } else {
        report.push_str("Scheduled Sequence: <Unresolved>\n");
    }

    if let Some(cond) = data.condition {
        report.push_str(&format!("Condition: {}\n", cond));
    } else {
        report.push_str("Condition: <None>\n");
    }

    if let Some(bef) = data.before {
        report.push_str(&format!("Scheduling Constraint: Before {}\n", bef));
    }
    if let Some(aft) = data.after {
        report.push_str(&format!("Scheduling Constraint: After {}\n", aft));
    }

    // Interpret common sequence ranges
    if let Some(seq) = data.sequence {
        let phase = if seq < 0 {
            "Negative (Exit/Error)"
        } else if seq < 1500 {
            "Initialization / Costing"
        } else if seq < 6600 {
            "File Transfer / System Change"
        } else {
            "Finalization"
        };
        report.push_str(&format!("Execution Phase: {}\n", phase));
    }

    report
}
