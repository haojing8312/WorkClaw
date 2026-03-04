use super::permissions::PermissionMode;
use super::registry::ToolRegistry;
use super::system_prompts::SystemPromptBuilder;
use super::types::{LLMResponse, ToolContext, ToolResult};
use crate::adapters;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

/// 单次工具输出允许的最大字符数
const MAX_TOOL_OUTPUT_CHARS: usize = 30_000;

/// 截断过长的工具输出
///
/// 当输出超过 `max_chars` 字符时，保留前 `max_chars` 个字符并附加截断提示信息。
pub fn truncate_tool_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        return output.to_string();
    }
    let truncated: String = output.chars().take(max_chars).collect();
    format!(
        "{}\n\n[输出已截断，共 {} 字符，已显示前 {} 字符]",
        truncated,
        output.len(),
        max_chars
    )
}

const CHARS_PER_TOKEN: usize = 4;
const DEFAULT_TOKEN_BUDGET: usize = 100_000; // 约 400k 字符

/// 估算消息列表的 token 数（简单估算：字符数 / 4）
pub fn estimate_tokens(messages: &[Value]) -> usize {
    let total_chars: usize = messages
        .iter()
        .map(|m| {
            // 纯文本 content
            let text_len = m["content"].as_str().map_or(0, |s| s.len());
            // 数组型 content（如 tool_use / tool_result blocks）
            let array_len = m["content"].as_array().map_or(0, |arr| {
                arr.iter()
                    .map(|v| serde_json::to_string(v).map_or(0, |s| s.len()))
                    .sum()
            });
            text_len + array_len
        })
        .sum();
    total_chars / CHARS_PER_TOKEN
}

/// Layer 1 微压缩：替换旧的 tool_result 内容为占位符
///
/// 保留最近 `keep_recent` 条 tool_result 的完整内容，
/// 将更早的替换为 "[已执行]" 占位符。
/// 仅修改发送给 LLM 的副本，不影响原始数据。
///
/// 同时支持两种格式：
/// - Anthropic：`content` 数组中 `type == "tool_result"` 的条目
/// - OpenAI：`role == "tool"` 的消息
pub fn micro_compact(messages: &[Value], keep_recent: usize) -> Vec<Value> {
    // 找出所有包含 tool_result 的消息索引
    let tool_result_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            // Anthropic: content 是数组且包含 tool_result
            m["content"].as_array().map_or(false, |arr| {
                arr.iter().any(|v| v["type"].as_str() == Some("tool_result"))
            })
            // OpenAI: role == "tool"
            || m["role"].as_str() == Some("tool")
        })
        .map(|(i, _)| i)
        .collect();

    if tool_result_indices.len() <= keep_recent {
        return messages.to_vec();
    }

    let cutoff = tool_result_indices.len() - keep_recent;
    let old_indices: std::collections::HashSet<usize> =
        tool_result_indices[..cutoff].iter().copied().collect();

    messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            if old_indices.contains(&i) {
                if m["role"].as_str() == Some("tool") {
                    // OpenAI 格式
                    json!({
                        "role": "tool",
                        "tool_call_id": m["tool_call_id"],
                        "content": "[已执行]"
                    })
                } else {
                    // Anthropic 格式：替换 content 数组中的 tool_result 条目
                    let replaced = m["content"].as_array().map(|arr| {
                        arr.iter()
                            .map(|v| {
                                if v["type"].as_str() == Some("tool_result") {
                                    json!({
                                        "type": "tool_result",
                                        "tool_use_id": v["tool_use_id"],
                                        "content": "[已执行]"
                                    })
                                } else {
                                    v.clone()
                                }
                            })
                            .collect::<Vec<_>>()
                    });
                    match replaced {
                        Some(arr) => json!({"role": "user", "content": arr}),
                        None => m.clone(),
                    }
                }
            } else {
                m.clone()
            }
        })
        .collect()
}

