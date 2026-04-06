use crate::agent::runtime::attempt_runner::RouteExecutionOutcome;
use crate::agent::runtime::runtime_io as chat_io;
use crate::agent::run_guard::RunStopReason;
use crate::session_journal::SessionJournalStore;
use tauri::{AppHandle, Emitter, Runtime};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OutcomeCommitter;

#[derive(Debug, Clone)]
pub(crate) enum TerminalOutcome {
    DirectDispatch(String),
    RouteExecution {
        route_execution: RouteExecutionOutcome,
        reconstructed_history_len: usize,
    },
    SkillCommandFailed(String),
    SkillCommandStopped {
        stop_reason: RunStopReason,
        error: String,
    },
}

impl OutcomeCommitter {
    pub(crate) async fn commit_terminal_outcome<R: Runtime>(
        app: &AppHandle<R>,
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        run_id: &str,
        outcome: TerminalOutcome,
    ) -> Result<(), String> {
        Self::commit_terminal_outcome_with_emitter(
            db,
            journal,
            session_id,
            run_id,
            outcome,
            |session_id, token, done, sub_agent| {
                let _ = app.emit(
                    "stream-token",
                    crate::agent::runtime::events::StreamToken {
                        session_id: session_id.to_string(),
                        token,
                        done,
                        sub_agent,
                    },
                );
            },
        )
        .await
    }

    async fn commit_terminal_outcome_with_emitter<F>(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        run_id: &str,
        outcome: TerminalOutcome,
        mut emit_stream_token: F,
    ) -> Result<(), String>
    where
        F: FnMut(&str, String, bool, bool),
    {
        match outcome {
            TerminalOutcome::DirectDispatch(output) => {
                chat_io::finalize_run_success_with_pool(
                    db, journal, session_id, run_id, &output, false, &output, "", None,
                )
                .await?;
                emit_stream_token(session_id, output, false, false);
                emit_stream_token(session_id, String::new(), true, false);
                Ok(())
            }
            TerminalOutcome::RouteExecution {
                route_execution,
                reconstructed_history_len,
            } => Self::commit_route_execution(
                db,
                journal,
                session_id,
                run_id,
                route_execution,
                reconstructed_history_len,
                &mut emit_stream_token,
            )
            .await,
            TerminalOutcome::SkillCommandFailed(error) => {
                chat_io::append_run_failed_with_pool(
                    db,
                    journal,
                    session_id,
                    run_id,
                    "skill_command_dispatch",
                    &error,
                )
                .await;
                emit_stream_token(session_id, String::new(), true, false);
                Err(error)
            }
            TerminalOutcome::SkillCommandStopped {
                stop_reason,
                error,
            } => {
                let _ = chat_io::append_run_stopped_with_pool(
                    db, journal, session_id, run_id, &stop_reason,
                )
                .await;
                emit_stream_token(session_id, String::new(), true, false);
                Err(error)
            }
        }
    }

