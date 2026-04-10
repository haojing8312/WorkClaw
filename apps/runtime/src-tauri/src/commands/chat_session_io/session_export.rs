use super::session_view::render_user_content_parts;
use crate::agent::runtime::task_lineage::{
    build_task_path, effective_task_identity, project_task_graph_nodes, SessionRunTaskGraphNode,
};
use crate::session_journal::{
    SessionJournalState, SessionJournalStore, SessionRunEvent, SessionRunSnapshot,
    SessionRunStatus, SessionRunTurnStateSnapshot, SessionTaskRecordSnapshot,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct ExportToolCall {
    call_id: String,
    name: String,
    input: Value,
    output: String,
    status: String,
}

#[derive(Debug, Clone)]
struct ExportRunStopSummary {
    title: String,
    detail: Option<String>,
    last_completed_step: Option<String>,
}

pub(crate) async fn export_session_markdown_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    journal: Option<&SessionJournalStore>,
) -> Result<String, String> {
    let (title,): (String,) = sqlx::query_as("SELECT title FROM sessions WHERE id = ?")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    let messages = sqlx::query_as::<_, (String, String, Option<String>, String, Option<String>)>(
        "SELECT
            m.role,
            m.content,
            m.content_json,
            m.created_at,
            NULLIF(sr.id, '') AS run_id
         FROM messages m
         LEFT JOIN session_runs sr ON sr.assistant_message_id = m.id
         WHERE m.session_id = ?
         ORDER BY m.created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let tool_calls_by_run = load_export_tool_calls_with_pool(pool, session_id).await?;
    let run_stop_summaries_by_run =
        load_export_run_stop_summaries_with_pool(pool, session_id).await?;
    let assistant_run_ids_in_messages: HashSet<String> = messages
        .iter()
        .filter_map(|(role, _, _, _, run_id)| {
            if role == "assistant" {
                run_id.as_ref().map(|value| value.to_string())
            } else {
                None
            }
        })
        .collect();

    let mut md = format!("# {}\n\n", title);
    for (role, content, content_json, created_at, run_id) in &messages {
        let label = if role == "user" { "用户" } else { "助手" };
        let tool_calls = run_id
            .as_ref()
            .and_then(|value| tool_calls_by_run.get(value))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let rendered_content =
            render_export_message_content(role, content, content_json.as_deref(), tool_calls);
        md.push_str(&format!(
            "## {} ({})\n\n{}\n\n---\n\n",
            label, created_at, rendered_content
        ));
    }

    if let Some(journal_store) = journal {
        if let Ok(state) = journal_store.read_state(session_id).await {
            let recovered = render_recovered_run_sections(
                &messages,
                &state,
                &tool_calls_by_run,
                &run_stop_summaries_by_run,
                &assistant_run_ids_in_messages,
            );
            if !recovered.is_empty() {
                md.push_str("## 恢复的运行记录\n\n");
                md.push_str(&recovered);
            }
        }
    }

    Ok(md)
}

pub(crate) fn write_export_file_to_path(path: &str, content: &str) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| format!("写入失败: {}", e))
}

