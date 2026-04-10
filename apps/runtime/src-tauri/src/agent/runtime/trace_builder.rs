use crate::agent::runtime::task_lineage::{
    build_task_path, project_task_graph_nodes, SessionRunTaskGraphNode,
};
use crate::session_journal::SessionRunEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredSessionRunEvent {
    pub session_id: String,
    pub run_id: String,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRunEventSummary {
    pub session_id: String,
    pub run_id: String,
    pub event_type: String,
    pub created_at: String,
    pub status: Option<String>,
    pub tool_name: Option<String>,
    pub call_id: Option<String>,
    pub approval_id: Option<String>,
    pub warning_kind: Option<String>,
    pub error_kind: Option<String>,
    pub message: Option<String>,
    pub detail: Option<String>,
    pub irreversible: Option<bool>,
    pub last_completed_step: Option<String>,
    pub child_session_id: Option<String>,
    pub is_error: Option<bool>,
    pub parse_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRunTraceLifecycle {
    pub started: bool,
    pub completed: bool,
    pub failed: bool,
    pub cancelled: bool,
    pub stopped: bool,
    pub waiting_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunTraceToolSummary {
    pub call_id: String,
    pub tool_name: String,
    pub status: String,
    pub input_preview: Option<String>,
    pub output_preview: Option<String>,
    pub child_session_id: Option<String>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunTraceApprovalSummary {
    pub approval_id: String,
    pub tool_name: String,
    pub call_id: String,
    pub summary: Option<String>,
    pub impact: Option<String>,
    pub irreversible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunTraceGuardWarningSummary {
    pub warning_kind: String,
    pub title: String,
    pub detail: Option<String>,
    pub last_completed_step: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRunChildSessionLink {
    pub parent_session_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRunTrace {
    pub session_id: String,
    pub run_id: String,
    pub final_status: String,
    pub event_count: usize,
    pub first_event_at: Option<String>,
    pub last_event_at: Option<String>,
    pub lifecycle: SessionRunTraceLifecycle,
    pub stop_reason_kind: Option<String>,
    pub tools: Vec<RunTraceToolSummary>,
    pub approvals: Vec<RunTraceApprovalSummary>,
    pub guard_warnings: Vec<RunTraceGuardWarningSummary>,
    pub parse_warnings: Vec<String>,
    pub child_session_link: Option<SessionRunChildSessionLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_graph: Vec<SessionRunTaskGraphNode>,
    pub events: Vec<SessionRunEventSummary>,
}

pub fn summarize_stored_event(record: &StoredSessionRunEvent) -> SessionRunEventSummary {
    match serde_json::from_str::<SessionRunEvent>(&record.payload_json) {
        Ok(event) => summarize_event(record, event),
        Err(error) => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: None,
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: Some(truncate_text(&record.payload_json, 160)),
            detail: None,
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: None,
            parse_warning: Some(format!("failed to parse payload_json: {error}")),
        },
    }
}

fn summarize_event(
    record: &StoredSessionRunEvent,
    event: SessionRunEvent,
) -> SessionRunEventSummary {
    match event {
        SessionRunEvent::TaskContinued { task_identity, .. } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("continued".to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: Some(format!(
                "task={} surface={} continued",
                task_identity.task_kind, task_identity.surface_kind
            )),
            detail: Some(summarize_task_identity_detail(&task_identity)),
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: Some(false),
            parse_warning: None,
        },
        SessionRunEvent::TaskStateProjected { task_identity, .. } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: None,
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: Some(format!(
                "task={} surface={}",
                task_identity.task_kind, task_identity.surface_kind
            )),
            detail: Some(summarize_task_identity_detail(&task_identity)),
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: Some(false),
            parse_warning: None,
        },
        SessionRunEvent::TaskDelegated {
            from_task_id,
            from_task_kind,
            from_surface_kind,
            delegated_task,
            ..
        } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("delegated".to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: Some(format!(
                "task={} surface={} delegated {}",
                from_task_kind, from_surface_kind, delegated_task.task_kind
            )),
            detail: Some(format!(
                "from_task_id={}, delegated_task_id={}, delegated_task_path={}",
                from_task_id,
                delegated_task.task_id,
                build_task_path(&delegated_task).unwrap_or_else(|| delegated_task.task_id.clone())
            )),
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: Some(false),
            parse_warning: None,
        },
        SessionRunEvent::TaskRecordUpserted { task, .. } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some(task.status.as_key().to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: Some(format!(
                "task={} surface={}",
                task.task_kind.journal_key(),
                task.surface_kind.journal_key()
            )),
            detail: Some(format!(
                "task_id={}, root_task_id={}, parent_task_id={}",
                task.task_identity.task_id,
                task.task_identity.root_task_id,
                task.task_identity.parent_task_id.as_deref().unwrap_or("-")
            )),
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: Some(false),
            parse_warning: None,
        },
        SessionRunEvent::TaskStatusChanged { status_change, .. } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some(status_change.to_status.as_key().to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: Some(format!(
                "task={} {} -> {}",
                status_change.task_id,
                status_change.from_status.as_key(),
                status_change.to_status.as_key()
            )),
            detail: status_change.terminal_reason.clone(),
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: Some(matches!(
                status_change.to_status,
                crate::agent::runtime::task_record::TaskLifecycleStatus::Failed
                    | crate::agent::runtime::task_record::TaskLifecycleStatus::Cancelled
            )),
            parse_warning: None,
        },
        SessionRunEvent::RunStarted {
            user_message_id, ..
        } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("thinking".to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: Some(format!("user_message_id={user_message_id}")),
            detail: None,
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: None,
            parse_warning: None,
        },
        SessionRunEvent::SkillRouteRecorded {
            route_latency_ms,
            candidate_count,
            selected_runner,
            selected_skill,
            fallback_reason,
            ..
        } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some(selected_runner.clone()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: fallback_reason,
            error_kind: None,
            message: selected_skill,
            detail: Some(format!(
                "route_latency_ms={route_latency_ms}, candidate_count={candidate_count}"
            )),
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: Some(false),
            parse_warning: None,
        },
        SessionRunEvent::AssistantChunkAppended { chunk, .. } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("thinking".to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: Some(truncate_text(&chunk, 160)),
            detail: None,
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: None,
            parse_warning: None,
        },
        SessionRunEvent::ToolStarted {
            tool_name,
            call_id,
            input,
            ..
        } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("tool_calling".to_string()),
            tool_name: Some(tool_name),
            call_id: Some(call_id),
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: compact_json(&input),
            detail: None,
            irreversible: None,
            last_completed_step: None,
            child_session_id: extract_child_session_id(&input, None),
            is_error: Some(false),
            parse_warning: None,
        },
        SessionRunEvent::ToolCompleted {
            tool_name,
            call_id,
            input,
            output,
            is_error,
            ..
        } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some(if is_error {
                "tool_error".to_string()
            } else {
                "thinking".to_string()
            }),
            tool_name: Some(tool_name),
            call_id: Some(call_id),
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: render_tool_output_preview(&output),
            detail: compact_json(&input),
            irreversible: None,
            last_completed_step: None,
            child_session_id: extract_child_session_id(&input, Some(output.as_str())),
            is_error: Some(is_error),
            parse_warning: None,
        },
        SessionRunEvent::ApprovalRequested {
            approval_id,
            tool_name,
            call_id,
            input,
            summary,
            impact,
            irreversible,
            ..
        } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("waiting_approval".to_string()),
            tool_name: Some(tool_name),
            call_id: Some(call_id),
            approval_id: Some(approval_id),
            warning_kind: None,
            error_kind: None,
            message: Some(summary),
            detail: impact.or_else(|| compact_json(&input)),
            irreversible: Some(irreversible),
            last_completed_step: None,
            child_session_id: extract_child_session_id(&input, None),
            is_error: Some(false),
            parse_warning: None,
        },
        SessionRunEvent::RunCompleted { .. } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("completed".to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: None,
            message: Some("run completed".to_string()),
            detail: None,
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: Some(false),
            parse_warning: None,
        },
        SessionRunEvent::RunGuardWarning {
            warning_kind,
            title,
            message,
            detail,
            last_completed_step,
            ..
        } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: None,
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: Some(warning_kind),
            error_kind: None,
            message: Some(title),
            detail: detail.or(Some(message)),
            irreversible: None,
            last_completed_step,
            child_session_id: None,
            is_error: Some(false),
            parse_warning: None,
        },
        SessionRunEvent::RunStopped { stop_reason, .. } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("stopped".to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: Some(stop_reason.kind.as_key().to_string()),
            message: Some(stop_reason.title),
            detail: Some(stop_reason.message),
            irreversible: None,
            last_completed_step: stop_reason.last_completed_step,
            child_session_id: None,
            is_error: Some(true),
            parse_warning: None,
        },
        SessionRunEvent::RunFailed {
            error_kind,
            error_message,
            ..
        } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("failed".to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: Some(error_kind),
            message: Some(truncate_text(&error_message, 160)),
            detail: None,
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: Some(true),
            parse_warning: None,
        },
        SessionRunEvent::RunCancelled { reason, .. } => SessionRunEventSummary {
            session_id: record.session_id.clone(),
            run_id: record.run_id.clone(),
            event_type: record.event_type.clone(),
            created_at: record.created_at.clone(),
            status: Some("cancelled".to_string()),
            tool_name: None,
            call_id: None,
            approval_id: None,
            warning_kind: None,
            error_kind: Some("cancelled".to_string()),
            message: reason,
            detail: None,
            irreversible: None,
            last_completed_step: None,
            child_session_id: None,
            is_error: Some(true),
            parse_warning: None,
        },
    }
}

pub fn build_session_run_trace(
    session_id: &str,
    run_id: &str,
    events: &[StoredSessionRunEvent],
) -> SessionRunTrace {
    let summaries: Vec<SessionRunEventSummary> =
        events.iter().map(summarize_stored_event).collect();
    let mut lifecycle = SessionRunTraceLifecycle {
        started: false,
        completed: false,
        failed: false,
        cancelled: false,
        stopped: false,
        waiting_approval: false,
    };
    let mut final_status = if summaries.is_empty() {
        "missing".to_string()
    } else {
        "unknown".to_string()
    };
    let mut stop_reason_kind = None;
    let mut tools: Vec<RunTraceToolSummary> = Vec::new();
    let mut approvals = Vec::new();
    let mut guard_warnings = Vec::new();
    let mut parse_warnings = Vec::new();
    let mut observed_task_identities = Vec::new();

    if summaries.is_empty() {
        parse_warnings.push("no persisted session_run_events found for run".to_string());
    }

    for record in events {
        let Ok(event) = serde_json::from_str::<SessionRunEvent>(&record.payload_json) else {
            continue;
        };
        if let Some(task_identity) = extract_task_identity(&event) {
            observed_task_identities.push(task_identity.clone());
        }
    }

    for summary in &summaries {
        if let Some(parse_warning) = &summary.parse_warning {
            parse_warnings.push(parse_warning.clone());
        }

        match summary.event_type.as_str() {
            "run_started" => {
                lifecycle.started = true;
                final_status = "thinking".to_string();
            }
            "assistant_chunk_appended" => {
                if !matches!(
                    final_status.as_str(),
                    "completed" | "failed" | "cancelled" | "stopped"
                ) {
                    final_status = "thinking".to_string();
                }
            }
            "tool_started" => {
                final_status = "tool_calling".to_string();
                upsert_tool_summary(
                    &mut tools,
                    summary.call_id.as_deref().unwrap_or_default(),
                    summary.tool_name.as_deref().unwrap_or_default(),
                    "running",
                    summary.message.clone(),
                    None,
                    summary.child_session_id.clone(),
                    summary.is_error.unwrap_or(false),
                );
            }
            "tool_completed" => {
                if !matches!(
                    final_status.as_str(),
                    "completed" | "failed" | "cancelled" | "stopped"
                ) {
                    final_status = "thinking".to_string();
                }
                upsert_tool_summary(
                    &mut tools,
                    summary.call_id.as_deref().unwrap_or_default(),
                    summary.tool_name.as_deref().unwrap_or_default(),
                    if summary.is_error.unwrap_or(false) {
                        "error"
                    } else {
                        "completed"
                    },
                    summary.detail.clone(),
                    summary.message.clone(),
                    summary.child_session_id.clone(),
                    summary.is_error.unwrap_or(false),
                );
            }
            "approval_requested" => {
                lifecycle.waiting_approval = true;
                final_status = "waiting_approval".to_string();
                approvals.push(RunTraceApprovalSummary {
                    approval_id: summary.approval_id.clone().unwrap_or_default(),
                    tool_name: summary.tool_name.clone().unwrap_or_default(),
                    call_id: summary.call_id.clone().unwrap_or_default(),
                    summary: summary.message.clone(),
                    impact: summary.detail.clone(),
                    irreversible: summary.irreversible.unwrap_or(false),
                });
            }
            "run_guard_warning" => {
                guard_warnings.push(RunTraceGuardWarningSummary {
                    warning_kind: summary.warning_kind.clone().unwrap_or_default(),
                    title: summary.message.clone().unwrap_or_default(),
                    detail: summary.detail.clone(),
                    last_completed_step: summary.last_completed_step.clone(),
                });
            }
            "run_completed" => {
                lifecycle.completed = true;
                final_status = "completed".to_string();
            }
            "run_stopped" => {
                lifecycle.failed = true;
                lifecycle.stopped = true;
                stop_reason_kind = summary.error_kind.clone();
                final_status = "stopped".to_string();
            }
            "run_failed" => {
                lifecycle.failed = true;
                stop_reason_kind = summary.error_kind.clone();
                final_status = "failed".to_string();
            }
            "run_cancelled" => {
                lifecycle.cancelled = true;
                final_status = "cancelled".to_string();
            }
            _ => {}
        }
    }

    SessionRunTrace {
        session_id: session_id.to_string(),
        run_id: run_id.to_string(),
        final_status,
        event_count: summaries.len(),
        first_event_at: events.first().map(|event| event.created_at.clone()),
        last_event_at: events.last().map(|event| event.created_at.clone()),
        lifecycle,
        stop_reason_kind,
        tools,
        approvals,
        guard_warnings,
        parse_warnings,
        child_session_link: hidden_child_session_link(session_id),
        task_graph: project_task_graph_nodes(observed_task_identities.iter()),
        events: summaries,
    }
}

pub fn normalize_trace_for_fixture(trace: &SessionRunTrace) -> Value {
    let mut value = serde_json::to_value(trace).unwrap_or(Value::Null);
    normalize_trace_value(None, &mut value);
    value
}

fn compact_json(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Object(map) if map.is_empty() => None,
        Value::Array(items) if items.is_empty() => None,
        _ => serde_json::to_string(value)
            .ok()
            .map(|json| truncate_text(&json, 160)),
    }
}

