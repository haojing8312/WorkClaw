use crate::agent::run_guard::RunStopReason;
use crate::agent::runtime::effective_tool_set::EffectiveToolDecisionRecord;
use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
use crate::agent::runtime::session_runs::{
    append_session_run_event_with_pool, attach_assistant_message_to_run_with_pool,
};
use crate::agent::runtime::skill_routing::observability::{
    route_fallback_reason_key, ImplicitRouteObservation,
};
use crate::agent::runtime::tool_dispatch::INTERNAL_SKILL_DISPATCH_INPUT_KEY;
use crate::commands::im_host::{
    maybe_dispatch_registered_im_session_reply_with_pool,
    maybe_emit_registered_host_lifecycle_phase_for_session_with_pool,
    maybe_stop_registered_host_processing_for_session_with_pool,
};
use crate::session_journal::{SessionJournalStore, SessionRunEvent, SessionRunTurnStateSnapshot};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) async fn insert_session_message_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    role: &str,
    content: &str,
    content_json: Option<&str>,
) -> Result<String, String> {
    let msg_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, content_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&msg_id)
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(content_json)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(msg_id)
}

pub(crate) async fn record_route_attempt_log_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    capability: &str,
    api_format: &str,
    model_name: &str,
    attempt_index: usize,
    retry_index: usize,
    error_kind: &str,
    success: bool,
    error_message: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO route_attempt_logs (id, session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, success, error_message, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(capability)
    .bind(api_format)
    .bind(model_name)
    .bind(attempt_index as i64)
    .bind(retry_index as i64)
    .bind(error_kind)
    .bind(success)
    .bind(error_message)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await;
}

pub(crate) async fn append_run_started_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    user_message_id: &str,
) -> Result<(), String> {
    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunStarted {
            run_id: run_id.to_string(),
            user_message_id: user_message_id.to_string(),
        },
    )
    .await
}

pub(crate) async fn append_skill_route_recorded_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    observation: &ImplicitRouteObservation,
    tool_plan_summary: Option<EffectiveToolDecisionRecord>,
) -> Result<(), String> {
    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::SkillRouteRecorded {
            run_id: run_id.to_string(),
            route_latency_ms: observation.route_latency_ms,
            candidate_count: observation.candidate_count,
            selected_runner: observation.selected_runner.clone(),
            selected_skill: observation.selected_skill.clone(),
            fallback_reason: observation
                .fallback_reason
                .map(|reason| route_fallback_reason_key(reason).to_string()),
            tool_recommendation_summary: observation.tool_recommendation_summary.clone(),
            tool_recommendation_aligned: observation.tool_recommendation_aligned,
            tool_plan_summary,
        },
    )
    .await
}

pub(crate) async fn append_run_failed_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    error_kind: &str,
    error_message: &str,
    turn_state: Option<&TurnStateSnapshot>,
) {
    let _ = maybe_emit_registered_host_lifecycle_phase_for_session_with_pool(
        pool,
        session_id,
        Some(run_id),
        crate::commands::openclaw_plugins::im_host_contract::ImReplyLifecyclePhase::Failed,
        None,
    )
    .await;
    let _ = maybe_stop_registered_host_processing_for_session_with_pool(
        pool,
        session_id,
        Some(run_id),
        Some("failed"),
        None,
    )
    .await;
    let _ = append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunFailed {
            run_id: run_id.to_string(),
            error_kind: error_kind.to_string(),
            error_message: error_message.to_string(),
            turn_state: turn_state.map(SessionRunTurnStateSnapshot::from),
        },
    )
    .await;
}

#[allow(dead_code)]
pub(crate) async fn append_run_guard_warning_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    warning_kind: &str,
    title: &str,
    message: &str,
    detail: Option<&str>,
    last_completed_step: Option<&str>,
) -> Result<(), String> {
    let _ = maybe_stop_registered_host_processing_for_session_with_pool(
        pool,
        session_id,
        Some(run_id),
        Some("run_guard_warning"),
        None,
    )
    .await;
    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunGuardWarning {
            run_id: run_id.to_string(),
            warning_kind: warning_kind.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            detail: detail.map(str::to_string),
            last_completed_step: last_completed_step.map(str::to_string),
        },
    )
    .await
}