async fn load_export_tool_calls_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<HashMap<String, Vec<ExportToolCall>>, String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT run_id, payload_json
         FROM session_run_events
         WHERE session_id = ? AND event_type IN ('tool_started', 'tool_completed')
         ORDER BY created_at ASC, id ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut by_run: HashMap<String, Vec<ExportToolCall>> = HashMap::new();
    for (run_id, payload_json) in rows {
        let Ok(event) = serde_json::from_str::<SessionRunEvent>(&payload_json) else {
            continue;
        };
        let entries = by_run.entry(run_id).or_default();
        match event {
            SessionRunEvent::ToolStarted {
                call_id,
                tool_name,
                input,
                ..
            } => {
                if let Some(existing) = entries.iter_mut().find(|entry| entry.call_id == call_id) {
                    existing.name = tool_name;
                    existing.input = input;
                    existing.status = "running".to_string();
                } else {
                    entries.push(ExportToolCall {
                        call_id,
                        name: tool_name,
                        input,
                        output: String::new(),
                        status: "running".to_string(),
                    });
                }
            }
            SessionRunEvent::ToolCompleted {
                call_id,
                tool_name,
                input,
                output,
                is_error,
                ..
            } => {
                let status = if is_error { "error" } else { "completed" }.to_string();
                if let Some(existing) = entries.iter_mut().find(|entry| entry.call_id == call_id) {
                    existing.name = tool_name;
                    existing.input = input;
                    existing.output = output;
                    existing.status = status;
                } else {
                    entries.push(ExportToolCall {
                        call_id,
                        name: tool_name,
                        input,
                        output,
                        status,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(by_run)
}

async fn load_export_run_stop_summaries_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<HashMap<String, ExportRunStopSummary>, String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT run_id, payload_json
         FROM session_run_events
         WHERE session_id = ? AND event_type = 'run_stopped'
         ORDER BY created_at ASC, id ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut by_run = HashMap::new();
    for (run_id, payload_json) in rows {
        let Ok(event) = serde_json::from_str::<SessionRunEvent>(&payload_json) else {
            continue;
        };
        if let SessionRunEvent::RunStopped { stop_reason, .. } = event {
            by_run.insert(
                run_id,
                ExportRunStopSummary {
                    title: stop_reason.title,
                    detail: stop_reason.detail,
                    last_completed_step: stop_reason.last_completed_step,
                },
            );
        }
    }

    Ok(by_run)
}

fn render_export_message_content(
    role: &str,
    content: &str,
    content_json: Option<&str>,
    supplemental_tool_calls: &[ExportToolCall],
) -> String {
    if role == "user" {
        return content_json
            .and_then(render_user_content_parts)
            .unwrap_or_else(|| content.to_string());
    }

    if role != "assistant" {
        return content.to_string();
    }

    let mut sections: Vec<String> = Vec::new();
    if let Ok(parsed) = serde_json::from_str::<Value>(content) {
        let final_text = parsed["text"].as_str().unwrap_or("").trim();
        if !final_text.is_empty() {
            sections.push(final_text.to_string());
        }

        if let Some(items) = parsed["items"].as_array() {
            for item in items {
                match item["type"].as_str() {
                    Some("text") => {
                        if let Some(text) = item["content"]
                            .as_str()
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                        {
                            if !sections.iter().any(|section| section.contains(text)) {
                                sections.push(text.to_string());
                            }
                        }
                    }
                    Some("tool_call") => {
                        if let Some(tool_section) = render_export_tool_call(item.get("toolCall")) {
                            push_unique_export_section(&mut sections, tool_section);
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(tool_calls) = parsed["tool_calls"].as_array() {
            for item in tool_calls {
                if let Some(tool_section) = render_export_tool_call(Some(item)) {
                    push_unique_export_section(&mut sections, tool_section);
                }
            }
        }
    } else if !content.trim().is_empty() {
        sections.push(content.trim().to_string());
    }

    for tool_call in supplemental_tool_calls {
        if let Some(tool_section) = render_export_tool_call_entry(tool_call) {
            push_unique_export_section(&mut sections, tool_section);
        }
    }

    if sections.is_empty() {
        content.to_string()
    } else {
        sections.join("\n\n")
    }
}

fn push_unique_export_section(sections: &mut Vec<String>, section: String) {
    if !section.trim().is_empty() && !sections.iter().any(|existing| existing == &section) {
        sections.push(section);
    }
}

fn render_export_tool_call(tool_call: Option<&Value>) -> Option<String> {
    let tool_call = tool_call?;

    let name = tool_call["name"]
        .as_str()
        .or_else(|| tool_call["function"]["name"].as_str())
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        return None;
    }

    let input = if tool_call["input"].is_object() {
        tool_call["input"].clone()
    } else if let Some(arguments) = tool_call["function"]["arguments"].as_str() {
        serde_json::from_str::<Value>(arguments).unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    let output = tool_call["output"].as_str().unwrap_or("").trim();
    let status = tool_call["status"].as_str().unwrap_or("").trim();
    let structured_output = parse_export_tool_output(output);

    let mut lines = vec![format!("**工具调用** `{}`", name)];
    if let Some(path) = read_tool_call_path(&input, structured_output.as_ref()) {
        lines.push(format!("- 路径：`{}`", path));
    }
    if !status.is_empty() {
        lines.push(format!(
            "- 状态：{}",
            render_export_tool_status(status, output, structured_output.as_ref())
        ));
    }
    if let Some(summary) = structured_output
        .as_ref()
        .and_then(|value| value["summary"].as_str())
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("- 摘要：{}", summary.trim()));
    }
    if !output.is_empty() {
        if let Some(rendered_output) = render_export_tool_output(structured_output.as_ref(), output)
        {
            lines.push("```text".to_string());
            lines.push(rendered_output);
            lines.push("```".to_string());
        }
    }

    Some(lines.join("\n"))
}

fn render_export_tool_call_entry(tool_call: &ExportToolCall) -> Option<String> {
    let tool_call_value = json!({
        "name": tool_call.name,
        "input": tool_call.input,
        "output": tool_call.output,
        "status": tool_call.status,
    });
    render_export_tool_call(Some(&tool_call_value))
}

fn read_tool_call_path<'a>(
    input: &'a Value,
    structured_output: Option<&'a Value>,
) -> Option<&'a str> {
    input["path"]
        .as_str()
        .or_else(|| input["file_path"].as_str())
        .or_else(|| {
            structured_output
                .and_then(|value| value.get("details"))
                .and_then(|details| details.get("path"))
                .and_then(Value::as_str)
        })
        .filter(|value| !value.trim().is_empty())
}

fn render_export_tool_status(
    status: &str,
    output: &str,
    structured_output: Option<&Value>,
) -> &'static str {
    if structured_output
        .and_then(|value| value["ok"].as_bool())
        .is_some_and(|ok| !ok)
    {
        return "错误";
    }
    if status.eq_ignore_ascii_case("error")
        || output.contains("工具执行错误")
        || output.contains("工具参数错误")
        || output.contains("工具执行线程异常")
    {
        "错误"
    } else if status.eq_ignore_ascii_case("running") {
        "进行中"
    } else {
        "已完成"
    }
}

fn parse_export_tool_output(output: &str) -> Option<Value> {
    let trimmed = output.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let parsed: Value = serde_json::from_str(trimmed).ok()?;
    if parsed.get("summary").is_some()
        || parsed.get("details").is_some()
        || parsed.get("error_code").is_some()
    {
        Some(parsed)
    } else {
        None
    }
}

fn render_export_tool_output(
    structured_output: Option<&Value>,
    raw_output: &str,
) -> Option<String> {
    if let Some(value) = structured_output {
        let summary = value["summary"].as_str().unwrap_or("").trim();
        let error_message = value["error_message"].as_str().unwrap_or("").trim();
        let mut parts = Vec::new();
        if !summary.is_empty() {
            parts.push(summary.to_string());
        }
        if !error_message.is_empty() && error_message != summary {
            parts.push(error_message.to_string());
        }
        if let Some(details) = value.get("details") {
            let compact_details = compact_export_tool_details(details);
            if !compact_details.is_empty() {
                parts.extend(compact_details);
            }
        }
        if !parts.is_empty() {
            return Some(parts.join("\n"));
        }
    }

    let trimmed = raw_output.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn compact_export_tool_details(details: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(path) = details
        .get("path")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        lines.push(format!("path: {}", path.trim()));
    }
    if let Some(destination) = details
        .get("destination")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        lines.push(format!("destination: {}", destination.trim()));
    }
    if let Some(bytes_written) = details.get("bytes_written").and_then(Value::as_u64) {
        lines.push(format!("bytes_written: {}", bytes_written));
    }
    if let Some(exit_code) = details.get("exit_code").and_then(Value::as_i64) {
        lines.push(format!("exit_code: {}", exit_code));
    }
    if let Some(stdout) = details
        .get("stdout")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        lines.push(format!("stdout: {}", stdout.trim()));
    }
    if let Some(stderr) = details
        .get("stderr")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        lines.push(format!("stderr: {}", stderr.trim()));
    }
    lines
}

fn render_recovered_run_sections(
    messages: &[(String, String, Option<String>, String, Option<String>)],
    state: &SessionJournalState,
    tool_calls_by_run: &HashMap<String, Vec<ExportToolCall>>,
    run_stop_summaries_by_run: &HashMap<String, ExportRunStopSummary>,
    assistant_run_ids_in_messages: &HashSet<String>,
) -> String {
    let assistant_contents: Vec<String> = messages
        .iter()
        .filter_map(|(role, content, content_json, _, run_id)| {
            if role != "assistant" {
                return None;
            }
            let tool_calls = run_id
                .as_ref()
                .and_then(|value| tool_calls_by_run.get(value))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            Some(render_export_message_content(
                role,
                content,
                content_json.as_deref(),
                tool_calls,
            ))
        })
        .collect();

    let recovered_task_graph = project_task_graph_nodes(
        state
            .runs
            .iter()
            .filter_map(|run| effective_recovered_task_identity(run)),
    );
    let mut sections = Vec::new();
    if !recovered_task_graph.is_empty() {
        sections.extend(render_recovered_task_graph_section(&recovered_task_graph));
    }
    for run in &state.runs {
        let buffered = run.buffered_text.trim();
        let error_message = run.last_error_message.as_deref().unwrap_or("").trim();
        let tool_sections: Vec<String> = tool_calls_by_run
            .get(&run.run_id)
            .map(|tool_calls| {
                tool_calls
                    .iter()
                    .filter_map(render_export_tool_call_entry)
                    .collect()
            })
            .unwrap_or_default();
        let buffered_already_exported = !buffered.is_empty()
            && assistant_contents
                .iter()
                .any(|content| content.contains(buffered));
        let error_already_exported = !error_message.is_empty()
            && assistant_contents
                .iter()
                .any(|content| content.contains(error_message));
        let missing_assistant_message_for_run =
            !assistant_run_ids_in_messages.contains(&run.run_id);
        let should_recover = missing_assistant_message_for_run
            && ((!buffered.is_empty() && !buffered_already_exported)
                || (!error_message.is_empty() && !error_already_exported)
                || !tool_sections.is_empty()
                || matches!(
                    &run.status,
                    SessionRunStatus::Failed | SessionRunStatus::Cancelled
                ));

        if !should_recover {
            continue;
        }

        sections.push(format!(
            "### Run {} ({})",
            run.run_id,
            export_status_label(&run.status)
        ));
        sections.push(String::new());
        if !buffered.is_empty() && !buffered_already_exported {
            sections.push("#### 已保留的部分输出".to_string());
            sections.push(String::new());
            sections.push(buffered.to_string());
            sections.push(String::new());
        }
        if let Some(error_kind) = &run.last_error_kind {
            if !error_kind.trim().is_empty() {
                sections.push(format!("- error_kind: {}", error_kind));
            }
        }
        if let Some(task_identity) = effective_recovered_task_identity(run) {
            sections.extend(render_recovered_task_identity_lines(task_identity));
        }
        if let Some(task_record) = resolve_recovered_task_record(state, run) {
            sections.extend(render_recovered_task_record_lines(task_record));
        }
        if let Some(summary) = run_stop_summaries_by_run.get(&run.run_id) {
            if !summary.title.trim().is_empty() {
                sections.push(format!("- 停止原因：{}", summary.title.trim()));
            }
            if let Some(detail) = summary.detail.as_deref().map(str::trim) {
                if !detail.is_empty() {
                    sections.push(format!("- 停止详情：{}", detail));
                }
            }
            if let Some(step) = summary.last_completed_step.as_deref().map(str::trim) {
                if !step.is_empty() && !error_message.contains(step) {
                    sections.push(format!("- 最后完成步骤：{}", step));
                }
            }
        }
        if !error_message.is_empty() && !error_already_exported {
            sections.push(format!("- error_message: {}", error_message));
        }
        if let Some(turn_state) = run.turn_state.as_ref() {
            sections.extend(render_recovered_turn_state_lines(turn_state));
        }
        if missing_assistant_message_for_run {
            for tool_section in tool_sections {
                sections.push(String::new());
                sections.push(tool_section);
            }
        }
        sections.push("\n---\n".to_string());
    }

    sections.join("\n")
}

fn render_recovered_task_graph_section(task_graph: &[SessionRunTaskGraphNode]) -> Vec<String> {
    if task_graph.is_empty() {
        return Vec::new();
    }

    let mut lines = vec!["#### 任务链路".to_string(), String::new()];
    for node in task_graph {
        lines.push(format!(
            "- {} ({}): {}",
            node.task_kind, node.surface_kind, node.task_path
        ));
    }
    lines.push(String::new());
    lines
}

fn render_recovered_task_identity_lines(
    task_identity: &crate::session_journal::SessionRunTaskIdentitySnapshot,
) -> Vec<String> {
    let mut lines = Vec::new();
    if !task_identity.task_id.trim().is_empty() {
        lines.push(format!("- task_id: {}", task_identity.task_id.trim()));
    }
    if let Some(parent_task_id) = task_identity.parent_task_id.as_deref().map(str::trim) {
        if !parent_task_id.is_empty() {
            lines.push(format!("- parent_task_id: {}", parent_task_id));
        }
    }
    if !task_identity.root_task_id.trim().is_empty() {
        lines.push(format!(
            "- root_task_id: {}",
            task_identity.root_task_id.trim()
        ));
    }
    if let Some(task_path) = build_task_path(task_identity) {
        lines.push(format!("- task_path: {}", task_path));
    }
    if !task_identity.task_kind.trim().is_empty() {
        lines.push(format!("- task_kind: {}", task_identity.task_kind.trim()));
    }
    if !task_identity.surface_kind.trim().is_empty() {
        lines.push(format!(
            "- surface_kind: {}",
            task_identity.surface_kind.trim()
        ));
    }
    if !task_identity.backend_kind.trim().is_empty() {
        lines.push(format!(
            "- backend_kind: {}",
            task_identity.backend_kind.trim()
        ));
    }
    lines
}

fn effective_recovered_task_identity(
    run: &SessionRunSnapshot,
) -> Option<&crate::session_journal::SessionRunTaskIdentitySnapshot> {
    effective_task_identity(run.task_identity.as_ref(), run.turn_state.as_ref())
}

fn resolve_recovered_task_record<'a>(
    state: &'a SessionJournalState,
    run: &SessionRunSnapshot,
) -> Option<&'a SessionTaskRecordSnapshot> {
    let effective_identity = effective_recovered_task_identity(run);

    if let Some(task_identity) = effective_identity {
        return state
            .tasks
            .iter()
            .rev()
            .find(|task| task.task_identity.task_id == task_identity.task_id);
    }

    state
        .tasks
        .iter()
        .rev()
        .find(|task| task.run_id == run.run_id)
}

