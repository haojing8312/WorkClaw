use super::runtime_io::insert_session_message_with_pool;
use crate::agent::runtime::task_backend::{
    HiddenChildTaskBackendPreparationRequest, TaskBackendPreparationRequest,
    TaskBackendTokenCallback,
};
use crate::agent::runtime::task_entry;
use crate::agent::runtime::task_entry::{
    DelegatedTaskBackendRunAndFinalizeRequest, DelegatedTaskEntryOutcome,
};
use crate::agent::runtime::task_lifecycle;
use crate::agent::runtime::task_lifecycle::TaskBeginParentContext;
use crate::agent::runtime::task_record::TaskRecord;
use crate::agent::runtime::task_state::TaskState;
use crate::agent::types::StreamDelta;
use crate::agent::{AgentExecutor, ToolRegistry};
use crate::session_journal::SessionJournalStore;
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
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
    pub task_state: TaskState,
    pub parent_task_record: Option<TaskRecord>,
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
    let parent_task_record =
        task_lifecycle::resolve_latest_task_record_for_session(journal, parent_session_id).await;
    let task_state = TaskState::new_sub_agent(
        &child_session_id,
        &user_message_id,
        &run_id,
        parent_task_record
            .as_ref()
            .map(|record| &record.task_identity),
    );
    journal.observability().record_child_session_link();

    Ok(PreparedChildSessionRun {
        child_session_id,
        run_id,
        task_state,
        parent_task_record,
    })
}

async fn project_hidden_child_task_return_to_parent(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    parent_session_id: &str,
    prepared: &PreparedChildSessionRun,
) {
    let Some(parent_task_record) = prepared.parent_task_record.as_ref() else {
        return;
    };
    let Some(returned_task_record) = task_lifecycle::resolve_task_record_for_session_task_id(
        journal,
        &prepared.child_session_id,
        &prepared.task_state.task_identity.task_id,
    )
    .await
    else {
        return;
    };

    let _ = task_lifecycle::project_delegated_task_return(
        db,
        journal,
        parent_session_id,
        parent_task_record,
        &returned_task_record,
    )
    .await;
}

#[cfg(test)]
pub(crate) async fn finalize_hidden_child_session_success_with_messages(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    prepared: &PreparedChildSessionRun,
    final_messages: &[Value],
) -> Result<String> {
    let active_task_record =
        task_lifecycle::begin_task_run(db, journal, &prepared.task_state, None)
            .await
            .map_err(anyhow::Error::msg)?;
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
    task_lifecycle::finalize_after_terminal(
        db,
        journal,
        &prepared.child_session_id,
        &active_task_record,
        true,
        None,
    )
    .await;

    Ok(final_text)
}

