use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::path::PathBuf;
use tempfile::TempDir;

/// 创建临时 SQLite 数据库，复制完整 schema（与 db.rs 保持一致）
pub async fn setup_test_db() -> (SqlitePool, TempDir) {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .unwrap();

    // 创建所有表（与 db.rs init_db 保持一致）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS installed_skills (
            id TEXT PRIMARY KEY,
            manifest TEXT NOT NULL,
            installed_at TEXT NOT NULL,
            last_used_at TEXT,
            username TEXT NOT NULL,
            pack_path TEXT NOT NULL DEFAULT '',
            source_type TEXT NOT NULL DEFAULT 'encrypted'
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
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
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS model_configs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            api_format TEXT NOT NULL,
            base_url TEXT NOT NULL,
            model_name TEXT NOT NULL,
            is_default INTEGER DEFAULT 0,
            api_key TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS mcp_servers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            command TEXT NOT NULL,
            args TEXT NOT NULL DEFAULT '[]',
            env TEXT NOT NULL DEFAULT '{}',
            enabled INTEGER DEFAULT 1,
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS provider_configs (
            id TEXT PRIMARY KEY,
            provider_key TEXT NOT NULL,
            display_name TEXT NOT NULL,
            protocol_type TEXT NOT NULL,
            base_url TEXT NOT NULL,
            auth_type TEXT NOT NULL DEFAULT 'api_key',
            api_key_encrypted TEXT NOT NULL DEFAULT '',
            org_id TEXT NOT NULL DEFAULT '',
            extra_json TEXT NOT NULL DEFAULT '{}',
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS provider_capabilities (
            provider_id TEXT NOT NULL,
            capability TEXT NOT NULL,
            supported INTEGER NOT NULL DEFAULT 1,
            priority INTEGER NOT NULL DEFAULT 100,
            default_model TEXT NOT NULL DEFAULT '',
            fallback_models_json TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (provider_id, capability)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS model_catalog_cache (
            provider_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            raw_json TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            ttl_seconds INTEGER NOT NULL DEFAULT 3600,
            PRIMARY KEY (provider_id, model_id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS routing_policies (
            capability TEXT PRIMARY KEY,
            primary_provider_id TEXT NOT NULL,
            primary_model TEXT NOT NULL DEFAULT '',
            fallback_chain_json TEXT NOT NULL DEFAULT '[]',
            timeout_ms INTEGER NOT NULL DEFAULT 60000,
            retry_count INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 1
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS route_attempt_logs (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            capability TEXT NOT NULL,
            api_format TEXT NOT NULL,
            model_name TEXT NOT NULL,
            attempt_index INTEGER NOT NULL DEFAULT 1,
            retry_index INTEGER NOT NULL DEFAULT 0,
            error_kind TEXT NOT NULL DEFAULT '',
            success INTEGER NOT NULL DEFAULT 0,
            error_message TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_event_dedup (
            event_id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_thread_bindings (
            thread_id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL DEFAULT '',
            scenario_template TEXT NOT NULL DEFAULT 'opportunity_review',
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_thread_roles (
            thread_id TEXT NOT NULL,
            role_id TEXT NOT NULL,
            role_order INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (thread_id, role_id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_routing_bindings (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            account_id TEXT NOT NULL DEFAULT '',
            peer_kind TEXT NOT NULL DEFAULT '',
            peer_id TEXT NOT NULL DEFAULT '',
            guild_id TEXT NOT NULL DEFAULT '',
            team_id TEXT NOT NULL DEFAULT '',
            role_ids_json TEXT NOT NULL DEFAULT '[]',
            connector_meta_json TEXT NOT NULL DEFAULT '{}',
            priority INTEGER NOT NULL DEFAULT 100,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_inbox_events (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL,
            thread_id TEXT NOT NULL,
            message_id TEXT NOT NULL DEFAULT '',
            text_preview TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_employees (
            id TEXT PRIMARY KEY,
            employee_id TEXT NOT NULL DEFAULT '',
            name TEXT NOT NULL,
            role_id TEXT NOT NULL,
            persona TEXT NOT NULL DEFAULT '',
            feishu_open_id TEXT NOT NULL DEFAULT '',
            feishu_app_id TEXT NOT NULL DEFAULT '',
            feishu_app_secret TEXT NOT NULL DEFAULT '',
            primary_skill_id TEXT NOT NULL DEFAULT '',
            default_work_dir TEXT NOT NULL DEFAULT '',
            openclaw_agent_id TEXT NOT NULL DEFAULT '',
            routing_priority INTEGER NOT NULL DEFAULT 100,
            enabled_scopes_json TEXT NOT NULL DEFAULT '[]',
            enabled INTEGER NOT NULL DEFAULT 1,
            is_default INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_employees_employee_id_unique ON agent_employees(employee_id)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_employees_role_id_unique ON agent_employees(role_id)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_employee_skills (
            employee_id TEXT NOT NULL,
            skill_id TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (employee_id, skill_id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS employee_groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            coordinator_employee_id TEXT NOT NULL,
            member_employee_ids_json TEXT NOT NULL DEFAULT '[]',
            member_count INTEGER NOT NULL DEFAULT 1 CHECK (member_count >= 1 AND member_count <= 10),
            template_id TEXT NOT NULL DEFAULT '',
            entry_employee_id TEXT NOT NULL DEFAULT '',
            review_mode TEXT NOT NULL DEFAULT 'none',
            execution_mode TEXT NOT NULL DEFAULT 'sequential',
            visibility_mode TEXT NOT NULL DEFAULT 'internal',
            is_bootstrap_seeded INTEGER NOT NULL DEFAULT 0,
            config_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS group_runs (
            id TEXT PRIMARY KEY,
            group_id TEXT NOT NULL,
            session_id TEXT NOT NULL DEFAULT '',
            user_goal TEXT NOT NULL DEFAULT '',
            state TEXT NOT NULL DEFAULT 'planning',
            current_round INTEGER NOT NULL DEFAULT 0,
            current_phase TEXT NOT NULL DEFAULT 'plan',
            entry_session_id TEXT NOT NULL DEFAULT '',
            main_employee_id TEXT NOT NULL DEFAULT '',
            review_round INTEGER NOT NULL DEFAULT 0,
            status_reason TEXT NOT NULL DEFAULT '',
            template_version TEXT NOT NULL DEFAULT '',
            waiting_for_employee_id TEXT NOT NULL DEFAULT '',
            waiting_for_user INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS group_run_steps (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            round_no INTEGER NOT NULL DEFAULT 0,
            parent_step_id TEXT NOT NULL DEFAULT '',
            assignee_employee_id TEXT NOT NULL DEFAULT '',
            dispatch_source_employee_id TEXT NOT NULL DEFAULT '',
            phase TEXT NOT NULL DEFAULT '',
            step_type TEXT NOT NULL DEFAULT 'execute',
            step_kind TEXT NOT NULL DEFAULT 'execute',
            input TEXT NOT NULL DEFAULT '',
            input_summary TEXT NOT NULL DEFAULT '',
            output TEXT NOT NULL DEFAULT '',
            output_summary TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'pending',
            requires_review INTEGER NOT NULL DEFAULT 0,
            review_status TEXT NOT NULL DEFAULT 'not_required',
            attempt_no INTEGER NOT NULL DEFAULT 0,
            session_id TEXT NOT NULL DEFAULT '',
            visibility TEXT NOT NULL DEFAULT 'internal',
            started_at TEXT NOT NULL DEFAULT '',
            finished_at TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS employee_group_rules (
            id TEXT PRIMARY KEY,
            group_id TEXT NOT NULL,
            from_employee_id TEXT NOT NULL,
            to_employee_id TEXT NOT NULL,
            relation_type TEXT NOT NULL,
            phase_scope TEXT NOT NULL DEFAULT '',
            required INTEGER NOT NULL DEFAULT 0,
            priority INTEGER NOT NULL DEFAULT 100,
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS group_run_events (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            step_id TEXT NOT NULL DEFAULT '',
            event_type TEXT NOT NULL,
            payload_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS seeded_team_templates (
            template_id TEXT PRIMARY KEY,
            template_version TEXT NOT NULL,
            instance_group_id TEXT NOT NULL DEFAULT '',
            instance_employee_ids_json TEXT NOT NULL DEFAULT '[]',
            seed_mode TEXT NOT NULL DEFAULT 'first_run',
            seeded_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_thread_employee_bindings (
            thread_id TEXT NOT NULL,
            employee_id TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            role_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (thread_id, employee_id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_thread_sessions (
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
    .unwrap();

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_im_thread_sessions_route_key ON im_thread_sessions(route_session_key)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_employee_groups_coordinator ON employee_groups(coordinator_employee_id)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_runs_group_id ON group_runs(group_id)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_runs_state ON group_runs(state)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_employee_group_rules_group_id ON employee_group_rules(group_id)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_group_run_events_run_id ON group_run_events(run_id)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_run_steps_run_id ON group_run_steps(run_id)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_group_run_steps_round_no ON group_run_steps(round_no)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_message_links (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            employee_id TEXT NOT NULL DEFAULT '',
            direction TEXT NOT NULL,
            im_event_id TEXT NOT NULL DEFAULT '',
            im_message_id TEXT NOT NULL DEFAULT '',
            app_message_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS skill_i18n_cache (
            cache_key TEXT PRIMARY KEY,
            source_text TEXT NOT NULL,
            translated_text TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS clawhub_http_cache (
            cache_key TEXT PRIMARY KEY,
            body TEXT NOT NULL,
            fetched_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    (pool, tmp)
}

/// 创建测试用 Skill 目录（含 SKILL.md + templates）
pub fn create_test_skill_dir() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let skill_dir = tmp.path().join("test-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: test-skill\ndescription: A test skill for E2E testing\nallowed_tools: \"ReadFile, Glob\"\nuser-invocable: true\n---\n\nYou are a helpful test assistant.\n",
    )
    .unwrap();
    let templates = skill_dir.join("templates");
    std::fs::create_dir_all(&templates).unwrap();
    std::fs::write(templates.join("greeting.md"), "Hello, {{name}}!").unwrap();
    (tmp, skill_dir)
}
