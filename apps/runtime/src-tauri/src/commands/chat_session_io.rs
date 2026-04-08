mod session_compaction;
mod session_export;
mod session_store;
mod session_view;

pub(crate) use session_compaction::{
    load_compaction_inputs_with_pool, replace_messages_with_compacted_with_pool,
};
pub(crate) use session_export::{export_session_markdown_with_pool, write_export_file_to_path};
pub(crate) use session_store::{
    create_session_with_pool, delete_session_with_pool, get_messages_with_pool,
    list_sessions_with_pool, search_sessions_global_with_pool, update_session_workspace_with_pool,
};



#[cfg(test)]
mod tests {
    use super::{
        export_session_markdown_with_pool, list_sessions_with_pool,
    };
    use crate::agent::run_guard::RunStopReason;
    use crate::commands::chat_policy::permission_mode_label_for_display;
    use crate::commands::session_runs::append_session_run_event_with_pool;
    use crate::session_journal::{SessionJournalStore, SessionRunEvent};
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    #[tokio::test]
    async fn list_sessions_with_pool_tolerates_null_titles() {
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
                permission_mode TEXT NOT NULL DEFAULT 'standard',
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
            "CREATE TABLE im_thread_sessions (
                thread_id TEXT NOT NULL,
                employee_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                route_session_key TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                channel TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (thread_id, employee_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_thread_sessions table");

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
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                role_id TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        sqlx::query(
            "CREATE TABLE employee_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create employee_groups table");

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES
             ('session-null-title', 'skill-1', NULL, '2026-03-13T00:00:00Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-normal', 'skill-1', 'Visible Session', '2026-03-13T00:01:00Z', 'model-1', 'full_access', '', '', 'general', '')",
        )
        .execute(&pool)
        .await
        .expect("seed sessions");

        let sessions = list_sessions_with_pool(&pool, permission_mode_label_for_display)
            .await
            .expect("list sessions should succeed");

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0]["id"], "session-normal");
        assert_eq!(sessions[0]["title"], "Visible Session");
        assert_eq!(sessions[1]["id"], "session-null-title");
        assert_eq!(sessions[1]["title"], "New Chat");
    }

    #[tokio::test]
    async fn list_sessions_with_pool_tolerates_legacy_im_thread_sessions_without_channel() {
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
                permission_mode TEXT NOT NULL DEFAULT 'standard',
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
            "CREATE TABLE im_thread_sessions (
                thread_id TEXT NOT NULL,
                employee_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                route_session_key TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (thread_id, employee_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create legacy im_thread_sessions table");

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
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                role_id TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        sqlx::query(
            "CREATE TABLE employee_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create employee_groups table");

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES ('legacy-session', 'skill-1', 'Legacy Session', '2026-03-13T00:00:00Z', 'model-1', 'standard', '', '', 'general', '')",
        )
        .execute(&pool)
        .await
        .expect("seed session");

        let sessions = list_sessions_with_pool(&pool, permission_mode_label_for_display)
            .await
            .expect("list sessions should succeed for legacy schema");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["id"], "legacy-session");
        assert_eq!(sessions[0]["source_channel"], "local");
        assert_eq!(sessions[0]["source_label"], "");
    }

    #[tokio::test]
    async fn list_sessions_with_pool_derives_display_title_for_general_sessions() {
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
                permission_mode TEXT NOT NULL DEFAULT 'standard',
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
            "CREATE TABLE im_thread_sessions (
                thread_id TEXT NOT NULL,
                employee_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                route_session_key TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                channel TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (thread_id, employee_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_thread_sessions table");

        sqlx::query(
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                role_id TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        sqlx::query(
            "CREATE TABLE employee_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create employee_groups table");

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES
             ('session-general', 'skill-1', 'New Chat', '2026-03-14T00:00:00Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-general-generic-first', 'skill-1', 'New Chat', '2026-03-14T00:01:00Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-team', 'skill-1', 'New Chat', '2026-03-14T00:02:00Z', 'model-1', 'standard', '', '', 'team_entry', 'team-a'),
             ('session-employee', 'skill-1', 'New Chat', '2026-03-14T00:03:00Z', 'model-1', 'standard', '', 'emp-1', 'employee_direct', '')",
        )
        .execute(&pool)
        .await
        .expect("seed sessions");

        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at)
             VALUES
             ('msg-1', 'session-general', 'user', '帮我整理本周销售周报', '2026-03-14T00:00:01Z'),
             ('msg-2', 'session-general-generic-first', 'user', '你好', '2026-03-14T00:01:01Z'),
             ('msg-3', 'session-general-generic-first', 'user', '修复登录接口超时问题', '2026-03-14T00:01:02Z')",
        )
        .execute(&pool)
        .await
        .expect("seed messages");

        sqlx::query("INSERT INTO employee_groups (id, name) VALUES ('team-a', '市场协作')")
            .execute(&pool)
            .await
            .expect("seed employee_groups");

        sqlx::query(
            "INSERT INTO agent_employees (id, employee_id, name, role_id) VALUES ('employee-row-1', 'emp-1', '张三', 'role-1')",
        )
        .execute(&pool)
        .await
        .expect("seed agent_employees");

        let sessions = list_sessions_with_pool(&pool, permission_mode_label_for_display)
            .await
            .expect("list sessions should succeed");

        assert_eq!(sessions[0]["id"], "session-employee");
        assert_eq!(sessions[0]["display_title"], "张三");
        assert_eq!(sessions[0]["employee_name"], "张三");
        assert_eq!(sessions[1]["id"], "session-team");
        assert_eq!(sessions[1]["display_title"], "市场协作");
        assert_eq!(sessions[1]["employee_name"], "");
        assert_eq!(sessions[2]["id"], "session-general-generic-first");
        assert_eq!(sessions[2]["display_title"], "修复登录接口超时问题");
        assert_eq!(sessions[2]["employee_name"], "");
        assert_eq!(sessions[3]["id"], "session-general");
        assert_eq!(sessions[3]["display_title"], "帮我整理本周销售周报");
        assert_eq!(sessions[3]["employee_name"], "");
    }

    #[tokio::test]
    async fn export_session_markdown_includes_structured_run_stopped_summary() {
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
                permission_mode TEXT NOT NULL DEFAULT 'standard',
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

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES ('session-export', 'skill-1', '导出测试', '2026-03-16T00:00:00Z', 'model-1', 'standard', '', '', 'general', '')",
        )
        .execute(&pool)
        .await
        .expect("seed session");

        let journal_dir = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());

        append_session_run_event_with_pool(
            &pool,
            &journal,
            "session-export",
            SessionRunEvent::RunStarted {
                run_id: "run-stop-1".to_string(),
                user_message_id: "user-1".to_string(),
            },
        )
        .await
        .expect("append run started");

        append_session_run_event_with_pool(
            &pool,
            &journal,
            "session-export",
            SessionRunEvent::RunStopped {
                run_id: "run-stop-1".to_string(),
                stop_reason: RunStopReason::loop_detected(
                    "工具 browser_snapshot 已连续 6 次返回相同结果。",
                )
                .with_last_completed_step("已填写封面标题"),
                turn_state: None,
            },
        )
        .await
        .expect("append run stopped");

        let markdown = export_session_markdown_with_pool(&pool, "session-export", Some(&journal))
            .await
            .expect("export markdown");

        assert!(markdown.contains("## 恢复的运行记录"));
        assert!(markdown.contains("任务疑似卡住，已自动停止"));
        assert!(markdown.contains("工具 browser_snapshot 已连续 6 次返回相同结果。"));
        assert!(markdown.contains("最后完成步骤：已填写封面标题"));
    }

    #[tokio::test]
    async fn export_session_markdown_skips_recovered_buffer_when_structured_assistant_text_matches()
    {
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
                permission_mode TEXT NOT NULL DEFAULT 'standard',
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

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES ('session-structured-export', 'skill-1', '结构化导出去重', '2026-03-19T00:00:00Z', 'model-1', 'standard', '', '', 'general', '')",
        )
        .execute(&pool)
        .await
        .expect("seed session");

        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at)
             VALUES
             ('user-1', 'session-structured-export', 'user', '继续执行', '2026-03-19T00:00:01Z'),
             ('assistant-1', 'session-structured-export', 'assistant', ?, '2026-03-19T00:00:02Z')",
        )
        .bind(r#"{"text":"让我先检查正确的目录路径。","items":[{"type":"text","content":"让我先检查正确的目录路径。"}]}"#)
        .execute(&pool)
        .await
        .expect("seed messages");

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES ('run-1', 'session-structured-export', 'user-1', 'assistant-1', 'completed', '让我先检查正确的目录路径。', '', '', '2026-03-19T00:00:01Z', '2026-03-19T00:00:03Z')",
        )
        .execute(&pool)
        .await
        .expect("seed session run");

        let journal_dir = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());
        append_session_run_event_with_pool(
            &pool,
            &journal,
            "session-structured-export",
            SessionRunEvent::RunStarted {
                run_id: "run-1".to_string(),
                user_message_id: "user-1".to_string(),
            },
        )
        .await
        .expect("append run started");
        append_session_run_event_with_pool(
            &pool,
            &journal,
            "session-structured-export",
            SessionRunEvent::AssistantChunkAppended {
                run_id: "run-1".to_string(),
                chunk: "让我先检查正确的目录路径。".to_string(),
            },
        )
        .await
        .expect("append assistant chunk");
        append_session_run_event_with_pool(
            &pool,
            &journal,
            "session-structured-export",
            SessionRunEvent::RunCompleted {
                run_id: "run-1".to_string(),
                turn_state: None,
            },
        )
        .await
        .expect("append run completed");

        let markdown =
            export_session_markdown_with_pool(&pool, "session-structured-export", Some(&journal))
                .await
                .expect("export markdown");

        assert!(markdown.contains("让我先检查正确的目录路径。"));
        assert!(!markdown.contains("## 恢复的运行记录"));
        assert_eq!(markdown.matches("让我先检查正确的目录路径。").count(), 1);
    }

    #[tokio::test]
    async fn export_session_markdown_includes_recovered_compaction_boundary_summary() {
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
                permission_mode TEXT NOT NULL DEFAULT 'standard',
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

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES ('session-compaction-export', 'skill-1', '压缩恢复导出', '2026-04-08T00:00:00Z', 'model-1', 'standard', '', '', 'general', '')",
        )
        .execute(&pool)
        .await
        .expect("seed session");

        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at)
             VALUES ('user-1', 'session-compaction-export', 'user', '继续上次任务', '2026-04-08T00:00:01Z')",
        )
        .execute(&pool)
        .await
        .expect("seed user message");

        let journal_dir = tempdir().expect("journal tempdir");
        let session_dir = journal_dir.path().join("session-compaction-export");
        tokio::fs::create_dir_all(&session_dir)
            .await
            .expect("create journal session dir");
        tokio::fs::write(
            session_dir.join("state.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "session_id": "session-compaction-export",
                "current_run_id": null,
                "runs": [{
                    "run_id": "run-recover-1",
                    "user_message_id": "user-1",
                    "status": "failed",
                    "buffered_text": "正在继续处理剩余步骤",
                    "last_error_kind": "max_turns",
                    "last_error_message": "达到最大迭代次数",
                    "turn_state": {
                        "execution_lane": "open_task",
                        "selected_runner": "open_task",
                        "selected_skill": null,
                        "fallback_reason": null,
                        "allowed_tools": ["read", "exec"],
                        "invoked_skills": [],
                        "partial_assistant_text": "正在继续处理剩余步骤",
                        "tool_failure_streak": 0,
                        "reconstructed_history_len": 5,
                        "compaction_boundary": {
                            "transcript_path": "temp/transcripts/session-1.json",
                            "original_tokens": 4096,
                            "compacted_tokens": 1024,
                            "summary": "压缩摘要"
                        }
                    }
                }]
            }))
            .expect("serialize state json"),
        )
        .await
        .expect("write journal state");

        let journal = SessionJournalStore::new(journal_dir.path().to_path_buf());
        let markdown =
            export_session_markdown_with_pool(&pool, "session-compaction-export", Some(&journal))
                .await
                .expect("export markdown");

        assert!(markdown.contains("## 恢复的运行记录"));
        assert!(markdown.contains("压缩边界"));
        assert!(markdown.contains("4096 -> 1024"));
        assert!(markdown.contains("temp/transcripts/session-1.json"));
        assert!(markdown.contains("压缩摘要"));
    }

    #[tokio::test]
    async fn list_sessions_with_pool_projects_runtime_status() {
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
                permission_mode TEXT NOT NULL DEFAULT 'standard',
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
            "CREATE TABLE im_thread_sessions (
                thread_id TEXT NOT NULL,
                employee_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                route_session_key TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                channel TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (thread_id, employee_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_thread_sessions table");

        sqlx::query(
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                role_id TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        sqlx::query(
            "CREATE TABLE employee_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create employee_groups table");

        sqlx::query(
            "CREATE TABLE session_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_message_id TEXT NOT NULL,
                assistant_message_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL,
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
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES
             ('session-failed', 'skill-1', '失败会话', '2026-03-16T00:00:04Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-completed', 'skill-1', '完成会话', '2026-03-16T00:00:03Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-waiting', 'skill-1', '审批会话', '2026-03-16T00:00:02Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-running', 'skill-1', '运行会话', '2026-03-16T00:00:01Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-idle', 'skill-1', '空闲会话', '2026-03-16T00:00:00Z', 'model-1', 'standard', '', '', 'general', '')",
        )
        .execute(&pool)
        .await
        .expect("seed sessions");

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES
             ('run-failed', 'session-failed', 'user-1', '', 'failed', '', 'billing', '额度不足', '2026-03-16T00:00:04Z', '2026-03-16T00:00:05Z'),
             ('run-completed', 'session-completed', 'user-2', 'assistant-2', 'completed', '已完成', '', '', '2026-03-16T00:00:03Z', '2026-03-16T00:00:04Z'),
             ('run-waiting', 'session-waiting', 'user-3', '', 'waiting_approval', '等待确认', '', '', '2026-03-16T00:00:02Z', '2026-03-16T00:00:06Z'),
             ('run-running', 'session-running', 'user-4', '', 'thinking', '执行中', '', '', '2026-03-16T00:00:01Z', '2026-03-16T00:00:07Z')",
        )
        .execute(&pool)
        .await
        .expect("seed session_runs");

        let sessions = list_sessions_with_pool(&pool, permission_mode_label_for_display)
            .await
            .expect("list sessions should succeed");

        assert_eq!(sessions[0]["id"], "session-failed");
        assert_eq!(sessions[0]["runtime_status"], "failed");
        assert_eq!(sessions[1]["id"], "session-completed");
        assert_eq!(sessions[1]["runtime_status"], "completed");
        assert_eq!(sessions[2]["id"], "session-waiting");
        assert_eq!(sessions[2]["runtime_status"], "waiting_approval");
        assert_eq!(sessions[3]["id"], "session-running");
        assert_eq!(sessions[3]["runtime_status"], "running");
        assert_eq!(sessions[4]["id"], "session-idle");
        assert!(sessions[4]["runtime_status"].is_null());
    }
}
