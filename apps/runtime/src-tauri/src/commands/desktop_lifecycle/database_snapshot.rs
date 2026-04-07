use crate::diagnostics;
use crate::runtime_environment::runtime_paths_from_app;
use serde_json::Value;
use sqlx::SqlitePool;
use tauri::AppHandle;

pub(crate) async fn collect_database_counts(pool: &SqlitePool) -> Value {
    let session_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions")
        .fetch_one(pool)
        .await
        .ok();
    let message_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM messages")
        .fetch_one(pool)
        .await
        .ok();
    serde_json::json!({
        "session_count": session_count,
        "message_count": message_count,
    })
}

pub(crate) fn collect_database_storage_snapshot(app: &AppHandle) -> Value {
    match runtime_paths_from_app(app) {
        Ok(runtime_paths) => serde_json::json!({
            "runtime_root": runtime_paths.root.to_string_lossy().to_string(),
            "sqlite_files": diagnostics::collect_sqlite_storage_snapshot(&runtime_paths.root),
        }),
        Err(error) => serde_json::json!({
            "runtime_root_error": error.to_string(),
        }),
    }
}
