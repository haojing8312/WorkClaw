use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tauri::{AppHandle, Manager};
use anyhow::Result;

fn build_builtin_manifest_json(skill_id: &str, skill_markdown: &str) -> String {
    let builtin_config = crate::agent::skill_config::SkillConfig::parse(
        skill_markdown,
    );
    let builtin_name = builtin_config
        .name
        .unwrap_or_else(|| skill_id.to_string());
    let builtin_description = builtin_config
        .description
        .unwrap_or_default();

    serde_json::json!({
        "id": skill_id,
        "name": builtin_name,
        "description": builtin_description,
        "version": "1.0.0",
        "author": "SkillMint",
        "recommended_model": "",
        "tags": [],
        "created_at": "2026-01-01T00:00:00Z",
        "username_hint": null,
        "encrypted_verify": ""
    })
    .to_string()
}

async fn sync_builtin_skills(pool: &SqlitePool) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    for entry in crate::builtin_skills::builtin_skill_entries() {
        let builtin_json = build_builtin_manifest_json(entry.id, entry.markdown);
        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES (?, ?, ?, '', '', 'builtin')
             ON CONFLICT(id) DO UPDATE SET
               manifest = excluded.manifest,
               username = '',
               pack_path = '',
               source_type = 'builtin'"
        )
        .bind(entry.id)
        .bind(&builtin_json)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn init_db(app: &AppHandle) -> Result<SqlitePool> {
    let app_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;
    let db_path = app_dir.join("skillmint.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS installed_skills (
            id TEXT PRIMARY KEY,
            manifest TEXT NOT NULL,
            installed_at TEXT NOT NULL,
            last_used_at TEXT,
            username TEXT NOT NULL,
            pack_path TEXT NOT NULL DEFAULT ''
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            skill_id TEXT NOT NULL,
            title TEXT,
            created_at TEXT NOT NULL,
            model_id TEXT NOT NULL,
            permission_mode TEXT NOT NULL DEFAULT 'accept_edits',
            work_dir TEXT NOT NULL DEFAULT ''
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS model_configs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            api_format TEXT NOT NULL,
            base_url TEXT NOT NULL,
            model_name TEXT NOT NULL,
            is_default INTEGER DEFAULT 0,
            api_key TEXT NOT NULL DEFAULT ''
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS mcp_servers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            command TEXT NOT NULL,
            args TEXT NOT NULL DEFAULT '[]',
            env TEXT NOT NULL DEFAULT '{}',
            enabled INTEGER DEFAULT 1,
            created_at TEXT NOT NULL
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )"
    )
    .execute(&pool)
    .await?;

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
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS provider_capabilities (
            provider_id TEXT NOT NULL,
            capability TEXT NOT NULL,
            supported INTEGER NOT NULL DEFAULT 1,
            priority INTEGER NOT NULL DEFAULT 100,
            default_model TEXT NOT NULL DEFAULT '',
            fallback_models_json TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (provider_id, capability)
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS model_catalog_cache (
            provider_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            raw_json TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            ttl_seconds INTEGER NOT NULL DEFAULT 3600,
            PRIMARY KEY (provider_id, model_id)
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS routing_policies (
            capability TEXT PRIMARY KEY,
            primary_provider_id TEXT NOT NULL,
            primary_model TEXT NOT NULL DEFAULT '',
            fallback_chain_json TEXT NOT NULL DEFAULT '[]',
            timeout_ms INTEGER NOT NULL DEFAULT 60000,
            retry_count INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 1
        )"
    )
    .execute(&pool)
    .await?;

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
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_event_dedup (
            event_id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_thread_bindings (
            thread_id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL DEFAULT '',
            scenario_template TEXT NOT NULL DEFAULT 'opportunity_review',
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_thread_roles (
            thread_id TEXT NOT NULL,
            role_id TEXT NOT NULL,
            role_order INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (thread_id, role_id)
        )"
    )
    .execute(&pool)
    .await?;

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
            priority INTEGER NOT NULL DEFAULT 100,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_inbox_events (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL,
            thread_id TEXT NOT NULL,
            message_id TEXT NOT NULL DEFAULT '',
            text_preview TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_employees (
            id TEXT PRIMARY KEY,
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
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_employee_skills (
            employee_id TEXT NOT NULL,
            skill_id TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (employee_id, skill_id)
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_thread_employee_bindings (
            thread_id TEXT NOT NULL,
            employee_id TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            role_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (thread_id, employee_id)
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS im_thread_sessions (
            thread_id TEXT NOT NULL,
            employee_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            route_session_key TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (thread_id, employee_id)
        )"
    )
    .execute(&pool)
    .await?;

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
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS skill_i18n_cache (
            cache_key TEXT PRIMARY KEY,
            source_text TEXT NOT NULL,
            translated_text TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"
    )
    .execute(&pool)
    .await?;

    // Migration: add api_key column for databases created before this column existed
    let _ = sqlx::query("ALTER TABLE model_configs ADD COLUMN api_key TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;

    // Migration: add permission_mode column to sessions
    let _ = sqlx::query("ALTER TABLE sessions ADD COLUMN permission_mode TEXT NOT NULL DEFAULT 'accept_edits'")
        .execute(&pool)
        .await;

    // Migration: add source_type column to installed_skills（区分加密 vs 本地 Skill）
    let _ = sqlx::query("ALTER TABLE installed_skills ADD COLUMN source_type TEXT NOT NULL DEFAULT 'encrypted'")
        .execute(&pool)
        .await;

    // Migration: add work_dir column to sessions（每会话独立工作目录）
    let _ = sqlx::query("ALTER TABLE sessions ADD COLUMN work_dir TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;

    // Migration: employee-level Feishu credentials
    let _ = sqlx::query("ALTER TABLE agent_employees ADD COLUMN feishu_app_id TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE agent_employees ADD COLUMN feishu_app_secret TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE agent_employees ADD COLUMN openclaw_agent_id TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE agent_employees ADD COLUMN routing_priority INTEGER NOT NULL DEFAULT 100")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE agent_employees ADD COLUMN enabled_scopes_json TEXT NOT NULL DEFAULT '[]'")
        .execute(&pool)
        .await;
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_employees_role_id_unique ON agent_employees(role_id)",
    )
    .execute(&pool)
    .await;
    let _ = sqlx::query("ALTER TABLE im_thread_sessions ADD COLUMN route_session_key TEXT NOT NULL DEFAULT ''")
        .execute(&pool)
        .await;
    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_im_thread_sessions_route_key ON im_thread_sessions(route_session_key)",
    )
    .execute(&pool)
    .await;

    // 默认路由配置
    let _ = sqlx::query("INSERT OR IGNORE INTO app_settings (key, value) VALUES ('route_max_call_depth', '4')")
        .execute(&pool)
        .await;
    let _ = sqlx::query("INSERT OR IGNORE INTO app_settings (key, value) VALUES ('route_node_timeout_seconds', '60')")
        .execute(&pool)
        .await;
    let _ = sqlx::query("INSERT OR IGNORE INTO app_settings (key, value) VALUES ('route_retry_count', '0')")
        .execute(&pool)
        .await;
    let _ = sqlx::query("INSERT OR IGNORE INTO app_settings (key, value) VALUES ('runtime_default_work_dir', '')")
        .execute(&pool)
        .await;

    // 内置 Skill：始终存在，无需用户安装，且每次启动同步最新 metadata
    let _ = sync_builtin_skills(&pool).await;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_memory_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE installed_skills (
                id TEXT PRIMARY KEY,
                manifest TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                last_used_at TEXT,
                username TEXT NOT NULL,
                pack_path TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'encrypted'
            )"
        )
        .execute(&pool)
        .await
        .expect("create installed_skills table");

        pool
    }

    #[tokio::test]
    async fn sync_builtin_skills_upserts_manifest_and_source_type() {
        let pool = setup_memory_pool().await;
        let stale_manifest = serde_json::json!({
            "id": "builtin-general",
            "name": "旧名称",
            "description": "旧描述",
            "version": "0.0.1"
        })
        .to_string();

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES ('builtin-general', ?, '2026-01-01T00:00:00Z', 'x', '/tmp', 'local')"
        )
        .bind(stale_manifest)
        .execute(&pool)
        .await
        .expect("seed stale builtin row");

        sync_builtin_skills(&pool)
            .await
            .expect("sync builtin skills");

        let (manifest_json, source_type, username, pack_path): (String, String, String, String) = sqlx::query_as(
            "SELECT manifest, source_type, username, pack_path FROM installed_skills WHERE id = 'builtin-general'"
        )
        .fetch_one(&pool)
        .await
        .expect("query builtin row");

        let manifest: serde_json::Value =
            serde_json::from_str(&manifest_json).expect("parse manifest json");
        let expected: serde_json::Value = serde_json::from_str(&build_builtin_manifest_json(
            crate::builtin_skills::BUILTIN_GENERAL_SKILL_ID,
            crate::builtin_skills::builtin_general_skill_markdown(),
        ))
            .expect("parse expected manifest");

        assert_eq!(manifest["name"], expected["name"]);
        assert_eq!(manifest["description"], expected["description"]);
        assert_eq!(source_type, "builtin");
        assert_eq!(username, "");
        assert_eq!(pack_path, "");
    }

    #[tokio::test]
    async fn sync_builtin_skills_is_idempotent() {
        let pool = setup_memory_pool().await;
        sync_builtin_skills(&pool)
            .await
            .expect("first sync");
        sync_builtin_skills(&pool)
            .await
            .expect("second sync");

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM installed_skills WHERE source_type = 'builtin'"
        )
        .fetch_one(&pool)
        .await
        .expect("count builtin skills");

        assert_eq!(count, crate::builtin_skills::builtin_skill_entries().len() as i64);
    }
}
