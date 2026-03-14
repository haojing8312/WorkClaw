use super::chat_session_io;
use crate::agent::compactor;
use runtime_executor_core::estimate_tokens;
use std::path::Path;

#[derive(serde::Serialize)]
pub struct CompactionResult {
    pub original_tokens: usize,
    pub new_tokens: usize,
    pub summary: String,
}

pub async fn compact_context_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    app_data_dir: &Path,
) -> Result<CompactionResult, String> {
    let (messages, api_format, base_url, api_key, model_name) =
        chat_session_io::load_compaction_inputs_with_pool(pool, session_id).await?;

    let original_tokens = estimate_tokens(&messages);
    let transcript_dir = app_data_dir.join("transcripts");
    std::fs::create_dir_all(&transcript_dir).map_err(|e| e.to_string())?;

    let transcript_path = compactor::save_transcript(&transcript_dir, session_id, &messages)
        .map_err(|e| e.to_string())?;

    let compacted = compactor::auto_compact(
        &api_format,
        &base_url,
        &api_key,
        &model_name,
        &messages,
        &transcript_path.to_string_lossy(),
    )
    .await
    .map_err(|e| e.to_string())?;

    chat_session_io::replace_messages_with_compacted_with_pool(pool, session_id, &compacted)
        .await?;

    let new_tokens = estimate_tokens(&compacted);
    let summary = compacted
        .iter()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
        .to_string();

    Ok(CompactionResult {
        original_tokens,
        new_tokens,
        summary,
    })
}