pub(crate) async fn append_run_stopped_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    stop_reason: &RunStopReason,
    turn_state: Option<&TurnStateSnapshot>,
) -> Result<(), String> {
    let _ = maybe_emit_registered_host_lifecycle_phase_for_session_with_pool(
        pool,
        session_id,
        Some(run_id),
        crate::commands::openclaw_plugins::im_host_contract::ImReplyLifecyclePhase::Stopped,
        None,
    )
    .await;
    let _ = maybe_stop_registered_host_processing_for_session_with_pool(
        pool,
        session_id,
        Some(run_id),
        Some(stop_reason.kind.as_key()),
        None,
    )
    .await;
    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunStopped {
            run_id: run_id.to_string(),
            stop_reason: stop_reason.clone(),
            turn_state: turn_state.map(SessionRunTurnStateSnapshot::from),
        },
    )
    .await
}

pub(crate) async fn append_partial_assistant_chunk_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    chunk: &str,
) {
    let _ = append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::AssistantChunkAppended {
            run_id: run_id.to_string(),
            chunk: chunk.to_string(),
        },
    )
    .await;
}

fn attach_reasoning_to_content(
    content: &str,
    final_text: &str,
    has_tool_calls: bool,
    reasoning_text: &str,
    reasoning_duration_ms: Option<u64>,
) -> String {
    if reasoning_text.trim().is_empty() || has_tool_calls {
        return content.to_string();
    }

    let base = if has_tool_calls {
        serde_json::from_str::<Value>(content).unwrap_or_else(|_| {
            json!({
                "text": final_text,
                "items": [],
            })
        })
    } else {
        json!({
            "text": final_text,
        })
    };

    let mut obj = base.as_object().cloned().unwrap_or_default();
    obj.insert(
        "reasoning".to_string(),
        json!({
            "status": "completed",
            "duration_ms": reasoning_duration_ms,
            "content": reasoning_text,
        }),
    );
    serde_json::to_string(&Value::Object(obj)).unwrap_or_else(|_| content.to_string())
}

async fn load_structured_tool_calls_for_run_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    run_id: &str,
) -> Result<Vec<Value>, String> {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT payload_json
         FROM session_run_events
         WHERE session_id = ? AND run_id = ? AND event_type IN ('tool_started', 'tool_completed')
         ORDER BY created_at ASC, id ASC",
    )
    .bind(session_id)
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取运行工具事件失败: {e}"))?;

    let mut tool_calls: Vec<Value> = Vec::new();
    for (payload_json,) in rows {
        let Ok(event) = serde_json::from_str::<SessionRunEvent>(&payload_json) else {
            continue;
        };
        match event {
            SessionRunEvent::ToolStarted {
                call_id,
                tool_name,
                input,
                ..
            } => {
                if is_internal_skill_dispatch_tool_input(&input) {
                    continue;
                }
                if let Some(existing) = tool_calls
                    .iter_mut()
                    .find(|entry| entry["toolCall"]["id"].as_str() == Some(call_id.as_str()))
                {
                    existing["toolCall"]["name"] = Value::String(tool_name);
                    existing["toolCall"]["input"] = input;
                    existing["toolCall"]["status"] = Value::String("running".to_string());
                } else {
                    tool_calls.push(json!({
                        "type": "tool_call",
                        "toolCall": {
                            "id": call_id,
                            "name": tool_name,
                            "input": input,
                            "status": "running"
                        }
                    }));
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
                if is_internal_skill_dispatch_tool_input(&input) {
                    continue;
                }
                let status = if is_error { "error" } else { "completed" };
                if let Some(existing) = tool_calls
                    .iter_mut()
                    .find(|entry| entry["toolCall"]["id"].as_str() == Some(call_id.as_str()))
                {
                    existing["toolCall"]["name"] = Value::String(tool_name);
                    existing["toolCall"]["input"] = input;
                    existing["toolCall"]["output"] = Value::String(output);
                    existing["toolCall"]["status"] = Value::String(status.to_string());
                } else {
                    tool_calls.push(json!({
                        "type": "tool_call",
                        "toolCall": {
                            "id": call_id,
                            "name": tool_name,
                            "input": input,
                            "output": output,
                            "status": status
                        }
                    }));
                }
            }
            _ => {}
        }
    }

    Ok(tool_calls)
}