/// 裁剪消息列表到 token 预算内
/// 保留第一条消息和最后的消息，从第二条开始裁剪中间的
pub fn trim_messages(messages: &[Value], token_budget: usize) -> Vec<Value> {
    if messages.len() <= 2 || estimate_tokens(messages) <= token_budget {
        return messages.to_vec();
    }

    let first = &messages[0];
    let last = &messages[messages.len() - 1];

    // 从后往前累加保留的消息
    let budget_chars = token_budget * CHARS_PER_TOKEN * 70 / 100;
    let first_chars = first["content"].as_str().map_or(0, |s| s.len());
    let last_chars = last["content"].as_str().map_or(0, |s| s.len());
    let mut char_count = first_chars + last_chars;

    let mut keep_from_end: Vec<&Value> = Vec::new();

    for msg in messages[1..messages.len() - 1].iter().rev() {
        let msg_chars = msg["content"].as_str().map_or(0, |s| s.len())
            + msg["content"].as_array().map_or(0, |arr| {
                arr.iter()
                    .map(|v| serde_json::to_string(v).map_or(0, |s| s.len()))
                    .sum()
            });
        if char_count + msg_chars > budget_chars {
            break;
        }
        char_count += msg_chars;
        keep_from_end.push(msg);
    }
    keep_from_end.reverse();

    let trimmed_count = messages.len() - 2 - keep_from_end.len();
    let mut result = vec![first.clone()];

    if trimmed_count > 0 {
        result.push(json!({
            "role": "user",
            "content": format!("[前 {} 条消息已省略]", trimmed_count)
        }));
    }

    for msg in keep_from_end {
        result.push(msg.clone());
    }
    result.push(last.clone());

    result
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ToolCallEvent {
    pub session_id: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub tool_output: Option<String>,
    pub status: String, // "started" | "completed" | "error"
}

/// Agent 状态事件，用于前端展示当前执行阶段
#[derive(serde::Serialize, Clone, Debug)]
pub struct AgentStateEvent {
    pub session_id: String,
    /// 状态类型: "thinking" | "tool_calling" | "finished" | "error"
    pub state: String,
    /// 工具名列表（tool_calling 时）或错误信息（error 时）
    pub detail: Option<String>,
    pub iteration: usize,
}

pub struct AgentExecutor {
    registry: Arc<ToolRegistry>,
    max_iterations: usize,
    system_prompt_builder: SystemPromptBuilder,
}

pub fn build_skill_route_event(
    session_id: &str,
    route_run_id: &str,
    node_id: &str,
    parent_node_id: Option<String>,
    skill_name: &str,
    depth: usize,
    status: &str,
    duration_ms: Option<u64>,
    error_code: Option<&str>,
    error_message: Option<&str>,
) -> Value {
    json!({
        "session_id": session_id,
        "route_run_id": route_run_id,
        "node_id": node_id,
        "parent_node_id": parent_node_id,
        "skill_name": skill_name,
        "depth": depth,
        "status": status,
        "duration_ms": duration_ms,
        "error_code": error_code,
        "error_message": error_message,
    })
}

pub fn split_error_code_and_message(text: &str) -> (String, String) {
    if let Some((code, msg)) = text.split_once(':') {
        let code = code.trim();
        if !code.is_empty()
            && code
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            return (code.to_string(), msg.trim().to_string());
        }
    }
    ("SKILL_EXECUTION_ERROR".to_string(), text.to_string())
}

