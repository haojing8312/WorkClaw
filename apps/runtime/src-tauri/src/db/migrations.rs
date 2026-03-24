use anyhow::Result;
use sqlx::SqlitePool;

pub(super) async fn apply_legacy_migrations(pool: &SqlitePool) -> Result<()> {
    let _ = sqlx::query(
        "ALTER TABLE im_routing_bindings ADD COLUMN connector_meta_json TEXT NOT NULL DEFAULT '{}'",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query("ALTER TABLE model_configs ADD COLUMN api_key TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;

    let _ = sqlx::query(
        "ALTER TABLE sessions ADD COLUMN permission_mode TEXT NOT NULL DEFAULT 'accept_edits'",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE installed_skills ADD COLUMN source_type TEXT NOT NULL DEFAULT 'encrypted'",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query("ALTER TABLE sessions ADD COLUMN work_dir TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE sessions ADD COLUMN employee_id TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE sessions ADD COLUMN session_mode TEXT NOT NULL DEFAULT 'general'")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE sessions ADD COLUMN team_id TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE messages ADD COLUMN content_json TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query(
        "ALTER TABLE session_runs ADD COLUMN assistant_message_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query(
        "ALTER TABLE agent_employees ADD COLUMN feishu_app_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE agent_employees ADD COLUMN feishu_app_secret TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE agent_employees ADD COLUMN openclaw_agent_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE agent_employees ADD COLUMN routing_priority INTEGER NOT NULL DEFAULT 100",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE agent_employees ADD COLUMN enabled_scopes_json TEXT NOT NULL DEFAULT '[]'",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE agent_employees ADD COLUMN employee_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "UPDATE agent_employees SET employee_id = role_id WHERE TRIM(employee_id) = ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_employees_employee_id_unique ON agent_employees(employee_id)",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_employees_role_id_unique ON agent_employees(role_id)",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query(
        "ALTER TABLE im_thread_sessions ADD COLUMN route_session_key TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    ensure_im_thread_sessions_channel_column(pool).await?;
    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_im_thread_sessions_route_key ON im_thread_sessions(route_session_key)",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_employee_groups_coordinator ON employee_groups(coordinator_employee_id)",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_employee_group_rules_group_id ON employee_group_rules(group_id)",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_runs_group_id ON group_runs(group_id)")
        .execute(pool)
        .await;
    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_runs_state ON group_runs(state)")
        .execute(pool)
        .await;
    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_group_run_events_run_id ON group_run_events(run_id)",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_group_run_steps_run_id ON group_run_steps(run_id)",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_group_run_steps_round_no ON group_run_steps(round_no)",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query("ALTER TABLE employee_groups ADD COLUMN template_id TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query(
        "ALTER TABLE employee_groups ADD COLUMN entry_employee_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE employee_groups ADD COLUMN review_mode TEXT NOT NULL DEFAULT 'none'",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE employee_groups ADD COLUMN execution_mode TEXT NOT NULL DEFAULT 'sequential'",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE employee_groups ADD COLUMN visibility_mode TEXT NOT NULL DEFAULT 'internal'",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE employee_groups ADD COLUMN is_bootstrap_seeded INTEGER NOT NULL DEFAULT 0",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE employee_groups ADD COLUMN config_json TEXT NOT NULL DEFAULT '{}'",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query("ALTER TABLE group_runs ADD COLUMN current_phase TEXT NOT NULL DEFAULT 'plan'")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE group_runs ADD COLUMN entry_session_id TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE group_runs ADD COLUMN main_employee_id TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE group_runs ADD COLUMN review_round INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE group_runs ADD COLUMN status_reason TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE group_runs ADD COLUMN template_version TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query(
        "ALTER TABLE group_runs ADD COLUMN waiting_for_employee_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE group_runs ADD COLUMN waiting_for_user INTEGER NOT NULL DEFAULT 0",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query(
        "ALTER TABLE group_run_steps ADD COLUMN parent_step_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query("ALTER TABLE group_run_steps ADD COLUMN phase TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query(
        "ALTER TABLE group_run_steps ADD COLUMN step_kind TEXT NOT NULL DEFAULT 'execute'",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE group_run_steps ADD COLUMN requires_review INTEGER NOT NULL DEFAULT 0",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE group_run_steps ADD COLUMN review_status TEXT NOT NULL DEFAULT 'not_required'",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query("ALTER TABLE group_run_steps ADD COLUMN attempt_no INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE group_run_steps ADD COLUMN session_id TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query(
        "ALTER TABLE group_run_steps ADD COLUMN dispatch_source_employee_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE group_run_steps ADD COLUMN input_summary TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE group_run_steps ADD COLUMN output_summary TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "ALTER TABLE group_run_steps ADD COLUMN visibility TEXT NOT NULL DEFAULT 'internal'",
    )
    .execute(pool)
    .await;

    Ok(())
}

pub(super) async fn ensure_im_thread_sessions_channel_column(pool: &SqlitePool) -> Result<()> {
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('im_thread_sessions')")
            .fetch_all(pool)
            .await?;
    if columns.iter().any(|name| name == "channel") {
        return Ok(());
    }

    let _ = sqlx::query("ALTER TABLE im_thread_sessions ADD COLUMN channel TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;

    Ok(())
}