fn render_recovered_task_record_lines(task_record: &SessionTaskRecordSnapshot) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("- task_status: {}", task_record.status.as_key()));
    if !task_record.created_at.trim().is_empty() {
        lines.push(format!(
            "- task_created_at: {}",
            task_record.created_at.trim()
        ));
    }
    if !task_record.updated_at.trim().is_empty() {
        lines.push(format!(
            "- task_updated_at: {}",
            task_record.updated_at.trim()
        ));
    }
    if let Some(started_at) = task_record.started_at.as_ref() {
        let started_at = started_at.trim();
        if !started_at.is_empty() {
            lines.push(format!("- task_started_at: {}", started_at));
        }
    }
    if let Some(completed_at) = task_record.completed_at.as_ref() {
        let completed_at = completed_at.trim();
        if !completed_at.is_empty() {
            lines.push(format!("- task_completed_at: {}", completed_at));
        }
    }
    if let Some(terminal_reason) = task_record.terminal_reason.as_ref() {
        let terminal_reason = terminal_reason.trim();
        if !terminal_reason.is_empty() {
            lines.push(format!("- task_terminal_reason: {}", terminal_reason));
        }
    }
    if !task_record.task_identity.backend_kind.trim().is_empty() {
        lines.push(format!(
            "- task_backend_kind: {}",
            task_record.task_identity.backend_kind.trim()
        ));
    }
    lines
}