fn is_internal_skill_dispatch_tool_input(input: &Value) -> bool {
    input
        .get(INTERNAL_SKILL_DISPATCH_INPUT_KEY)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) async fn persist_partial_assistant_message_for_run_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    run_id: &str,
    partial_text: &str,
) -> Result<Option<String>, String> {
    let trimmed_text = partial_text.trim();
    let tool_calls = load_structured_tool_calls_for_run_with_pool(pool, session_id, run_id).await?;
    if trimmed_text.is_empty() && tool_calls.is_empty() {
        return Ok(None);
    }

    let mut items = Vec::new();
    if !trimmed_text.is_empty() {
        items.push(json!({
            "type": "text",
            "content": trimmed_text,
        }));
    }
    items.extend(tool_calls);

    let content = serde_json::to_string(&json!({
        "text": trimmed_text,
        "items": items,
    }))
    .map_err(|e| format!("序列化部分助手消息失败: {e}"))?;

    let msg_id =
        insert_session_message_with_pool(pool, session_id, "assistant", &content, None).await?;
    attach_assistant_message_to_run_with_pool(pool, run_id, &msg_id).await?;
    Ok(Some(msg_id))
}

pub(crate) async fn finalize_run_success_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    final_text: &str,
    has_tool_calls: bool,
    content: &str,
    reasoning_text: &str,
    reasoning_duration_ms: Option<u64>,
    turn_state: Option<&TurnStateSnapshot>,
) -> Result<(), String> {
    if !final_text.is_empty() {
        append_session_run_event_with_pool(
            pool,
            journal,
            session_id,
            SessionRunEvent::AssistantChunkAppended {
                run_id: run_id.to_string(),
                chunk: final_text.to_string(),
            },
        )
        .await?;
    }

    if !final_text.is_empty() || has_tool_calls {
        let persisted_content = attach_reasoning_to_content(
            content,
            final_text,
            has_tool_calls,
            reasoning_text,
            reasoning_duration_ms,
        );
        let msg_id = insert_session_message_with_pool(
            pool,
            session_id,
            "assistant",
            &persisted_content,
            None,
        )
        .await?;
        attach_assistant_message_to_run_with_pool(pool, run_id, &msg_id).await?;
    }

    if !final_text.trim().is_empty() {
        if let Err(error) =
            maybe_dispatch_registered_im_session_reply_with_pool(pool, session_id, final_text).await
        {
            eprintln!("failed to dispatch IM final reply for session {session_id}: {error}");
        }
    }

    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunCompleted {
            run_id: run_id.to_string(),
            turn_state: turn_state.map(SessionRunTurnStateSnapshot::from),
        },
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod run_guard_persistence_tests {
    use super::{
        append_run_guard_warning_with_pool, append_run_started_with_pool,
        append_run_stopped_with_pool, finalize_run_success_with_pool,
        persist_partial_assistant_message_for_run_with_pool,
    };
    use crate::agent::run_guard::RunStopReason;
    use crate::commands::feishu_gateway::{
        clear_feishu_runtime_state_for_outbound, remember_feishu_runtime_state_for_outbound,
        set_feishu_official_runtime_outbound_send_hook_for_tests,
    };
    use crate::commands::openclaw_plugins::{
        OpenClawPluginFeishuOutboundDeliveryResult, OpenClawPluginFeishuRuntimeState,
    };
    use crate::session_journal::SessionJournalStore;
    use serde_json::{json, Value};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    async fn setup_run_event_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE session_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_message_id TEXT NOT NULL DEFAULT '',
                assistant_message_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'queued',
                buffered_text TEXT NOT NULL DEFAULT '',
                error_kind TEXT NOT NULL DEFAULT '',
                error_message TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_runs table");

        sqlx::query(
            "CREATE TABLE session_run_events (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_run_events table");

        sqlx::query(
            "CREATE TABLE im_thread_sessions (
                thread_id TEXT NOT NULL,
                employee_id TEXT NOT NULL DEFAULT '',
                session_id TEXT NOT NULL,
                route_session_key TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_thread_sessions table");

        sqlx::query(
            "CREATE TABLE im_inbox_events (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL DEFAULT '',
                thread_id TEXT NOT NULL,
                message_id TEXT NOT NULL DEFAULT '',
                text_preview TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_inbox_events table");

        sqlx::query(
            "CREATE TABLE app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create app_settings table");

        sqlx::query(
            "CREATE TABLE agent_employees (
                employee_id TEXT NOT NULL DEFAULT '',
                role_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL DEFAULT '',
                feishu_app_id TEXT NOT NULL DEFAULT '',
                feishu_app_secret TEXT NOT NULL DEFAULT '',
                enabled INTEGER NOT NULL DEFAULT 1,
                is_default INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        pool
    }

    async fn setup_partial_message_pool() -> sqlx::SqlitePool {
        let pool = setup_run_event_pool().await;
        sqlx::query(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                content_json TEXT,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create messages table");
        pool
    }

    #[tokio::test]
    async fn append_run_stopped_event_persists_loop_detected_reason() {
        let pool = setup_run_event_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let stop_reason =
            RunStopReason::loop_detected("工具 browser_snapshot 已连续 6 次返回相同结果。")
                .with_last_completed_step("已填写封面标题");

        append_run_started_with_pool(&pool, &journal, "session-1", "run-1", "user-1")
            .await
            .expect("append run started");
        append_run_stopped_with_pool(&pool, &journal, "session-1", "run-1", &stop_reason, None)
            .await
            .expect("append run stopped");

        let (event_type, payload_json): (String, String) = sqlx::query_as(
            "SELECT event_type, payload_json
             FROM session_run_events
             WHERE run_id = 'run-1' AND event_type = 'run_stopped'
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("query run_stopped event");
        assert_eq!(event_type, "run_stopped");
        assert!(payload_json.contains("\"kind\":\"loop_detected\""));
        assert!(payload_json.contains("\"last_completed_step\":\"已填写封面标题\""));

        let (status, error_kind, error_message): (String, String, String) = sqlx::query_as(
            "SELECT status, error_kind, error_message
             FROM session_runs
             WHERE id = 'run-1'",
        )
        .fetch_one(&pool)
        .await
        .expect("query session run projection");
        assert_eq!(status, "failed");
        assert_eq!(error_kind, "loop_detected");
        assert!(error_message.contains("最后完成步骤：已填写封面标题"));
    }

    #[tokio::test]
    async fn append_run_guard_warning_event_persists_warning_payload() {
        let pool = setup_run_event_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        append_run_started_with_pool(&pool, &journal, "session-2", "run-2", "user-2")
            .await
            .expect("append run started");
        append_run_guard_warning_with_pool(
            &pool,
            &journal,
            "session-2",
            "run-2",
            "loop_detected",
            "任务可能即将卡住",
            "系统检测到连续重复步骤，若继续无变化将自动停止。",
            Some("工具 browser_snapshot 已连续 5 次使用相同输入执行。"),
            Some("已填写封面标题"),
        )
        .await
        .expect("append run guard warning");

        let (event_type, payload_json): (String, String) = sqlx::query_as(
            "SELECT event_type, payload_json
             FROM session_run_events
             WHERE run_id = 'run-2' AND event_type = 'run_guard_warning'
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("query run_guard_warning event");

        assert_eq!(event_type, "run_guard_warning");
        assert!(payload_json.contains("\"warning_kind\":\"loop_detected\""));
        assert!(payload_json.contains("\"last_completed_step\":\"已填写封面标题\""));
    }

    #[tokio::test]
    async fn persist_partial_assistant_message_for_run_keeps_tool_calls() {
        let pool = setup_partial_message_pool().await;

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES ('run-1', 'session-1', 'user-1', '', 'thinking', 'partial text', '', '', '2026-03-29T00:00:00Z', '2026-03-29T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed session run");

        sqlx::query(
            "INSERT INTO session_run_events (id, run_id, session_id, event_type, payload_json, created_at)
             VALUES
             ('evt-1', 'run-1', 'session-1', 'tool_started', ?, '2026-03-29T00:00:02Z'),
             ('evt-2', 'run-1', 'session-1', 'tool_completed', ?, '2026-03-29T00:00:03Z')",
        )
        .bind(
            serde_json::to_string(&crate::session_journal::SessionRunEvent::ToolStarted {
                run_id: "run-1".to_string(),
                tool_name: "bash".to_string(),
                call_id: "call-1".to_string(),
                task_identity: None,
                task_continuation: None,
                input: json!({"command": "echo hi"}),
            })
            .expect("serialize tool started"),
        )
        .bind(
            serde_json::to_string(&crate::session_journal::SessionRunEvent::ToolCompleted {
                run_id: "run-1".to_string(),
                tool_name: "bash".to_string(),
                call_id: "call-1".to_string(),
                task_identity: None,
                task_continuation: None,
                input: json!({"command": "echo hi"}),
                output: "hi".to_string(),
                is_error: false,
            })
            .expect("serialize tool completed"),
        )
        .execute(&pool)
        .await
        .expect("seed session run events");

        let msg_id = persist_partial_assistant_message_for_run_with_pool(
            &pool,
            "session-1",
            "run-1",
            "我先检查一下环境。",
        )
        .await
        .expect("persist partial assistant")
        .expect("assistant message id");

        let (assistant_message_id,): (String,) =
            sqlx::query_as("SELECT assistant_message_id FROM session_runs WHERE id = 'run-1'")
                .fetch_one(&pool)
                .await
                .expect("query assistant message id");
        assert_eq!(assistant_message_id, msg_id);

        let (content,): (String,) = sqlx::query_as("SELECT content FROM messages WHERE id = ?")
            .bind(&msg_id)
            .fetch_one(&pool)
            .await
            .expect("query partial assistant content");
        let parsed: Value = serde_json::from_str(&content).expect("structured assistant content");
        assert_eq!(parsed["text"].as_str(), Some("我先检查一下环境。"));
        assert_eq!(parsed["items"].as_array().map(|items| items.len()), Some(2));
        assert_eq!(
            parsed["items"][1]["toolCall"]["name"].as_str(),
            Some("bash")
        );
        assert_eq!(
            parsed["items"][1]["toolCall"]["output"].as_str(),
            Some("hi")
        );
    }

    #[tokio::test]
    async fn finalize_run_success_dispatches_feishu_reply_from_backend() {
        let pool = setup_partial_message_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let runtime_state = OpenClawPluginFeishuRuntimeState::default();
        let sent_texts = Arc::new(Mutex::new(Vec::<String>::new()));
        let sent_texts_for_hook = sent_texts.clone();

        remember_feishu_runtime_state_for_outbound(&runtime_state);

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES ('run-feishu-1', 'session-feishu-1', 'user-1', '', 'thinking', '', '', '', '2026-03-29T00:00:00Z', '2026-03-29T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed feishu session run");

        sqlx::query(
            "INSERT INTO im_thread_sessions (thread_id, employee_id, session_id, route_session_key, created_at, updated_at)
             VALUES ('oc_chat_backend_final', '', 'session-feishu-1', '', '2026-03-29T00:00:00Z', '2026-03-29T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed im thread session");

        sqlx::query(
            "INSERT INTO im_inbox_events (id, event_id, thread_id, message_id, text_preview, source, created_at)
             VALUES ('evt-feishu-1', 'evt-feishu-1', 'oc_chat_backend_final', 'om_parent_1', '你好', 'feishu', '2026-03-29T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("seed feishu inbox event");

        set_feishu_official_runtime_outbound_send_hook_for_tests(Some(Arc::new(move |request| {
            sent_texts_for_hook
                .lock()
                .expect("lock sent texts")
                .push(request.text.clone());
            Ok(OpenClawPluginFeishuOutboundDeliveryResult {
                delivered: true,
                channel: "feishu".to_string(),
                account_id: request.account_id.clone(),
                target: request.target.clone(),
                thread_id: request.thread_id.clone(),
                text: request.text.clone(),
                mode: request.mode.clone(),
                message_id: format!("om_{}", request.request_id),
                chat_id: "oc_chat_backend_final".to_string(),
                sequence: 1,
            })
        })));

        finalize_run_success_with_pool(
            &pool,
            &journal,
            "session-feishu-1",
            "run-feishu-1",
            &"A".repeat(4000),
            false,
            &"A".repeat(4000),
            "",
            None,
            None,
        )
        .await
        .expect("finalize run success");

        set_feishu_official_runtime_outbound_send_hook_for_tests(None);
        clear_feishu_runtime_state_for_outbound();

        let rebuilt = sent_texts
            .lock()
            .expect("lock sent texts")
            .iter()
            .map(String::as_str)
            .collect::<String>();
        assert_eq!(rebuilt, "A".repeat(4000));
    }

    #[tokio::test]
    async fn persist_partial_assistant_message_for_run_filters_internal_skill_dispatch_exec_calls()
    {
        let pool = setup_partial_message_pool().await;

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES ('run-2', 'session-2', 'user-2', '', 'thinking', 'partial text', '', '', '2026-03-29T00:00:00Z', '2026-03-29T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed session run");

        sqlx::query(
            "INSERT INTO session_run_events (id, run_id, session_id, event_type, payload_json, created_at)
             VALUES
             ('evt-10', 'run-2', 'session-2', 'tool_started', ?, '2026-03-29T00:00:02Z'),
             ('evt-11', 'run-2', 'session-2', 'tool_completed', ?, '2026-03-29T00:00:03Z'),
             ('evt-12', 'run-2', 'session-2', 'tool_started', ?, '2026-03-29T00:00:04Z'),
             ('evt-13', 'run-2', 'session-2', 'tool_completed', ?, '2026-03-29T00:00:05Z')",
        )
        .bind(
            serde_json::to_string(&crate::session_journal::SessionRunEvent::ToolStarted {
                run_id: "run-2".to_string(),
                tool_name: "skill".to_string(),
                call_id: "call-skill-1".to_string(),
                task_identity: None,
                task_continuation: None,
                input: json!({"skill_name": "dispatch-skill", "arguments": ["--employee", "xt"]}),
            })
            .expect("serialize skill tool started"),
        )
        .bind(
            serde_json::to_string(&crate::session_journal::SessionRunEvent::ToolCompleted {
                run_id: "run-2".to_string(),
                tool_name: "skill".to_string(),
                call_id: "call-skill-1".to_string(),
                task_identity: None,
                task_continuation: None,
                input: json!({"skill_name": "dispatch-skill", "arguments": ["--employee", "xt"]}),
                output: "{\"ok\":true}".to_string(),
                is_error: false,
            })
            .expect("serialize skill tool completed"),
        )
        .bind(
            serde_json::to_string(&crate::session_journal::SessionRunEvent::ToolStarted {
                run_id: "run-2".to_string(),
                tool_name: "exec".to_string(),
                call_id: "skill-command-bridge-1".to_string(),
                task_identity: None,
                task_continuation: None,
                input: json!({
                    "command": "--employee xt",
                    "commandName": "dispatch-skill",
                    "skillName": "dispatch-skill",
                    "__workclaw_internal_skill_dispatch": true
                }),
            })
            .expect("serialize exec tool started"),
        )
        .bind(
            serde_json::to_string(&crate::session_journal::SessionRunEvent::ToolCompleted {
                run_id: "run-2".to_string(),
                tool_name: "exec".to_string(),
                call_id: "skill-command-bridge-1".to_string(),
                task_identity: None,
                task_continuation: None,
                input: json!({
                    "command": "--employee xt",
                    "commandName": "dispatch-skill",
                    "skillName": "dispatch-skill",
                    "__workclaw_internal_skill_dispatch": true
                }),
                output: "{\"ok\":true}".to_string(),
                is_error: false,
            })
            .expect("serialize exec tool completed"),
        )
        .execute(&pool)
        .await
        .expect("seed session run events");

        let msg_id = persist_partial_assistant_message_for_run_with_pool(
            &pool,
            "session-2",
            "run-2",
            "我先调用飞书技能。",
        )
        .await
        .expect("persist partial assistant")
        .expect("assistant message id");

        let (content,): (String,) = sqlx::query_as("SELECT content FROM messages WHERE id = ?")
            .bind(&msg_id)
            .fetch_one(&pool)
            .await
            .expect("query partial assistant content");
        let parsed: Value = serde_json::from_str(&content).expect("structured assistant content");

        assert_eq!(parsed["items"].as_array().map(|items| items.len()), Some(2));
        assert_eq!(
            parsed["items"][1]["toolCall"]["name"].as_str(),
            Some("skill")
        );
    }

    #[test]
    fn attach_reasoning_to_content_skips_tool_call_transcripts() {
        let content = r#"{"text":"先检查目录","items":[{"type":"text","content":"先检查目录"},{"type":"tool_call","toolCall":{"id":"call-1","name":"list_dir","input":{"path":"."},"status":"completed"}}]}"#;

        let persisted = super::attach_reasoning_to_content(
            content,
            "先检查目录",
            true,
            "先思考再调用工具",
            Some(760_000),
        );

        let parsed: Value = serde_json::from_str(&persisted).expect("structured content");
        assert!(parsed.get("reasoning").is_none());
        assert_eq!(parsed["items"].as_array().map(|items| items.len()), Some(2));
    }
}