fn summarize_task_identity_detail(
    task_identity: &crate::session_journal::SessionRunTaskIdentitySnapshot,
) -> String {
    let mut detail_parts = Vec::new();
    detail_parts.push(format!("task_id={}", task_identity.task_id));
    if let Some(parent_task_id) = task_identity.parent_task_id.as_deref() {
        if !parent_task_id.trim().is_empty() {
            detail_parts.push(format!("parent_task_id={}", parent_task_id.trim()));
        }
    }
    detail_parts.push(format!("root_task_id={}", task_identity.root_task_id));
    if let Some(task_path) = build_task_path(task_identity) {
        detail_parts.push(format!("task_path={}", task_path));
    }
    if !task_identity.backend_kind.trim().is_empty() {
        detail_parts.push(format!(
            "backend_kind={}",
            task_identity.backend_kind.trim()
        ));
    }
    detail_parts.join(", ")
}

fn extract_task_identity(
    event: &SessionRunEvent,
) -> Option<&crate::session_journal::SessionRunTaskIdentitySnapshot> {
    match event {
        SessionRunEvent::TaskContinued { task_identity, .. }
        | SessionRunEvent::TaskStateProjected { task_identity, .. } => Some(task_identity),
        SessionRunEvent::TaskDelegated { delegated_task, .. } => Some(delegated_task),
        SessionRunEvent::RunCompleted { turn_state, .. }
        | SessionRunEvent::RunFailed { turn_state, .. }
        | SessionRunEvent::RunStopped { turn_state, .. } => turn_state
            .as_ref()
            .and_then(|turn_state| turn_state.task_identity.as_ref()),
        SessionRunEvent::RunCancelled { .. } => None,
        _ => None,
    }
}

