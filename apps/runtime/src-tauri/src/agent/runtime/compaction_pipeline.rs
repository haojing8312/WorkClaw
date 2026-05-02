use super::observability::RuntimeObservability;
use crate::agent::compactor;
use anyhow::Result;
use runtime_executor_core::estimate_tokens;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) struct RuntimeCompactionRequest<'a> {
    pub api_format: &'a str,
    pub base_url: &'a str,
    pub api_key: &'a str,
    pub model: &'a str,
    pub session_id: &'a str,
    pub messages: &'a [Value],
    pub transcript_root: &'a Path,
    pub observability: Option<&'a RuntimeObservability>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeCompactionOutcome {
    pub compacted_messages: Vec<Value>,
    pub transcript_path: PathBuf,
    pub original_tokens: usize,
    pub new_tokens: usize,
    pub summary: String,
}

pub(crate) async fn maybe_auto_compact(
    request: RuntimeCompactionRequest<'_>,
) -> Result<Option<RuntimeCompactionOutcome>> {
    if !compactor::needs_auto_compact(estimate_tokens(request.messages)) {
        return Ok(None);
    }

    run_compaction(request).await.map(Some)
}

pub(crate) async fn run_compaction(
    request: RuntimeCompactionRequest<'_>,
) -> Result<RuntimeCompactionOutcome> {
    let original_tokens = estimate_tokens(request.messages);
    let transcript_path = compactor::save_transcript(
        &request.transcript_root.to_path_buf(),
        request.session_id,
        request.messages,
    )?;
    let compacted_messages = compactor::auto_compact(
        request.api_format,
        request.base_url,
        request.api_key,
        request.model,
        request.messages,
        &transcript_path.to_string_lossy(),
    )
    .await?;
    let new_tokens = estimate_tokens(&compacted_messages);
    let summary = extract_compaction_summary(&compacted_messages);
    if let Some(observability) = request.observability {
        observability.record_compaction_run();
    }

    Ok(RuntimeCompactionOutcome {
        compacted_messages,
        transcript_path,
        original_tokens,
        new_tokens,
        summary,
    })
}

fn extract_compaction_summary(compacted_messages: &[Value]) -> String {
    compactor::extract_compaction_display_summary(compacted_messages)
}

#[cfg(test)]
mod tests {
    use super::{maybe_auto_compact, run_compaction, RuntimeCompactionRequest};
    use crate::agent::runtime::RuntimeObservability;
    use serde_json::json;

    #[tokio::test]
    async fn maybe_auto_compact_skips_short_histories() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let messages = vec![json!({"role": "user", "content": "hello"})];
        let outcome = maybe_auto_compact(RuntimeCompactionRequest {
            api_format: "openai",
            base_url: "http://mock",
            api_key: "mock-key",
            model: "mock-model",
            session_id: "session-short",
            messages: &messages,
            transcript_root: temp_dir.path(),
            observability: None,
        })
        .await
        .expect("maybe compact");

        assert!(outcome.is_none());
    }

    #[tokio::test]
    async fn run_compaction_updates_observability_snapshot() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let messages = vec![
            json!({"role": "user", "content": "summarize progress"}),
            json!({"role": "assistant", "content": "half complete"}),
        ];
        let observability = RuntimeObservability::new(8);

        let _outcome = run_compaction(RuntimeCompactionRequest {
            api_format: "openai",
            base_url: "http://mock",
            api_key: "mock-key",
            model: "mock-model",
            session_id: "session-compact",
            messages: &messages,
            transcript_root: temp_dir.path(),
            observability: Some(&observability),
        })
        .await
        .expect("run compaction");

        assert_eq!(observability.snapshot().compaction.runs, 1);
    }

    #[tokio::test]
    async fn run_compaction_creates_transcript_and_summary_with_mock_provider() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let messages = vec![
            json!({"role": "user", "content": "请总结当前进展"}),
            json!({"role": "assistant", "content": "已经完成一半"}),
        ];
        let outcome = run_compaction(RuntimeCompactionRequest {
            api_format: "openai",
            base_url: "http://mock",
            api_key: "mock-key",
            model: "mock-model",
            session_id: "session-compact",
            messages: &messages,
            transcript_root: temp_dir.path(),
            observability: None,
        })
        .await
        .expect("run compaction");

        assert!(outcome.transcript_path.exists());
        assert_eq!(outcome.compacted_messages.len(), 2);
        assert!(outcome.summary.contains("MOCK_RESPONSE"));
        assert!(outcome.original_tokens > 0);
        assert!(outcome.new_tokens > 0);
    }
}
