use crate::agent::types::{ToolResultEnvelope, ToolResultError};
use anyhow::Result;
use serde_json::Value;

fn to_pretty_json(envelope: &ToolResultEnvelope) -> Result<String> {
    Ok(serde_json::to_string_pretty(envelope)?)
}

pub fn envelope(
    tool: &str,
    summary: impl Into<String>,
    data: Option<Value>,
    error: Option<ToolResultError>,
    artifacts: Vec<Value>,
) -> Result<String> {
    let summary = summary.into();
    let details = data.clone();
    let error_code = error.as_ref().map(|item| item.code.clone());
    let error_message = error.as_ref().map(|item| item.message.clone());

    to_pretty_json(&ToolResultEnvelope {
        ok: error.is_none(),
        tool: tool.to_string(),
        summary,
        data,
        error,
        artifacts,
        details,
        error_code,
        error_message,
    })
}

pub fn success(tool: &str, summary: impl Into<String>, details: Value) -> Result<String> {
    envelope(tool, summary, Some(details), None, Vec::new())
}

pub fn failure(
    tool: &str,
    summary: impl Into<String>,
    error_code: impl Into<String>,
    error_message: impl Into<String>,
    details: Value,
) -> Result<String> {
    envelope(
        tool,
        summary,
        Some(details),
        Some(ToolResultError {
            code: error_code.into(),
            message: error_message.into(),
        }),
        Vec::new(),
    )
}