fn render_tool_output_preview(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        if let Some(summary) = parsed
            .get("summary")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            return Some(truncate_text(summary, 160));
        }
    }

    Some(truncate_text(trimmed, 160))
}

fn extract_child_session_id(input: &Value, output: Option<&str>) -> Option<String> {
    input
        .get("child_session_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            output.and_then(|raw| {
                serde_json::from_str::<Value>(raw).ok().and_then(|value| {
                    value
                        .get("child_session_id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
            })
        })
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let mut chars = trimmed.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn upsert_tool_summary(
    tools: &mut Vec<RunTraceToolSummary>,
    call_id: &str,
    tool_name: &str,
    status: &str,
    input_preview: Option<String>,
    output_preview: Option<String>,
    child_session_id: Option<String>,
    is_error: bool,
) {
    if let Some(existing) = tools
        .iter_mut()
        .find(|tool| tool.call_id == call_id && !call_id.is_empty())
    {
        if !tool_name.trim().is_empty() {
            existing.tool_name = tool_name.to_string();
        }
        existing.status = status.to_string();
        if input_preview.is_some() {
            existing.input_preview = input_preview;
        }
        if output_preview.is_some() {
            existing.output_preview = output_preview;
        }
        if child_session_id.is_some() {
            existing.child_session_id = child_session_id;
        }
        existing.is_error = is_error;
        return;
    }

    tools.push(RunTraceToolSummary {
        call_id: call_id.to_string(),
        tool_name: tool_name.to_string(),
        status: status.to_string(),
        input_preview,
        output_preview,
        child_session_id,
        is_error,
    });
}

fn hidden_child_session_link(session_id: &str) -> Option<SessionRunChildSessionLink> {
    let rest = session_id.strip_prefix("subagent--")?;
    let (parent_session_key, _) = rest.split_once("--")?;
    if parent_session_key.trim().is_empty() {
        return None;
    }
    Some(SessionRunChildSessionLink {
        parent_session_key: parent_session_key.to_string(),
    })
}

fn normalize_trace_value(field_name: Option<&str>, value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map.iter_mut() {
                if matches!(
                    key.as_str(),
                    "created_at" | "first_event_at" | "last_event_at"
                ) && !nested.is_null()
                {
                    *nested = Value::String("<timestamp>".to_string());
                } else {
                    normalize_trace_value(Some(key.as_str()), nested);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_trace_value(field_name, item);
            }
        }
        Value::String(text) => {
            *text = normalize_trace_string(field_name, text);
        }
        _ => {}
    }
}

