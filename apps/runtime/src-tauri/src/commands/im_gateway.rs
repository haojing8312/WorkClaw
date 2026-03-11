use crate::commands::skills::DbState;
use crate::im::types::ImEvent;
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeishuCallbackResult {
    pub accepted: bool,
    pub deduped: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RecentImThread {
    pub thread_id: String,
    pub source: String,
    pub last_text_preview: String,
    pub last_seen_at: String,
}

pub async fn process_im_event(
    pool: &SqlitePool,
    event: ImEvent,
) -> Result<FeishuCallbackResult, String> {
    let thread_id = event.thread_id.clone();
    let message_id = event.message_id.clone().unwrap_or_default();
    let text_preview = event
        .text
        .clone()
        .unwrap_or_default()
        .chars()
        .take(120)
        .collect::<String>();

    let event_id = event
        .event_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}_{}", thread_id, message_id));

    let now = chrono::Utc::now().to_rfc3339();
    let result =
        sqlx::query("INSERT OR IGNORE INTO im_event_dedup (event_id, created_at) VALUES (?, ?)")
            .bind(&event_id)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

    if result.rows_affected() > 0 {
        let source = event.channel.trim();
        sqlx::query(
            "INSERT INTO im_inbox_events (id, event_id, thread_id, message_id, text_preview, source, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&event_id)
        .bind(thread_id)
        .bind(message_id)
        .bind(text_preview)
        .bind(if source.is_empty() { "app" } else { source })
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(FeishuCallbackResult {
        accepted: true,
        deduped: result.rows_affected() == 0,
    })
}

pub async fn list_recent_im_threads_with_pool(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<RecentImThread>, String> {
    let lim = limit.clamp(1, 200);
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT e.thread_id, e.source, e.text_preview, e.created_at
         FROM im_inbox_events e
         INNER JOIN (
           SELECT thread_id, MAX(created_at) AS latest_at
           FROM im_inbox_events
           GROUP BY thread_id
         ) g ON g.thread_id = e.thread_id AND g.latest_at = e.created_at
         ORDER BY e.created_at DESC
         LIMIT ?",
    )
    .bind(lim)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(
            |(thread_id, source, last_text_preview, last_seen_at)| RecentImThread {
                thread_id,
                source,
                last_text_preview,
                last_seen_at,
            },
        )
        .collect())
}

#[tauri::command]
pub async fn handle_feishu_callback(
    payload: String,
    db: State<'_, DbState>,
) -> Result<FeishuCallbackResult, String> {
    // Compatibility alias: direct Feishu callback style payload.
    // Preferred new ingress is handle_openclaw_event.
    let event: ImEvent =
        serde_json::from_str(&payload).map_err(|e| format!("invalid callback payload: {}", e))?;
    process_im_event(&db.0, event).await
}

#[tauri::command]
pub async fn list_recent_im_threads(
    limit: Option<i64>,
    db: State<'_, DbState>,
) -> Result<Vec<RecentImThread>, String> {
    list_recent_im_threads_with_pool(&db.0, limit.unwrap_or(20)).await
}
