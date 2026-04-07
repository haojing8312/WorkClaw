use super::chat_session_io;
use crate::agent::runtime::compaction_pipeline::{run_compaction, RuntimeCompactionRequest};
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
    transcript_root: &Path,
) -> Result<CompactionResult, String> {
    let (messages, api_format, base_url, api_key, model_name) =
        chat_session_io::load_compaction_inputs_with_pool(pool, session_id).await?;
    let outcome = run_compaction(RuntimeCompactionRequest {
        api_format: &api_format,
        base_url: &base_url,
        api_key: &api_key,
        model: &model_name,
        session_id,
        messages: &messages,
        transcript_root,
        observability: None,
    })
    .await
    .map_err(|e| e.to_string())?;

    chat_session_io::replace_messages_with_compacted_with_pool(
        pool,
        session_id,
        &outcome.compacted_messages,
    )
    .await?;

    Ok(CompactionResult {
        original_tokens: outcome.original_tokens,
        new_tokens: outcome.new_tokens,
        summary: outcome.summary,
    })
}