    async fn commit_route_execution<F>(
        db: &sqlx::SqlitePool,
        journal: &SessionJournalStore,
        session_id: &str,
        run_id: &str,
        route_execution: RouteExecutionOutcome,
        reconstructed_history_len: usize,
        emit_stream_token: &mut F,
    ) -> Result<(), String>
    where
        F: FnMut(&str, String, bool, bool),
    {
        let final_messages = match route_execution.final_messages {
            Some(messages) => messages,
            None => {
                let partial_text = route_execution.partial_text;
                if !partial_text.is_empty() {
                    chat_io::append_partial_assistant_chunk_with_pool(
                        db,
                        journal,
                        session_id,
                        run_id,
                        &partial_text,
                    )
                    .await;
                }
                let _ = chat_io::persist_partial_assistant_message_for_run_with_pool(
                    db,
                    session_id,
                    run_id,
                    &partial_text,
                )
                .await;

                let error_message = route_execution
                    .last_error
                    .unwrap_or_else(|| "所有候选模型执行失败".to_string());
                let error_kind = route_execution
                    .last_error_kind
                    .unwrap_or_else(|| "unknown".to_string());
                if let Some(stop_reason) = route_execution.last_stop_reason.as_ref() {
                    let _ = chat_io::append_run_stopped_with_pool(
                        db, journal, session_id, run_id, stop_reason,
                    )
                    .await;
                } else {
                    chat_io::append_run_failed_with_pool(
                        db,
                        journal,
                        session_id,
                        run_id,
                        &error_kind,
                        &error_message,
                    )
                    .await;
                }
                emit_stream_token(session_id, String::new(), true, false);
                return Err(error_message);
            }
        };

        let (final_text, has_tool_calls, content) =
            crate::agent::runtime::RuntimeTranscript::build_assistant_content_from_final_messages(
                &final_messages,
                reconstructed_history_len,
            );

        let finalize_result = chat_io::finalize_run_success_with_pool(
            db,
            journal,
            session_id,
            run_id,
            &final_text,
            has_tool_calls,
            &content,
            &route_execution.reasoning_text,
            route_execution.reasoning_duration_ms,
        )
        .await;

        if let Err(err) = finalize_result {
            chat_io::append_run_failed_with_pool(
                db,
                journal,
                session_id,
                run_id,
                "persistence",
                &err,
            )
            .await;
            emit_stream_token(session_id, String::new(), true, false);
            return Err(err);
        }

        emit_stream_token(session_id, String::new(), true, false);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{OutcomeCommitter, TerminalOutcome};
    use crate::agent::run_guard::RunStopReason;
    use crate::session_journal::SessionJournalStore;
    use serde_json::Value;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn setup_commit_pool() -> sqlx::SqlitePool {
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

    fn setup_session_journal() -> SessionJournalStore {
        SessionJournalStore::new(tempdir().expect("journal tempdir").path().to_path_buf())
    }

    fn build_route_execution_with_partial_text(
        partial_text: &str,
        stop_reason: Option<RunStopReason>,
    ) -> crate::agent::runtime::attempt_runner::RouteExecutionOutcome {
        crate::agent::runtime::attempt_runner::RouteExecutionOutcome {
            final_messages: None,
            last_error: Some("all candidates failed".to_string()),
            last_error_kind: Some("network".to_string()),
            last_stop_reason: stop_reason,
            partial_text: partial_text.to_string(),
            reasoning_text: "reasoning".to_string(),
            reasoning_duration_ms: Some(17),
        }
    }

    #[tokio::test]
    async fn commit_terminal_outcome_handles_direct_dispatch_success() {
        let pool = setup_commit_pool().await;
        let journal = setup_session_journal();
        let emitted = std::sync::Mutex::new(Vec::<(String, String, bool, bool)>::new());

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES ('run-1', 'session-1', 'user-1', '', 'thinking', '', '', '', '2026-03-29T00:00:00Z', '2026-03-29T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed session run");

        OutcomeCommitter::commit_terminal_outcome_with_emitter(
            &pool,
            &journal,
            "session-1",
            "run-1",
            TerminalOutcome::DirectDispatch("ok".to_string()),
            |session_id, token, done, sub_agent| {
                emitted.lock().expect("lock emitted tokens").push((
                    session_id.to_string(),
                    token,
                    done,
                    sub_agent,
                ));
            },
        )
        .await
        .expect("commit direct dispatch");

        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM session_runs WHERE id = 'run-1'")
                .fetch_one(&pool)
                .await
                .expect("query session status");
        assert_eq!(status, "completed");

        let (content,): (String,) =
            sqlx::query_as("SELECT content FROM messages WHERE session_id = 'session-1'")
                .fetch_one(&pool)
                .await
                .expect("query assistant message");
        assert_eq!(content, "ok");

        let emitted = emitted.lock().expect("read emitted tokens");
        assert_eq!(
            emitted.as_slice(),
            &[
                ("session-1".to_string(), "ok".to_string(), false, false),
                ("session-1".to_string(), String::new(), true, false),
            ]
        );
    }

    #[tokio::test]
    async fn commit_terminal_outcome_handles_route_execution_failure_with_partial_output() {
        let pool = setup_commit_pool().await;
        let journal = setup_session_journal();
        let emitted = std::sync::Mutex::new(Vec::<(String, String, bool, bool)>::new());

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES ('run-2', 'session-1', 'user-1', '', 'thinking', '', '', '', '2026-03-29T00:00:00Z', '2026-03-29T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed session run");

        let route_execution = build_route_execution_with_partial_text("partial output", None);
        let result = OutcomeCommitter::commit_terminal_outcome_with_emitter(
            &pool,
            &journal,
            "session-1",
            "run-2",
            TerminalOutcome::RouteExecution {
                route_execution,
                reconstructed_history_len: 1,
            },
            |session_id, token, done, sub_agent| {
                emitted.lock().expect("lock emitted tokens").push((
                    session_id.to_string(),
                    token,
                    done,
                    sub_agent,
                ));
            },
        )
        .await;

        assert_eq!(result.unwrap_err(), "all candidates failed");

        let (status, error_kind, error_message): (String, String, String) = sqlx::query_as(
            "SELECT status, error_kind, error_message FROM session_runs WHERE id = 'run-2'",
        )
        .fetch_one(&pool)
        .await
        .expect("query failed run state");
        assert_eq!(status, "failed");
        assert_eq!(error_kind, "network");
        assert_eq!(error_message, "all candidates failed");

        let (content,): (String,) =
            sqlx::query_as("SELECT content FROM messages WHERE session_id = 'session-1'")
                .fetch_one(&pool)
                .await
                .expect("query partial assistant message");
        let parsed: Value = serde_json::from_str(&content).expect("assistant content");
        assert_eq!(parsed["text"].as_str(), Some("partial output"));

        let emitted = emitted.lock().expect("read emitted tokens");
        assert_eq!(
            emitted.as_slice(),
            &[("session-1".to_string(), String::new(), true, false)]
        );
    }

    #[tokio::test]
    async fn commit_terminal_outcome_handles_skill_command_failure() {
        let pool = setup_commit_pool().await;
        let journal = setup_session_journal();
        let emitted = std::sync::Mutex::new(Vec::<(String, String, bool, bool)>::new());

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES ('run-3', 'session-1', 'user-1', '', 'thinking', '', '', '', '2026-03-29T00:00:00Z', '2026-03-29T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed session run");

        let result = OutcomeCommitter::commit_terminal_outcome_with_emitter(
            &pool,
            &journal,
            "session-1",
            "run-3",
            TerminalOutcome::SkillCommandFailed("dispatch failed".to_string()),
            |session_id, token, done, sub_agent| {
                emitted.lock().expect("lock emitted tokens").push((
                    session_id.to_string(),
                    token,
                    done,
                    sub_agent,
                ));
            },
        )
        .await;

        assert_eq!(result.unwrap_err(), "dispatch failed");

        let (status, error_kind, error_message): (String, String, String) = sqlx::query_as(
            "SELECT status, error_kind, error_message FROM session_runs WHERE id = 'run-3'",
        )
        .fetch_one(&pool)
        .await
        .expect("query failed run state");
        assert_eq!(status, "failed");
        assert_eq!(error_kind, "skill_command_dispatch");
        assert_eq!(error_message, "dispatch failed");

        let emitted = emitted.lock().expect("read emitted tokens");
        assert_eq!(
            emitted.as_slice(),
            &[("session-1".to_string(), String::new(), true, false)]
        );
    }

    #[tokio::test]
    async fn commit_terminal_outcome_handles_skill_command_stop() {
        let pool = setup_commit_pool().await;
        let journal = setup_session_journal();
        let emitted = std::sync::Mutex::new(Vec::<(String, String, bool, bool)>::new());

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES ('run-4', 'session-1', 'user-1', '', 'thinking', '', '', '', '2026-03-29T00:00:00Z', '2026-03-29T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed session run");

        let result = OutcomeCommitter::commit_terminal_outcome_with_emitter(
            &pool,
            &journal,
            "session-1",
            "run-4",
            TerminalOutcome::SkillCommandStopped {
                stop_reason: RunStopReason::loop_detected("exec repeated with the same input"),
                error: "execution stopped".to_string(),
            },
            |session_id, token, done, sub_agent| {
                emitted.lock().expect("lock emitted tokens").push((
                    session_id.to_string(),
                    token,
                    done,
                    sub_agent,
                ));
            },
        )
        .await;

        assert_eq!(result.unwrap_err(), "execution stopped");

        let (status, error_kind, error_message): (String, String, String) = sqlx::query_as(
            "SELECT status, error_kind, error_message FROM session_runs WHERE id = 'run-4'",
        )
        .fetch_one(&pool)
        .await
        .expect("query stopped run state");
        assert_eq!(status, "failed");
        assert_eq!(error_kind, "loop_detected");
        assert!(
            error_message.contains("连续重复步骤"),
            "unexpected stop message: {error_message}"
        );

        let emitted = emitted.lock().expect("read emitted tokens");
        assert_eq!(
            emitted.as_slice(),
            &[("session-1".to_string(), String::new(), true, false)]
        );
    }
}
