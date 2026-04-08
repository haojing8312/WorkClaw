use super::runtime_io::{
    append_partial_assistant_chunk_with_pool, append_run_failed_with_pool,
    append_run_started_with_pool, append_run_stopped_with_pool, finalize_run_success_with_pool,
    insert_session_message_with_pool,
};
use super::RuntimeTranscript;
use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::parse_run_stop_reason;
use crate::agent::types::StreamDelta;
use crate::agent::{AgentExecutor, ToolRegistry};
use crate::session_journal::SessionJournalStore;
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

pub(crate) struct ChildSessionRunRequest<'a> {
    pub parent_session_id: &'a str,
    pub prompt: &'a str,
    pub agent_type: &'a str,
    pub delegate_display_name: &'a str,
    pub registry: Arc<ToolRegistry>,
    pub db: &'a sqlx::SqlitePool,
    pub journal: &'a SessionJournalStore,
    pub api_format: &'a str,
    pub base_url: &'a str,
    pub api_key: &'a str,
    pub model: &'a str,
    pub allowed_tools: Option<Vec<String>>,
    pub max_iterations: usize,
    pub app_handle: Option<&'a AppHandle>,
    pub parent_stream_session_id: Option<&'a str>,
    pub delegate_role_id: Option<&'a str>,
    pub delegate_role_name: Option<&'a str>,
    pub work_dir: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChildSessionRunOutcome {
    pub final_text: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedChildSessionRun {
    pub child_session_id: String,
    pub run_id: String,
}

pub(crate) async fn prepare_hidden_child_session_run(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    parent_session_id: &str,
    prompt: &str,
) -> Result<PreparedChildSessionRun> {
    let child_session_id = build_hidden_child_session_id(parent_session_id);
    let user_message_id =
        insert_session_message_with_pool(db, &child_session_id, "user", prompt, None)
            .await
            .map_err(anyhow::Error::msg)?;
    let run_id = Uuid::new_v4().to_string();
    append_run_started_with_pool(db, journal, &child_session_id, &run_id, &user_message_id)
        .await
        .map_err(anyhow::Error::msg)?;
    journal.observability().record_child_session_link();

    Ok(PreparedChildSessionRun {
        child_session_id,
        run_id,
    })
}

pub(crate) async fn finalize_hidden_child_session_success_with_messages(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    prepared: &PreparedChildSessionRun,
    final_messages: &[Value],
) -> Result<String> {
    let (final_text, has_tool_calls, content) =
        RuntimeTranscript::build_assistant_content_from_final_messages(final_messages, 0);
    finalize_run_success_with_pool(
        db,
        journal,
        &prepared.child_session_id,
        &prepared.run_id,
        &final_text,
        has_tool_calls,
        &content,
        "",
        None,
        None,
    )
    .await
    .map_err(anyhow::Error::msg)?;

    Ok(final_text)
}

async fn finalize_hidden_child_session_failure(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    prepared: &PreparedChildSessionRun,
    partial_text: &str,
    error: &anyhow::Error,
) -> Result<()> {
    let error_text = error.to_string();
    if !partial_text.is_empty() {
        append_partial_assistant_chunk_with_pool(
            db,
            journal,
            &prepared.child_session_id,
            &prepared.run_id,
            partial_text,
        )
        .await;
    }

    if let Some(stop_reason) = parse_run_stop_reason(&error_text) {
        append_run_stopped_with_pool(
            db,
            journal,
            &prepared.child_session_id,
            &prepared.run_id,
            &stop_reason,
            None,
        )
        .await
        .map_err(anyhow::Error::msg)?;
    } else {
        append_run_failed_with_pool(
            db,
            journal,
            &prepared.child_session_id,
            &prepared.run_id,
            "child_session",
            &error_text,
            None,
        )
        .await;
    }

    Ok(())
}

pub(crate) async fn run_hidden_child_session(
    params: ChildSessionRunRequest<'_>,
) -> Result<ChildSessionRunOutcome> {
    let prepared = prepare_hidden_child_session_run(
        params.db,
        params.journal,
        params.parent_session_id,
        params.prompt,
    )
    .await?;

    let executor =
        AgentExecutor::with_max_iterations(Arc::clone(&params.registry), params.max_iterations);
    let partial_text = Arc::new(Mutex::new(String::new()));
    let partial_text_for_stream = Arc::clone(&partial_text);
    let stream_app = params.app_handle.cloned();
    let parent_stream_session_id = params.parent_stream_session_id.map(str::to_string);
    let child_session_for_stream = prepared.child_session_id.clone();
    let child_run_for_stream = prepared.run_id.clone();
    let role_id = params.delegate_role_id.unwrap_or_default().to_string();
    let role_name = params.delegate_role_name.unwrap_or_default().to_string();
    let system_prompt = format!(
        "你是一个专注的子 Agent (类型: {})，当前承接角色: {}。完成以下任务后返回结果。简洁地报告你的发现。",
        params.agent_type, params.delegate_display_name
    );
    let messages = vec![json!({
        "role": "user",
        "content": params.prompt,
    })];

    let attempt = executor
        .execute_turn(
            params.api_format,
            params.base_url,
            params.api_key,
            params.model,
            &system_prompt,
            messages,
            move |delta: StreamDelta| {
                if let StreamDelta::Text(token) = delta {
                    if let Ok(mut buffer) = partial_text_for_stream.lock() {
                        buffer.push_str(&token);
                    }
                    if let (Some(app), Some(parent_session_id)) =
                        (&stream_app, &parent_stream_session_id)
                    {
                        let _ = app.emit(
                            "stream-token",
                            json!({
                                "session_id": parent_session_id,
                                "token": token,
                                "done": false,
                                "sub_agent": true,
                                "child_session_id": child_session_for_stream,
                                "child_run_id": child_run_for_stream,
                                "role_id": role_id,
                                "role_name": role_name,
                            }),
                        );
                    }
                }
            },
            params.app_handle,
            Some(&prepared.child_session_id),
            params.allowed_tools.as_deref(),
            PermissionMode::Unrestricted,
            None,
            params.work_dir.clone(),
            Some(params.max_iterations),
            None,
            None,
            None,
        )
        .await;

    match attempt {
        Ok(final_messages) => {
            let final_text = finalize_hidden_child_session_success_with_messages(
                params.db,
                params.journal,
                &prepared,
                &final_messages,
            )
            .await?;
            if let (Some(app), Some(parent_session_id)) =
                (params.app_handle, params.parent_stream_session_id)
            {
                let _ = app.emit(
                    "stream-token",
                    json!({
                        "session_id": parent_session_id,
                        "token": String::new(),
                        "done": true,
                        "sub_agent": true,
                        "child_session_id": &prepared.child_session_id,
                        "child_run_id": &prepared.run_id,
                        "role_id": params.delegate_role_id.unwrap_or_default(),
                        "role_name": params.delegate_role_name.unwrap_or_default(),
                    }),
                );
            }
            Ok(ChildSessionRunOutcome { final_text })
        }
        Err(error) => {
            let partial_text = partial_text
                .lock()
                .map(|buffer| buffer.clone())
                .unwrap_or_default();
            finalize_hidden_child_session_failure(
                params.db,
                params.journal,
                &prepared,
                &partial_text,
                &error,
            )
            .await?;

            Err(error)
        }
    }
}

fn build_hidden_child_session_id(parent_session_id: &str) -> String {
    let parent_component = sanitize_session_component(parent_session_id);
    format!("subagent--{}--{}", parent_component, Uuid::new_v4())
}

fn sanitize_session_component(raw: &str) -> String {
    let sanitized: String = raw
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "session".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::{
        finalize_hidden_child_session_success_with_messages, prepare_hidden_child_session_run,
    };
    use crate::agent::runtime::{RunRegistry, RuntimeObservability};
    use crate::session_journal::SessionJournalStore;
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn setup_hidden_child_session_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                model_id TEXT NOT NULL,
                permission_mode TEXT NOT NULL DEFAULT 'accept_edits',
                work_dir TEXT NOT NULL DEFAULT '',
                employee_id TEXT NOT NULL DEFAULT '',
                session_mode TEXT NOT NULL DEFAULT 'general',
                team_id TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create sessions table");

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

        pool
    }

    #[tokio::test]
    async fn prepare_hidden_child_session_run_persists_user_message_without_creating_visible_session(
    ) {
        let pool = setup_hidden_child_session_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let prepared = prepare_hidden_child_session_run(
            &pool,
            &journal,
            "parent-session",
            "请总结当前目录情况",
        )
        .await
        .expect("prepare hidden child session");

        assert!(prepared
            .child_session_id
            .starts_with("subagent--parent-session--"));

        let (session_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE id = ?")
            .bind(&prepared.child_session_id)
            .fetch_one(&pool)
            .await
            .expect("count sessions");
        assert_eq!(session_rows, 0);

        let (message_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM messages WHERE session_id = ?")
                .bind(&prepared.child_session_id)
                .fetch_one(&pool)
                .await
                .expect("count messages");
        assert_eq!(message_count, 1);

        let (status,): (String,) = sqlx::query_as("SELECT status FROM session_runs WHERE id = ?")
            .bind(&prepared.run_id)
            .fetch_one(&pool)
            .await
            .expect("load session run");
        assert_eq!(status, "thinking");

        let journal_dir = journal_root.path().join(&prepared.child_session_id);
        assert!(journal_dir.join("events.jsonl").exists());
        assert!(journal_dir.join("state.json").exists());
    }


    #[tokio::test]
    async fn prepare_hidden_child_session_run_updates_observability_child_session_total() {
        let pool = setup_hidden_child_session_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let observability = std::sync::Arc::new(RuntimeObservability::new(8));
        let journal = SessionJournalStore::with_registry_and_observability(
            journal_root.path().to_path_buf(),
            std::sync::Arc::new(RunRegistry::default()),
            observability.clone(),
        );

        let _prepared = prepare_hidden_child_session_run(
            &pool,
            &journal,
            "parent-session",
            "summarize workspace",
        )
        .await
        .expect("prepare hidden child session");

        assert_eq!(observability.snapshot().child_sessions.linked_total, 1);
    }

    #[tokio::test]
    async fn finalize_hidden_child_session_success_persists_assistant_message_and_completes_run() {
        let pool = setup_hidden_child_session_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let prepared = prepare_hidden_child_session_run(
            &pool,
            &journal,
            "parent-session",
            "请总结当前目录情况",
        )
        .await
        .expect("prepare hidden child session");

        let final_text = finalize_hidden_child_session_success_with_messages(
            &pool,
            &journal,
            &prepared,
            &[json!({
                "role": "assistant",
                "content": "子会话完成总结",
            })],
        )
        .await
        .expect("finalize hidden child session success");

        assert_eq!(final_text, "子会话完成总结");

        let (message_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM messages WHERE session_id = ?")
                .bind(&prepared.child_session_id)
                .fetch_one(&pool)
                .await
                .expect("count messages");
        assert_eq!(message_count, 2);

        let (status, assistant_message_id): (String, String) =
            sqlx::query_as("SELECT status, assistant_message_id FROM session_runs WHERE id = ?")
                .bind(&prepared.run_id)
                .fetch_one(&pool)
                .await
                .expect("load session run");
        assert_eq!(status, "completed");
        assert!(!assistant_message_id.is_empty());
    }
}