fn normalize_trace_string(field_name: Option<&str>, value: &str) -> String {
    if matches!(
        field_name,
        Some("created_at" | "first_event_at" | "last_event_at")
    ) || looks_like_rfc3339_timestamp(value)
    {
        return "<timestamp>".to_string();
    }

    if let Some(normalized) = normalize_hidden_child_session_id(value) {
        return normalized;
    }

    if uuid::Uuid::parse_str(value).is_ok() {
        return "<uuid>".to_string();
    }

    value.to_string()
}

fn looks_like_rfc3339_timestamp(value: &str) -> bool {
    value.len() >= 20 && value.contains('T') && value.ends_with('Z')
}

fn normalize_hidden_child_session_id(value: &str) -> Option<String> {
    let rest = value.strip_prefix("subagent--")?;
    let (parent_session_key, tail) = rest.split_once("--")?;
    if parent_session_key.trim().is_empty() || tail.trim().is_empty() {
        return None;
    }
    if uuid::Uuid::parse_str(tail).is_ok() || tail.starts_with("child-") {
        Some(format!("subagent--{}--<uuid>", parent_session_key))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_session_run_trace, normalize_trace_for_fixture, summarize_stored_event,
        StoredSessionRunEvent,
    };
    use crate::agent::run_guard::RunStopReason;
    use crate::session_journal::SessionRunEvent;
    use serde::Deserialize;
    use serde_json::json;
    use std::path::PathBuf;

    #[derive(Debug, Deserialize)]
    struct TraceFixtureCase {
        session_id: String,
        run_id: String,
        events: Vec<TraceFixtureEvent>,
        expected: serde_json::Value,
    }

    #[derive(Debug, Deserialize)]
    struct TraceFixtureEvent {
        event_type: String,
        created_at: String,
        payload: serde_json::Value,
    }

    fn stored_event(
        session_id: &str,
        run_id: &str,
        event_type: &str,
        created_at: &str,
        event: SessionRunEvent,
    ) -> StoredSessionRunEvent {
        StoredSessionRunEvent {
            session_id: session_id.to_string(),
            run_id: run_id.to_string(),
            event_type: event_type.to_string(),
            payload_json: serde_json::to_string(&event).expect("serialize session run event"),
            created_at: created_at.to_string(),
        }
    }

    fn fixture_path(file_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("run_traces")
            .join(file_name)
    }

    #[test]
    fn summarize_stored_event_degrades_gracefully_for_malformed_payload() {
        let summary = summarize_stored_event(&StoredSessionRunEvent {
            session_id: "session-1".to_string(),
            run_id: "run-1".to_string(),
            event_type: "tool_started".to_string(),
            payload_json: "{\"type\":\"tool_started\",\"run_id\":".to_string(),
            created_at: "2026-03-27T00:00:00Z".to_string(),
        });

        assert_eq!(summary.session_id, "session-1");
        assert_eq!(summary.run_id, "run-1");
        assert_eq!(summary.event_type, "tool_started");
        assert!(summary.parse_warning.is_some());
        assert!(summary.message.is_some());
    }

    #[test]
    fn build_session_run_trace_summarizes_successful_tool_run() {
        let trace = build_session_run_trace(
            "session-success",
            "run-success",
            &[
                stored_event(
                    "session-success",
                    "run-success",
                    "run_started",
                    "2026-03-27T01:00:00Z",
                    SessionRunEvent::RunStarted {
                        run_id: "run-success".to_string(),
                        user_message_id: "user-1".to_string(),
                    },
                ),
                stored_event(
                    "session-success",
                    "run-success",
                    "tool_started",
                    "2026-03-27T01:00:01Z",
                    SessionRunEvent::ToolStarted {
                        run_id: "run-success".to_string(),
                        tool_name: "read_file".to_string(),
                        call_id: "call-1".to_string(),
                        input: json!({ "path": "README.md" }),
                    },
                ),
                stored_event(
                    "session-success",
                    "run-success",
                    "tool_completed",
                    "2026-03-27T01:00:02Z",
                    SessionRunEvent::ToolCompleted {
                        run_id: "run-success".to_string(),
                        tool_name: "read_file".to_string(),
                        call_id: "call-1".to_string(),
                        input: json!({ "path": "README.md" }),
                        output: "README loaded".to_string(),
                        is_error: false,
                    },
                ),
                stored_event(
                    "session-success",
                    "run-success",
                    "run_completed",
                    "2026-03-27T01:00:03Z",
                    SessionRunEvent::RunCompleted {
                        run_id: "run-success".to_string(),
                        turn_state: None,
                    },
                ),
            ],
        );

        assert_eq!(trace.session_id, "session-success");
        assert_eq!(trace.run_id, "run-success");
        assert_eq!(trace.final_status, "completed");
        assert_eq!(trace.event_count, 4);
        assert_eq!(
            trace.first_event_at.as_deref(),
            Some("2026-03-27T01:00:00Z")
        );
        assert_eq!(trace.last_event_at.as_deref(), Some("2026-03-27T01:00:03Z"));
        assert!(trace.lifecycle.started);
        assert!(trace.lifecycle.completed);
        assert_eq!(trace.tools.len(), 1);
        assert_eq!(trace.tools[0].tool_name, "read_file");
        assert_eq!(trace.tools[0].status, "completed");
    }

    #[test]
    fn build_session_run_trace_captures_guard_warning_and_stop_reason() {
        let trace = build_session_run_trace(
            "session-loop",
            "run-loop",
            &[
                stored_event(
                    "session-loop",
                    "run-loop",
                    "run_started",
                    "2026-03-27T02:00:00Z",
                    SessionRunEvent::RunStarted {
                        run_id: "run-loop".to_string(),
                        user_message_id: "user-1".to_string(),
                    },
                ),
                stored_event(
                    "session-loop",
                    "run-loop",
                    "run_guard_warning",
                    "2026-03-27T02:00:01Z",
                    SessionRunEvent::RunGuardWarning {
                        run_id: "run-loop".to_string(),
                        warning_kind: "loop_detected".to_string(),
                        title: "任务可能即将卡住".to_string(),
                        message: "系统检测到连续重复步骤。".to_string(),
                        detail: Some("browser_snapshot 连续 5 次返回相同结果。".to_string()),
                        last_completed_step: Some("已填写封面标题".to_string()),
                    },
                ),
                stored_event(
                    "session-loop",
                    "run-loop",
                    "run_stopped",
                    "2026-03-27T02:00:02Z",
                    SessionRunEvent::RunStopped {
                        run_id: "run-loop".to_string(),
                        stop_reason: RunStopReason::loop_detected(
                            "browser_snapshot 连续 6 次返回相同结果。",
                        )
                        .with_last_completed_step("已填写封面标题"),
                        turn_state: None,
                    },
                ),
            ],
        );

        assert_eq!(trace.final_status, "stopped");
        assert!(trace.lifecycle.failed);
        assert!(trace.lifecycle.stopped);
        assert_eq!(trace.guard_warnings.len(), 1);
        assert_eq!(trace.guard_warnings[0].warning_kind, "loop_detected");
        assert_eq!(
            trace.guard_warnings[0].last_completed_step.as_deref(),
            Some("已填写封面标题")
        );
        assert_eq!(trace.stop_reason_kind.as_deref(), Some("loop_detected"));
    }

    #[test]
    fn build_session_run_trace_records_approval_and_cancellation() {
        let trace = build_session_run_trace(
            "session-approval",
            "run-approval",
            &[
                stored_event(
                    "session-approval",
                    "run-approval",
                    "run_started",
                    "2026-03-27T03:00:00Z",
                    SessionRunEvent::RunStarted {
                        run_id: "run-approval".to_string(),
                        user_message_id: "user-1".to_string(),
                    },
                ),
                stored_event(
                    "session-approval",
                    "run-approval",
                    "approval_requested",
                    "2026-03-27T03:00:01Z",
                    SessionRunEvent::ApprovalRequested {
                        run_id: "run-approval".to_string(),
                        approval_id: "approval-1".to_string(),
                        tool_name: "shell_command".to_string(),
                        call_id: "call-1".to_string(),
                        input: json!({ "command": "git status" }),
                        summary: "需要执行 shell_command".to_string(),
                        impact: Some("可能读取当前仓库状态".to_string()),
                        irreversible: false,
                    },
                ),
                stored_event(
                    "session-approval",
                    "run-approval",
                    "run_cancelled",
                    "2026-03-27T03:00:02Z",
                    SessionRunEvent::RunCancelled {
                        run_id: "run-approval".to_string(),
                        reason: Some("user cancelled".to_string()),
                    },
                ),
            ],
        );

        assert_eq!(trace.final_status, "cancelled");
        assert!(trace.lifecycle.waiting_approval);
        assert!(trace.lifecycle.cancelled);
        assert_eq!(trace.approvals.len(), 1);
        assert_eq!(trace.approvals[0].approval_id, "approval-1");
        assert_eq!(trace.approvals[0].tool_name, "shell_command");
    }

    #[test]
    fn build_session_run_trace_marks_hidden_child_session_linkage() {
        let trace = build_session_run_trace(
            "subagent--parent_session--child-1",
            "run-child",
            &[
                stored_event(
                    "subagent--parent_session--child-1",
                    "run-child",
                    "run_started",
                    "2026-03-27T04:00:00Z",
                    SessionRunEvent::RunStarted {
                        run_id: "run-child".to_string(),
                        user_message_id: "user-1".to_string(),
                    },
                ),
                stored_event(
                    "subagent--parent_session--child-1",
                    "run-child",
                    "run_completed",
                    "2026-03-27T04:00:01Z",
                    SessionRunEvent::RunCompleted {
                        run_id: "run-child".to_string(),
                        turn_state: None,
                    },
                ),
            ],
        );

        let child_link = trace
            .child_session_link
            .expect("hidden child session linkage");
        assert_eq!(child_link.parent_session_key, "parent_session");
        assert_eq!(trace.final_status, "completed");
    }

    #[test]
    fn summarize_stored_event_includes_task_lineage_details() {
        let summary = summarize_stored_event(&stored_event(
            "session-1",
            "run-1",
            "task_state_projected",
            "2026-04-09T00:00:00Z",
            SessionRunEvent::TaskStateProjected {
                run_id: "run-1".to_string(),
                task_identity: crate::session_journal::SessionRunTaskIdentitySnapshot {
                    task_id: "task-child".to_string(),
                    parent_task_id: Some("task-parent".to_string()),
                    root_task_id: "task-root".to_string(),
                    task_kind: "sub_agent_task".to_string(),
                    surface_kind: "hidden_child_surface".to_string(),
                    backend_kind: "hidden_child_backend".to_string(),
                },
            },
        ));

        assert_eq!(
            summary.message.as_deref(),
            Some("task=sub_agent_task surface=hidden_child_surface")
        );
        assert!(summary
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("backend_kind=hidden_child_backend")));
        assert!(summary
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("parent_task_id=task-parent")));
        assert!(summary.detail.as_deref().is_some_and(
            |detail| detail.contains("task_path=task-root -> task-parent -> task-child")
        ));
    }

    #[test]
    fn task_delegated_events_project_delegated_tasks_into_task_graph() {
        let trace = build_session_run_trace(
            "session-1",
            "run-1",
            &[stored_event(
                "session-1",
                "run-1",
                "task_delegated",
                "2026-04-09T00:00:00Z",
                SessionRunEvent::TaskDelegated {
                    run_id: "run-1".to_string(),
                    from_task_id: "task-parent".to_string(),
                    from_task_kind: "primary_user_task".to_string(),
                    from_surface_kind: "local_chat_surface".to_string(),
                    delegated_task: crate::session_journal::SessionRunTaskIdentitySnapshot {
                        task_id: "task-child".to_string(),
                        parent_task_id: Some("task-parent".to_string()),
                        root_task_id: "task-root".to_string(),
                        task_kind: "sub_agent_task".to_string(),
                        surface_kind: "hidden_child_surface".to_string(),
                        backend_kind: "hidden_child_backend".to_string(),
                    },
                },
            )],
        );

        assert!(trace
            .task_graph
            .iter()
            .any(|node| node.task_id == "task-child"
                && node.parent_task_id.as_deref() == Some("task-parent")
                && node.backend_kind == "hidden_child_backend"));
    }

    #[test]
    fn trace_fixture_cases_match_expected_output() {
        for file_name in [
            "success.json",
            "loop_intercepted.json",
            "admission_conflict.json",
            "approval_resume.json",
            "child_session_success.json",
            "child_session_failure.json",
        ] {
            let raw = std::fs::read_to_string(fixture_path(file_name)).expect("read trace fixture");
            let fixture: TraceFixtureCase =
                serde_json::from_str(&raw).expect("parse trace fixture json");
            let events: Vec<StoredSessionRunEvent> = fixture
                .events
                .into_iter()
                .map(|event| StoredSessionRunEvent {
                    session_id: fixture.session_id.clone(),
                    run_id: fixture.run_id.clone(),
                    event_type: event.event_type,
                    payload_json: serde_json::to_string(&event.payload)
                        .expect("serialize fixture payload"),
                    created_at: event.created_at,
                })
                .collect();

            let actual = normalize_trace_for_fixture(&build_session_run_trace(
                &fixture.session_id,
                &fixture.run_id,
                &events,
            ));

            assert_eq!(
                actual, fixture.expected,
                "fixture {file_name} did not match"
            );
        }
    }
}
