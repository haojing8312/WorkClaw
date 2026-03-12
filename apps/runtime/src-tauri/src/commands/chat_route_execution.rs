use super::chat::{ModelRouteErrorKind, StreamToken, ToolConfirmResponder};
use super::chat_runtime_io as chat_io;
use crate::agent::permissions::PermissionMode;
use crate::agent::AgentExecutor;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub(crate) struct RouteExecutionOutcome {
    pub final_messages: Option<Vec<Value>>,
    pub last_error: Option<String>,
    pub last_error_kind: Option<String>,
    pub partial_text: String,
}

pub(crate) struct RouteExecutionParams<'a> {
    pub app: &'a AppHandle,
    pub agent_executor: &'a AgentExecutor,
    pub db: &'a sqlx::SqlitePool,
    pub session_id: &'a str,
    pub requested_capability: &'a str,
    pub route_candidates: &'a [(String, String, String, String)],
    pub per_candidate_retry_count: usize,
    pub system_prompt: &'a str,
    pub messages: &'a [Value],
    pub allowed_tools: Option<&'a [String]>,
    pub permission_mode: PermissionMode,
    pub tool_confirm_responder: ToolConfirmResponder,
    pub executor_work_dir: Option<String>,
    pub max_iterations: Option<usize>,
    pub cancel_flag: Arc<AtomicBool>,
    pub node_timeout_seconds: u64,
    pub route_retry_count: usize,
    pub classify_error: fn(&str) -> ModelRouteErrorKind,
    pub error_kind_key: fn(ModelRouteErrorKind) -> &'static str,
    pub should_retry_same_candidate: fn(ModelRouteErrorKind) -> bool,
    pub retry_budget_for_error: fn(ModelRouteErrorKind, usize) -> usize,
    pub retry_backoff_ms: fn(ModelRouteErrorKind, usize) -> u64,
}

pub(crate) async fn execute_route_candidates(
    params: RouteExecutionParams<'_>,
) -> RouteExecutionOutcome {
    let mut final_messages_opt: Option<Vec<Value>> = None;
    let mut last_error: Option<String> = None;
    let mut last_error_kind: Option<String> = None;
    let streamed_text = Arc::new(std::sync::Mutex::new(String::new()));

    for (candidate_api_format, candidate_base_url, candidate_model_name, candidate_api_key) in
        params.route_candidates
    {
        let mut attempt_idx = 0usize;
        loop {
            if let Ok(mut buffer) = streamed_text.lock() {
                buffer.clear();
            }
            let app_clone = params.app.clone();
            let session_id_clone = params.session_id.to_string();
            let streamed_text_clone = Arc::clone(&streamed_text);
            let attempt = params
                .agent_executor
                .execute_turn(
                    candidate_api_format,
                    candidate_base_url,
                    candidate_api_key,
                    candidate_model_name,
                    params.system_prompt,
                    params.messages.to_vec(),
                    move |token: String| {
                        if let Ok(mut buffer) = streamed_text_clone.lock() {
                            buffer.push_str(&token);
                        }
                        let _ = app_clone.emit(
                            "stream-token",
                            StreamToken {
                                session_id: session_id_clone.clone(),
                                token,
                                done: false,
                                sub_agent: false,
                            },
                        );
                    },
                    Some(params.app),
                    Some(params.session_id),
                    params.allowed_tools,
                    params.permission_mode,
                    Some(params.tool_confirm_responder.clone()),
                    params.executor_work_dir.clone(),
                    params.max_iterations,
                    Some(params.cancel_flag.clone()),
                    Some(params.node_timeout_seconds),
                    Some(params.route_retry_count),
                )
                .await;

            match attempt {
                Ok(messages_out) => {
                    chat_io::record_route_attempt_log_with_pool(
                        params.db,
                        params.session_id,
                        params.requested_capability,
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        attempt_idx,
                        "ok",
                        true,
                        "",
                    )
                    .await;
                    final_messages_opt = Some(messages_out);
                    break;
                }
                Err(err) => {
                    let err_text = err.to_string();
                    let kind = (params.classify_error)(&err_text);
                    let kind_text = (params.error_kind_key)(kind);
                    chat_io::record_route_attempt_log_with_pool(
                        params.db,
                        params.session_id,
                        params.requested_capability,
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        attempt_idx,
                        kind_text,
                        false,
                        &err_text,
                    )
                    .await;
                    last_error = Some(err_text.clone());
                    last_error_kind = Some(kind_text.to_string());
                    eprintln!(
                        "[routing] 候选模型执行失败: format={}, model={}, attempt={}, kind={:?}, err={}",
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        kind,
                        err_text
                    );

                    let retry_budget =
                        (params.retry_budget_for_error)(kind, params.per_candidate_retry_count);
                    if (params.should_retry_same_candidate)(kind) && attempt_idx < retry_budget {
                        let backoff_ms = (params.retry_backoff_ms)(kind, attempt_idx);
                        if backoff_ms > 0 {
                            eprintln!(
                                "[routing] 同候选重试等待: format={}, model={}, wait_ms={}, next_attempt={}",
                                candidate_api_format,
                                candidate_model_name,
                                backoff_ms,
                                attempt_idx + 2
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        }
                        attempt_idx += 1;
                        continue;
                    }
                    break;
                }
            }
        }
        if final_messages_opt.is_some() {
            break;
        }
    }

    let partial_text = streamed_text
        .lock()
        .map(|buffer| buffer.clone())
        .unwrap_or_default();

    RouteExecutionOutcome {
        final_messages: final_messages_opt,
        last_error,
        last_error_kind,
        partial_text,
    }
}