#[cfg(test)]
async fn finalize_hidden_child_session_execution_outcome(
    db: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    _prepared: &PreparedChildSessionRun,
    outcome: TaskExecutionOutcome,
) -> Result<ChildSessionRunOutcome> {
    match task_entry::finalize_delegated_task_execution_outcome_entry(
        DelegatedTaskTerminalFinalizeEntryRequest {
            db,
            journal,
            task_execution_outcome: outcome,
        },
    )
    .await
    .map_err(anyhow::Error::msg)?
    {
        DelegatedTaskEntryOutcome::Completed { output } => {
            Ok(ChildSessionRunOutcome { final_text: output })
        }
        DelegatedTaskEntryOutcome::Stopped { error, .. }
        | DelegatedTaskEntryOutcome::Failed { error } => Err(anyhow::Error::msg(error)),
    }
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

    let executor = Arc::new(AgentExecutor::with_max_iterations(
        Arc::clone(&params.registry),
        params.max_iterations,
    ));
    let stream_app = params.app_handle.cloned();
    let parent_stream_session_id = params.parent_stream_session_id.map(str::to_string);
    let child_session_for_stream = prepared.child_session_id.clone();
    let child_run_for_stream = prepared.run_id.clone();
    let role_id = params.delegate_role_id.unwrap_or_default().to_string();
    let role_name = params.delegate_role_name.unwrap_or_default().to_string();
    let token_callback: TaskBackendTokenCallback = Arc::new(move |delta: StreamDelta| {
        if let StreamDelta::Text(token) = delta {
            if let (Some(app), Some(parent_session_id)) = (&stream_app, &parent_stream_session_id) {
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
    });
    let finalized = task_entry::run_and_finalize_delegated_task_backend(
        DelegatedTaskBackendRunAndFinalizeRequest {
            db: params.db,
            journal: params.journal,
            task_state: prepared.task_state.clone(),
            parent_context: prepared.parent_task_record.as_ref().map(|record| {
                TaskBeginParentContext {
                    session_id: params.parent_session_id,
                    active_task_record: record,
                }
            }),
            preparation_request: TaskBackendPreparationRequest::HiddenChild(
                HiddenChildTaskBackendPreparationRequest {
                    agent_executor: &executor,
                    prompt: params.prompt,
                    agent_type: params.agent_type,
                    delegate_display_name: params.delegate_display_name,
                    api_format: params.api_format,
                    base_url: params.base_url,
                    api_key: params.api_key,
                    model: params.model,
                    allowed_tools: params.allowed_tools.clone(),
                    max_iterations: params.max_iterations,
                    work_dir: params.work_dir.clone(),
                },
            ),
            app_handle: params.app_handle.cloned(),
            agent_executor: Arc::clone(&executor),
            on_token: token_callback,
            prepare_surface: |_| {},
        },
    )
    .await;

    project_hidden_child_task_return_to_parent(
        params.db,
        params.journal,
        params.parent_session_id,
        &prepared,
    )
    .await;

    let finalized = match finalized.map_err(anyhow::Error::msg)? {
        DelegatedTaskEntryOutcome::Completed { output } => {
            Ok(ChildSessionRunOutcome { final_text: output })
        }
        DelegatedTaskEntryOutcome::Stopped { error, .. }
        | DelegatedTaskEntryOutcome::Failed { error } => Err(anyhow::Error::msg(error)),
    };

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

    finalized
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
        finalize_hidden_child_session_execution_outcome,
        finalize_hidden_child_session_success_with_messages, prepare_hidden_child_session_run,
    };
    use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
    use crate::agent::runtime::kernel::execution_plan::{ExecutionLane, ExecutionOutcome};
    use crate::agent::runtime::kernel::session_profile::SessionSurfaceKind;
    use crate::agent::runtime::kernel::turn_state::TurnStateSnapshot;
    use crate::agent::runtime::task_execution::TaskExecutionOutcome;
    use crate::agent::runtime::task_lifecycle;
    use crate::agent::runtime::task_lifecycle::TaskBeginParentContext;
    use crate::agent::runtime::{RunRegistry, RuntimeObservability};
    use crate::session_journal::{
        SessionJournalStore, SessionRunEvent, SessionRunTaskIdentitySnapshot,
    };
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
    async fn prepare_hidden_child_session_run_projects_subagent_task_identity_from_parent_session()
    {
        let pool = setup_hidden_child_session_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        journal
            .append_event(
                "parent-session",
                SessionRunEvent::TaskStateProjected {
                    run_id: "run-parent".to_string(),
                    task_identity: SessionRunTaskIdentitySnapshot {
                        task_id: "task-parent".to_string(),
                        parent_task_id: None,
                        root_task_id: "task-root".to_string(),
                        task_kind: "primary_user_task".to_string(),
                        surface_kind: "local_chat_surface".to_string(),
                        backend_kind: "interactive_chat_backend".to_string(),
                    },
                },
            )
            .await
            .expect("append parent task identity");

        let prepared = prepare_hidden_child_session_run(
            &pool,
            &journal,
            "parent-session",
            "summarize workspace",
        )
        .await
        .expect("prepare hidden child session");

        let state = journal
            .read_state(&prepared.child_session_id)
            .await
            .expect("read child state");
        let run = state.runs.first().expect("run snapshot");

        assert_eq!(
            run.task_identity
                .as_ref()
                .and_then(|identity| identity.parent_task_id.as_deref()),
            Some("task-parent")
        );
        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.root_task_id.as_str()),
            Some("task-root")
        );
        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.task_kind.as_str()),
            Some("sub_agent_task")
        );
        assert_eq!(
            run.task_identity
                .as_ref()
                .map(|identity| identity.surface_kind.as_str()),
            Some("hidden_child_surface")
        );
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

    #[tokio::test]
    async fn finalize_hidden_child_session_execution_outcome_persists_hidden_surface_turn_state() {
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

        let outcome = ExecutionOutcome::RouteExecution {
            route_execution: RouteExecutionOutcome {
                final_messages: Some(vec![
                    json!({
                        "role": "user",
                        "content": "请总结当前目录情况",
                    }),
                    json!({
                        "role": "assistant",
                        "content": "子会话完成总结",
                    }),
                ]),
                last_error: None,
                last_error_kind: None,
                last_stop_reason: None,
                partial_text: String::new(),
                reasoning_text: String::new(),
                reasoning_duration_ms: None,
                tool_exposure_expanded: false,
                tool_exposure_expansion_reason: None,
                compaction_boundary: None,
            },
            reconstructed_history_len: 1,
            turn_state: TurnStateSnapshot::default()
                .with_session_surface(SessionSurfaceKind::HiddenChildSession)
                .with_execution_lane(ExecutionLane::OpenTask),
        };

        let active_task_record = task_lifecycle::begin_task_run(
            &pool,
            &journal,
            &prepared.task_state,
            None::<TaskBeginParentContext<'_>>,
        )
        .await
        .expect("begin hidden child task run");
        let wrapped =
            TaskExecutionOutcome::new(prepared.task_state.clone(), active_task_record, outcome);

        let final_text =
            finalize_hidden_child_session_execution_outcome(&pool, &journal, &prepared, wrapped)
                .await
                .expect("finalize hidden child execution outcome")
                .final_text;

        assert_eq!(final_text, "子会话完成总结");

        let state = journal
            .read_state(&prepared.child_session_id)
            .await
            .expect("read journal state");
        let run = state.runs.first().expect("run snapshot");
        assert_eq!(
            run.turn_state
                .as_ref()
                .and_then(|turn_state| turn_state.session_surface.as_deref()),
            Some("hidden_child_session")
        );
        assert_eq!(
            run.turn_state
                .as_ref()
                .and_then(|turn_state| turn_state.task_identity.as_ref())
                .map(|identity| identity.task_kind.as_str()),
            Some("sub_agent_task")
        );
    }
}