fn render_recovered_turn_state_lines(turn_state: &SessionRunTurnStateSnapshot) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(compaction_boundary) = turn_state.compaction_boundary.as_ref() {
        lines.push(format!(
            "- 压缩边界：{} -> {}",
            compaction_boundary.original_tokens, compaction_boundary.compacted_tokens
        ));
        if !compaction_boundary.transcript_path.trim().is_empty() {
            lines.push(format!(
                "- 压缩转录：{}",
                compaction_boundary.transcript_path.trim()
            ));
        }
        if !compaction_boundary.summary.trim().is_empty() {
            lines.push(format!(
                "- 压缩摘要：{}",
                compaction_boundary.summary.trim()
            ));
        }
    }
    if let Some(reconstructed_history_len) = turn_state.reconstructed_history_len {
        lines.push(format!("- 重建历史消息数：{}", reconstructed_history_len));
    }
    lines
}

fn export_status_label(status: &SessionRunStatus) -> &'static str {
    match status {
        SessionRunStatus::Queued => "queued",
        SessionRunStatus::Thinking => "thinking",
        SessionRunStatus::ToolCalling => "tool_calling",
        SessionRunStatus::WaitingApproval => "waiting_approval",
        SessionRunStatus::WaitingUser => "waiting_user",
        SessionRunStatus::Completed => "completed",
        SessionRunStatus::Failed => "failed",
        SessionRunStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::render_recovered_run_sections;
    use crate::session_journal::{
        SessionJournalState, SessionRunSnapshot, SessionRunStatus, SessionRunTaskIdentitySnapshot,
    };
    use std::collections::{HashMap, HashSet};

    #[test]
    fn recovered_run_sections_include_task_identity_lines() {
        let output = render_recovered_run_sections(
            &[],
            &SessionJournalState {
                session_id: "session-1".to_string(),
                current_run_id: None,
                runs: vec![SessionRunSnapshot {
                    run_id: "run-1".to_string(),
                    user_message_id: "user-1".to_string(),
                    status: SessionRunStatus::Failed,
                    buffered_text: "保留输出".to_string(),
                    last_error_kind: Some("max_turns".to_string()),
                    last_error_message: Some("已达到执行步数上限".to_string()),
                    task_identity: Some(SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    }),
                    task_continuation_mode: None,
                    task_continuation_reason: None,
                    turn_state: None,
                }],
                tasks: vec![],
            },
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
        );

        assert!(output.contains("task_id: task-1"));
        assert!(output.contains("task_kind: primary_user_task"));
        assert!(output.contains("surface_kind: local_chat_surface"));
        assert!(output.contains("backend_kind: interactive_chat_backend"));
        assert!(output.contains("task_path: task-1"));
    }

    #[test]
    fn recovered_run_sections_fall_back_to_turn_state_task_identity_lines() {
        let output = render_recovered_run_sections(
            &[],
            &SessionJournalState {
                session_id: "session-1".to_string(),
                current_run_id: None,
                runs: vec![SessionRunSnapshot {
                    run_id: "run-1".to_string(),
                    user_message_id: "user-1".to_string(),
                    status: SessionRunStatus::Failed,
                    buffered_text: "保留输出".to_string(),
                    last_error_kind: Some("max_turns".to_string()),
                    last_error_message: Some("已达到执行步数上限".to_string()),
                    task_identity: None,
                    task_continuation_mode: None,
                    task_continuation_reason: None,
                    turn_state: Some(crate::session_journal::SessionRunTurnStateSnapshot {
                        task_identity: Some(SessionRunTaskIdentitySnapshot {
                            task_id: "task-child".to_string(),
                            parent_task_id: Some("task-parent".to_string()),
                            root_task_id: "task-root".to_string(),
                            task_kind: "sub_agent_task".to_string(),
                            surface_kind: "hidden_child_surface".to_string(),
                            backend_kind: "hidden_child_backend".to_string(),
                        }),
                        session_surface: None,
                        execution_lane: None,
                        selected_runner: None,
                        selected_skill: None,
                        fallback_reason: None,
                        allowed_tools: Vec::new(),
                        invoked_skills: Vec::new(),
                        partial_assistant_text: String::new(),
                        tool_failure_streak: 0,
                        reconstructed_history_len: None,
                        compaction_boundary: None,
                    }),
                }],
                tasks: vec![],
            },
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
        );

        assert!(output.contains("task_id: task-child"));
        assert!(output.contains("parent_task_id: task-parent"));
        assert!(output.contains("backend_kind: hidden_child_backend"));
        assert!(output.contains("task_path: task-root -> task-parent -> task-child"));
    }

    #[test]
    fn recovered_run_sections_include_task_graph_section() {
        let output = render_recovered_run_sections(
            &[],
            &SessionJournalState {
                session_id: "session-1".to_string(),
                current_run_id: None,
                runs: vec![
                    SessionRunSnapshot {
                        run_id: "run-1".to_string(),
                        user_message_id: "user-1".to_string(),
                        status: SessionRunStatus::Failed,
                        buffered_text: "保留输出".to_string(),
                        last_error_kind: Some("max_turns".to_string()),
                        last_error_message: Some("已达到执行步数上限".to_string()),
                        task_identity: Some(SessionRunTaskIdentitySnapshot {
                            task_id: "task-root".to_string(),
                            parent_task_id: None,
                            root_task_id: "task-root".to_string(),
                            task_kind: "primary_user_task".to_string(),
                            surface_kind: "local_chat_surface".to_string(),
                            backend_kind: "interactive_chat_backend".to_string(),
                        }),
                        task_continuation_mode: None,
                        task_continuation_reason: None,
                        turn_state: None,
                    },
                    SessionRunSnapshot {
                        run_id: "run-2".to_string(),
                        user_message_id: "user-2".to_string(),
                        status: SessionRunStatus::Failed,
                        buffered_text: "子任务输出".to_string(),
                        last_error_kind: Some("max_turns".to_string()),
                        last_error_message: Some("子任务停止".to_string()),
                        task_identity: None,
                        task_continuation_mode: None,
                        task_continuation_reason: None,
                        turn_state: Some(crate::session_journal::SessionRunTurnStateSnapshot {
                            task_identity: Some(SessionRunTaskIdentitySnapshot {
                                task_id: "task-child".to_string(),
                                parent_task_id: Some("task-root".to_string()),
                                root_task_id: "task-root".to_string(),
                                task_kind: "sub_agent_task".to_string(),
                                surface_kind: "hidden_child_surface".to_string(),
                                backend_kind: "hidden_child_backend".to_string(),
                            }),
                            session_surface: None,
                            execution_lane: None,
                            selected_runner: None,
                            selected_skill: None,
                            fallback_reason: None,
                            allowed_tools: Vec::new(),
                            invoked_skills: Vec::new(),
                            partial_assistant_text: String::new(),
                            tool_failure_streak: 0,
                            reconstructed_history_len: None,
                            compaction_boundary: None,
                        }),
                    },
                ],
                tasks: vec![],
            },
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
        );

        assert!(output.contains("#### 任务链路"));
        assert!(output.contains("primary_user_task (local_chat_surface): task-root"));
        assert!(output.contains("sub_agent_task (hidden_child_surface): task-root -> task-child"));
    }

    #[test]
    fn recovered_run_sections_include_task_record_lines() {
        let output = render_recovered_run_sections(
            &[],
            &SessionJournalState {
                session_id: "session-1".to_string(),
                current_run_id: None,
                runs: vec![SessionRunSnapshot {
                    run_id: "run-1".to_string(),
                    user_message_id: "user-1".to_string(),
                    status: SessionRunStatus::Failed,
                    buffered_text: "保留输出".to_string(),
                    last_error_kind: Some("max_turns".to_string()),
                    last_error_message: Some("已达到执行步数上限".to_string()),
                    task_identity: Some(SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    }),
                    task_continuation_mode: None,
                    task_continuation_reason: None,
                    turn_state: None,
                }],
                tasks: vec![crate::session_journal::SessionTaskRecordSnapshot {
                    task_identity: SessionRunTaskIdentitySnapshot {
                        task_id: "task-1".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-1".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    },
                    session_id: "session-1".to_string(),
                    user_message_id: "user-1".to_string(),
                    run_id: "run-1".to_string(),
                    status: crate::agent::runtime::task_record::TaskLifecycleStatus::Failed,
                    created_at: "2026-04-09T00:00:00Z".to_string(),
                    updated_at: "2026-04-09T00:00:02Z".to_string(),
                    started_at: Some("2026-04-09T00:00:01Z".to_string()),
                    completed_at: Some("2026-04-09T00:00:02Z".to_string()),
                    terminal_reason: Some("max_turns".to_string()),
                }],
            },
            &HashMap::new(),
            &HashMap::new(),
            &HashSet::new(),
        );

        assert!(output.contains("task_status: failed"));
        assert!(output.contains("task_backend_kind: interactive_chat_backend"));
        assert!(output.contains("task_started_at: 2026-04-09T00:00:01Z"));
        assert!(output.contains("task_completed_at: 2026-04-09T00:00:02Z"));
        assert!(output.contains("task_terminal_reason: max_turns"));
    }
}
