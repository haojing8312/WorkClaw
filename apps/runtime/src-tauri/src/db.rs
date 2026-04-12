use crate::runtime_environment::initialize_runtime_environment;
use crate::runtime_paths::RuntimePaths;
use anyhow::Result;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tauri::AppHandle;

mod migrations;
mod schema;
mod seed;

use migrations::apply_legacy_migrations;
use schema::apply_current_schema;
use seed::seed_runtime_defaults;

#[cfg(test)]
use migrations::ensure_im_thread_sessions_channel_column;
#[cfg(test)]
use seed::{build_builtin_manifest_json, sync_builtin_skills_with_root};

#[cfg_attr(not(test), allow(dead_code))]
pub async fn init_db(app: &AppHandle) -> Result<SqlitePool> {
    let runtime_environment = initialize_runtime_environment(app).map_err(anyhow::Error::msg)?;
    init_db_at_runtime_paths(&runtime_environment.paths).await
}

pub async fn init_db_at_runtime_paths(runtime_paths: &RuntimePaths) -> Result<SqlitePool> {
    std::fs::create_dir_all(&runtime_paths.root)?;
    let db_path = runtime_paths.database.db_path.clone();
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    apply_current_schema(&pool).await?;
    apply_legacy_migrations(&pool).await?;
    seed_runtime_defaults(&pool, &runtime_paths.root).await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
            )",
        )
        .execute(&pool)
        .await
        .expect("create installed_skills table");

        pool
    }

    #[tokio::test]
    async fn sync_builtin_skills_upserts_manifest_and_source_type() {
        let pool = setup_memory_pool().await;
        let vendor_root = tempdir().expect("create vendor root");
        let stale_manifest = serde_json::json!({
            "id": "builtin-general",
            "name": "旧名称",
            "description": "旧描述",
            "version": "0.0.1"
        })
        .to_string();

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES ('builtin-general', ?, '2026-01-01T00:00:00Z', 'x', '/tmp', 'local')",
        )
        .bind(stale_manifest)
        .execute(&pool)
        .await
        .expect("seed stale builtin row");

        sync_builtin_skills_with_root(&pool, vendor_root.path())
            .await
            .expect("sync builtin skills");

        let (manifest_json, source_type, username, pack_path): (String, String, String, String) = sqlx::query_as(
            "SELECT manifest, source_type, username, pack_path FROM installed_skills WHERE id = 'builtin-general'",
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
        assert_eq!(source_type, "vendored");
        assert_eq!(username, "");
        assert!(pack_path.contains("builtin-general"));
        assert!(std::path::Path::new(&pack_path).join("SKILL.md").exists());
    }

    #[tokio::test]
    async fn sync_builtin_skills_is_idempotent() {
        let pool = setup_memory_pool().await;
        let vendor_root = tempdir().expect("create vendor root");
        sync_builtin_skills_with_root(&pool, vendor_root.path())
            .await
            .expect("first sync");
        sync_builtin_skills_with_root(&pool, vendor_root.path())
            .await
            .expect("second sync");

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM installed_skills WHERE source_type = 'vendored'")
                .fetch_one(&pool)
                .await
                .expect("count vendored skills");

        assert_eq!(
            count,
            crate::builtin_skills::builtin_skill_entries().len() as i64
        );
    }

    #[tokio::test]
    async fn ensure_im_thread_sessions_channel_column_migrates_legacy_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

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

        ensure_im_thread_sessions_channel_column(&pool)
            .await
            .expect("migrate legacy im_thread_sessions schema");

        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('im_thread_sessions')")
                .fetch_all(&pool)
                .await
                .expect("load im_thread_sessions columns");

        assert!(columns.iter().any(|name| name == "channel"));
    }
}