impl AgentExecutor {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            max_iterations: 50,
            system_prompt_builder: SystemPromptBuilder::default(),
        }
    }

    pub fn with_max_iterations(registry: Arc<ToolRegistry>, max_iterations: usize) -> Self {
        Self {
            registry,
            max_iterations,
            system_prompt_builder: SystemPromptBuilder::default(),
        }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub fn registry_arc(&self) -> Arc<ToolRegistry> {
        Arc::clone(&self.registry)
    }

    /// 轮询 cancel_flag，直到收到取消信号
    async fn wait_for_cancel(cancel_flag: &Option<Arc<AtomicBool>>) {
        loop {
            if let Some(ref flag) = cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    return;
                }
            } else {
                // 没有 cancel_flag，永远不会取消
                std::future::pending::<()>().await;
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    pub async fn execute_turn(
        &self,
        api_format: &str,
        base_url: &str,
        api_key: &str,
        model: &str,
        skill_system_prompt: &str,
        mut messages: Vec<Value>,
        on_token: impl Fn(String) + Send + Clone,
        app_handle: Option<&AppHandle>,
        session_id: Option<&str>,
        allowed_tools: Option<&[String]>,
        permission_mode: PermissionMode,
        tool_confirm_tx: Option<
            std::sync::Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<bool>>>>,
        >,
        work_dir: Option<String>,
        max_iterations_override: Option<usize>,
        cancel_flag: Option<Arc<AtomicBool>>,
        route_node_timeout_secs: Option<u64>,
        route_retry_count: Option<usize>,
    ) -> Result<Vec<Value>> {
        // 组合系统级 prompt 和 Skill prompt
        let system_prompt = self.system_prompt_builder.build(skill_system_prompt);

        let tool_ctx = ToolContext {
            work_dir: work_dir.map(PathBuf::from),
            allowed_tools: allowed_tools.map(|tools| tools.to_vec()),
        };
        let max_iterations = max_iterations_override.unwrap_or(self.max_iterations);
        let route_node_timeout_secs = route_node_timeout_secs.unwrap_or(60).clamp(5, 600);
        let route_retry_count = route_retry_count.unwrap_or(0).clamp(0, 2);
        let mut iteration = 0;
        let route_run_id = Uuid::new_v4().to_string();

        loop {
            // 检查取消标志
            if let Some(ref flag) = cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    eprintln!("[agent] 任务被用户取消");
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let _ = app.emit(
                            "agent-state-event",
                            AgentStateEvent {
                                session_id: sid.to_string(),
                                state: "finished".to_string(),
                                detail: Some("用户取消".to_string()),
                                iteration,
                            },
                        );
                    }
                    messages.push(json!({
                        "role": "assistant",
                        "content": "任务已被取消。"
                    }));
                    return Ok(messages);
                }
            }

            if iteration >= max_iterations {
                // 发射 error 状态事件
                if let (Some(app), Some(sid)) = (app_handle, session_id) {
                    let _ = app.emit(
                        "agent-state-event",
                        AgentStateEvent {
                            session_id: sid.to_string(),
                            state: "error".to_string(),
                            detail: Some(format!("达到最大迭代次数 {}", max_iterations)),
                            iteration,
                        },
                    );
                }
                return Err(anyhow!("达到最大迭代次数 {}", max_iterations));
            }
            iteration += 1;

            eprintln!("[agent] Iteration {}/{}", iteration, max_iterations);

            // 发射 thinking 状态事件
            if let (Some(app), Some(sid)) = (app_handle, session_id) {
                let _ = app.emit(
                    "agent-state-event",
                    AgentStateEvent {
                        session_id: sid.to_string(),
                        state: "thinking".to_string(),
                        detail: None,
                        iteration,
                    },
                );
            }

            // 自动压缩检查（仅在第二轮及之后，避免首轮触发）
            if iteration > 1 {
                let tokens = estimate_tokens(&messages);
                if super::compactor::needs_auto_compact(tokens) {
                    eprintln!("[agent] Token 数 {} 超过阈值，触发自动压缩", tokens);
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let transcript_dir = app
                            .path()
                            .app_data_dir()
                            .unwrap_or_default()
                            .join("transcripts");
                        if let Ok(path) =
                            super::compactor::save_transcript(&transcript_dir, sid, &messages)
                        {
                            let path_str = path.to_string_lossy().to_string();
                            match super::compactor::auto_compact(
                                api_format, base_url, api_key, model, &messages, &path_str,
                            )
                            .await
                            {
                                Ok(compacted) => {
                                    eprintln!(
                                        "[agent] 自动压缩完成，消息数 {} → {}",
                                        messages.len(),
                                        compacted.len()
                                    );
                                    messages = compacted;
                                }
                                Err(e) => eprintln!("[agent] 自动压缩失败: {}", e),
                            }
                        }
                    }
                }
            }

            // 根据白名单过滤工具定义
            let tools = match allowed_tools {
                Some(whitelist) => self.registry.get_filtered_tool_definitions(whitelist),
                None => self.registry.get_tool_definitions(),
            };

            // 上下文压缩：Layer 1 微压缩 + token 预算裁剪
            let compacted = micro_compact(&messages, 3);
            let trimmed = trim_messages(&compacted, DEFAULT_TOKEN_BUDGET);

            // 调用 LLM（使用组合后的系统 prompt）
            let response = if api_format == "anthropic" {
                adapters::anthropic::chat_stream_with_tools(
                    base_url,
                    api_key,
                    model,
                    &system_prompt,
                    trimmed.clone(),
                    tools,
                    on_token.clone(),
                )
                .await?
            } else {
                // OpenAI 兼容格式
                adapters::openai::chat_stream_with_tools(
                    base_url,
                    api_key,
                    model,
                    &system_prompt,
                    trimmed.clone(),
                    tools,
                    on_token.clone(),
                )
                .await?
            };

            // 处理响应
            match response {
                LLMResponse::Text(content) => {
                    // 纯文本响应 - 结束循环
                    messages.push(json!({
                        "role": "assistant",
                        "content": content
                    }));
                    eprintln!("[agent] Finished with text response");

                    // 发射 finished 状态事件
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let _ = app.emit(
                            "agent-state-event",
                            AgentStateEvent {
                                session_id: sid.to_string(),
                                state: "finished".to_string(),
                                detail: None,
                                iteration,
                            },
                        );
                    }

                    return Ok(messages);
                }
                tc_response
                @ (LLMResponse::ToolCalls(_) | LLMResponse::TextWithToolCalls(_, _)) => {
                    let (companion_text, tool_calls) = match tc_response {
                        LLMResponse::ToolCalls(tc) => (String::new(), tc),
                        LLMResponse::TextWithToolCalls(text, tc) => (text, tc),
                        _ => unreachable!(),
                    };

                    eprintln!(
                        "[agent] Executing {} tool calls (companion_text={})",
                        tool_calls.len(),
                        !companion_text.is_empty()
                    );

                    // 发射 tool_calling 状态事件
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let tool_names: Vec<&str> =
                            tool_calls.iter().map(|tc| tc.name.as_str()).collect();
                        let _ = app.emit(
                            "agent-state-event",
                            AgentStateEvent {
                                session_id: sid.to_string(),
                                state: "tool_calling".to_string(),
                                detail: Some(tool_names.join(", ")),
                                iteration,
                            },
                        );
                    }

                    // 执行所有工具调用
                    let mut tool_results = vec![];
                    for (call_index, call) in tool_calls.iter().enumerate() {
                        let skill_name = call
                            .input
                            .get("skill_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let is_skill_call = call.name == "skill";
                        let node_id = format!("{}-{}-{}", iteration, call_index, call.id);
                        let started_at = std::time::Instant::now();

                        if is_skill_call {
                            if let (Some(app), Some(sid)) = (app_handle, session_id) {
                                let _ = app.emit(
                                    "skill-route-node-updated",
                                    build_skill_route_event(
                                        sid,
                                        &route_run_id,
                                        &node_id,
                                        None,
                                        &skill_name,
                                        1,
                                        "routing",
                                        None,
                                        None,
                                        None,
                                    ),
                                );
                            }
                        }
                        // 执行每个工具前检查取消标志
                        if let Some(ref flag) = cancel_flag {
                            if flag.load(Ordering::SeqCst) {
                                eprintln!("[agent] 工具执行中被用户取消");
                                // 发射 finished 事件，确保前端清除状态指示器
                                if let (Some(app), Some(sid)) = (app_handle, session_id) {
                                    let _ = app.emit(
                                        "agent-state-event",
                                        AgentStateEvent {
                                            session_id: sid.to_string(),
                                            state: "finished".to_string(),
                                            detail: Some("用户取消".to_string()),
                                            iteration,
                                        },
                                    );
                                }
                                messages.push(json!({
                                    "role": "assistant",
                                    "content": "任务已被取消。"
                                }));
                                return Ok(messages);
                            }
                        }

                        eprintln!("[agent] Calling tool: {}", call.name);

                        if is_skill_call {
                            if let (Some(app), Some(sid)) = (app_handle, session_id) {
                                let _ = app.emit(
                                    "skill-route-node-updated",
                                    build_skill_route_event(
                                        sid,
                                        &route_run_id,
                                        &node_id,
                                        None,
                                        &skill_name,
                                        1,
                                        "executing",
                                        None,
                                        None,
                                        None,
                                    ),
                                );
                            }
                        }

                        // 发送工具开始事件
                        if let (Some(app), Some(sid)) = (app_handle, session_id) {
                            let _ = app.emit(
                                "tool-call-event",
                                ToolCallEvent {
                                    session_id: sid.to_string(),
                                    tool_name: call.name.clone(),
                                    tool_input: call.input.clone(),
                                    tool_output: None,
                                    status: "started".to_string(),
                                },
                            );
                        }

                        // 权限确认检查：在执行工具前判断是否需要用户确认
                        if permission_mode.needs_confirmation(&call.name) {
                            if let (Some(app), Some(sid)) = (app_handle, session_id) {
                                // 发射确认请求事件，前端弹出确认对话框
                                let _ = app.emit(
                                    "tool-confirm-event",
                                    serde_json::json!({
                                        "session_id": sid,
                                        "tool_name": call.name,
                                        "tool_input": call.input,
                                    }),
                                );

                                // 创建一次性通道并将发送端存入全局状态
                                let (tx, rx) = std::sync::mpsc::channel::<bool>();
                                if let Some(ref confirm_state) = tool_confirm_tx {
                                    if let Ok(mut guard) = confirm_state.lock() {
                                        *guard = Some(tx);
                                    }
                                }

                                // 阻塞等待用户确认（最多 300 秒），超时视为拒绝
                                let confirmed = rx
                                    .recv_timeout(std::time::Duration::from_secs(300))
                                    .unwrap_or(false);

                                // 清理发送端，避免下次误用
                                if let Some(ref confirm_state) = tool_confirm_tx {
                                    if let Ok(mut guard) = confirm_state.lock() {
                                        *guard = None;
                                    }
                                }

                                if !confirmed {
                                    // 用户拒绝 — 记录拒绝事件并跳过此工具
                                    let _ = app.emit(
                                        "tool-call-event",
                                        ToolCallEvent {
                                            session_id: sid.to_string(),
                                            tool_name: call.name.clone(),
                                            tool_input: call.input.clone(),
                                            tool_output: Some("用户拒绝了此操作".to_string()),
                                            status: "error".to_string(),
                                        },
                                    );
                                    tool_results.push(ToolResult {
                                        tool_use_id: call.id.clone(),
                                        content: "用户拒绝了此操作".to_string(),
                                    });
                                    continue;
                                }
                            }
                        }

                        let max_attempts = if is_skill_call {
                            route_retry_count + 1
                        } else {
                            1
                        };
                        let mut attempt = 0usize;
                        let (result, is_error) = loop {
                            attempt += 1;
                            let (result, is_error) = match self.registry.get(&call.name) {
                                Some(tool) => {
                                    // 检查白名单：若设置了白名单但工具不在其中，拒绝执行
                                    if let Some(whitelist) = allowed_tools {
                                        if !whitelist.iter().any(|w| w == &call.name) {
                                            (
                                                format!("此 Skill 不允许使用工具: {}", call.name),
                                                true,
                                            )
                                        } else {
                                            let tool_clone = Arc::clone(&tool);
                                            let input_clone = call.input.clone();
                                            let ctx_clone = tool_ctx.clone();
                                            let handle = tokio::task::spawn_blocking(move || {
                                                tool_clone.execute(input_clone, &ctx_clone)
                                            });
                                            let cancel_flag_ref = cancel_flag.clone();
                                            let exec_future = async move {
                                                tokio::select! {
                                                    res = handle => {
                                                        match res {
                                                            Ok(Ok(output)) => (output, false),
                                                            Ok(Err(e)) => (format!("工具执行错误: {}", e), true),
                                                            Err(e) => (format!("工具执行线程异常: {}", e), true),
                                                        }
                                                    }
                                                    _ = Self::wait_for_cancel(&cancel_flag_ref) => {
                                                        ("工具执行被用户取消".to_string(), true)
                                                    }
                                                }
                                            };
                                            if is_skill_call {
                                                match tokio::time::timeout(
                                                    std::time::Duration::from_secs(
                                                        route_node_timeout_secs,
                                                    ),
                                                    exec_future,
                                                )
                                                .await
                                                {
                                                    Ok(v) => v,
                                                    Err(_) => (
                                                        "TIMEOUT: 子 Skill 执行超时".to_string(),
                                                        true,
                                                    ),
                                                }
                                            } else {
                                                exec_future.await
                                            }
                                        }
                                    } else {
                                        let tool_clone = Arc::clone(&tool);
                                        let input_clone = call.input.clone();
                                        let ctx_clone = tool_ctx.clone();
                                        let handle = tokio::task::spawn_blocking(move || {
                                            tool_clone.execute(input_clone, &ctx_clone)
                                        });
                                        let cancel_flag_ref = cancel_flag.clone();
                                        let exec_future = async move {
                                            tokio::select! {
                                                res = handle => {
                                                    match res {
                                                        Ok(Ok(output)) => (output, false),
                                                        Ok(Err(e)) => (format!("工具执行错误: {}", e), true),
                                                        Err(e) => (format!("工具执行线程异常: {}", e), true),
                                                    }
                                                }
                                                _ = Self::wait_for_cancel(&cancel_flag_ref) => {
                                                    ("工具执行被用户取消".to_string(), true)
                                                }
                                            }
                                        };
                                        if is_skill_call {
                                            match tokio::time::timeout(
                                                std::time::Duration::from_secs(
                                                    route_node_timeout_secs,
                                                ),
                                                exec_future,
                                            )
                                            .await
                                            {
                                                Ok(v) => v,
                                                Err(_) => {
                                                    ("TIMEOUT: 子 Skill 执行超时".to_string(), true)
                                                }
                                            }
                                        } else {
                                            exec_future.await
                                        }
                                    }
                                }
                                None => {
                                    // 列出可用工具，引导 LLM 使用正确的工具
                                    let available: Vec<String> = self
                                        .registry
                                        .get_tool_definitions()
                                        .iter()
                                        .filter_map(|t| t["name"].as_str().map(String::from))
                                        .collect();
                                    (
                                        format!(
                                            "错误: 工具 '{}' 不存在。请勿再次调用此工具。可用工具: {}",
                                            call.name,
                                            available.join(", ")
                                        ),
                                        true,
                                    )
                                }
                            };
                            if !is_error || attempt >= max_attempts {
                                break (result, is_error);
                            }
                        };
                        // 截断过长的工具输出，防止超出上下文窗口
                        let result = truncate_tool_output(&result, MAX_TOOL_OUTPUT_CHARS);

                        // 发送工具完成事件
                        if let (Some(app), Some(sid)) = (app_handle, session_id) {
                            let _ = app.emit(
                                "tool-call-event",
                                ToolCallEvent {
                                    session_id: sid.to_string(),
                                    tool_name: call.name.clone(),
                                    tool_input: call.input.clone(),
                                    tool_output: Some(result.clone()),
                                    status: if is_error {
                                        "error".to_string()
                                    } else {
                                        "completed".to_string()
                                    },
                                },
                            );
                        }

                        if is_skill_call {
                            if let (Some(app), Some(sid)) = (app_handle, session_id) {
                                let duration_ms = started_at.elapsed().as_millis() as u64;
                                let parsed_error = if is_error {
                                    Some(split_error_code_and_message(&result))
                                } else {
                                    None
                                };
                                let _ = app.emit(
                                    "skill-route-node-updated",
                                    build_skill_route_event(
                                        sid,
                                        &route_run_id,
                                        &node_id,
                                        None,
                                        &skill_name,
                                        1,
                                        if is_error { "failed" } else { "completed" },
                                        Some(duration_ms),
                                        parsed_error.as_ref().map(|(code, _)| code.as_str()),
                                        parsed_error.as_ref().map(|(_, msg)| msg.as_str()),
                                    ),
                                );
                            }
                        }

                        tool_results.push(ToolResult {
                            tool_use_id: call.id.clone(),
                            content: result,
                        });
                    }

                    // 添加工具调用和结果到消息历史（包含伴随文本）
                    if api_format == "anthropic" {
                        // Anthropic 格式: assistant 消息包含 text block + tool_use blocks
                        let mut content_blocks: Vec<Value> = vec![];
                        if !companion_text.is_empty() {
                            content_blocks.push(json!({"type": "text", "text": companion_text}));
                        }
                        for tc in &tool_calls {
                            content_blocks.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.input,
                            }));
                        }
                        messages.push(json!({
                            "role": "assistant",
                            "content": content_blocks
                        }));

                        // user 消息包含 tool_result blocks
                        messages.push(json!({
                            "role": "user",
                            "content": tool_results.iter().map(|tr| json!({
                                "type": "tool_result",
                                "tool_use_id": tr.tool_use_id,
                                "content": tr.content,
                            })).collect::<Vec<_>>()
                        }));
                    } else {
                        // OpenAI 格式: companion_text 放 content 字段
                        let content_val = if companion_text.is_empty() {
                            Value::Null
                        } else {
                            Value::String(companion_text.clone())
                        };
                        messages.push(json!({
                            "role": "assistant",
                            "content": content_val,
                            "tool_calls": tool_calls.iter().map(|tc| json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": serde_json::to_string(&tc.input).unwrap_or_default(),
                                }
                            })).collect::<Vec<_>>()
                        }));
                        // OpenAI: 每个工具结果是独立的 "tool" 角色消息
                        for tr in &tool_results {
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tr.tool_use_id,
                                "content": tr.content,
                            }));
                        }
                    }

                    // 继续下一轮迭代
                    continue;
                }
            }
        }
    }
}
