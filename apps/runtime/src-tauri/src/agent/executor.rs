use super::browser_progress::BrowserProgressSnapshot;
use super::permissions::PermissionMode;
use super::registry::ToolRegistry;
use super::run_guard::{
    encode_run_stop_reason, ProgressFingerprint, ProgressGuard, RunBudgetPolicy, RunBudgetScope,
    RunGuardWarning, RunStopReason,
};
use super::system_prompts::SystemPromptBuilder;
use super::types::{LLMResponse, StreamDelta, ToolContext, ToolResult};
use crate::adapters;
use crate::approval_bus::{
    approval_bus_rollout_enabled_with_pool, ApprovalDecision, ApprovalManager,
    CreateApprovalRequest,
};
use crate::approval_rules::find_matching_approval_rule_with_pool;
use crate::commands::chat::{ApprovalManagerState, PendingApprovalBridgeState};
use crate::commands::feishu_gateway::notify_feishu_approval_requested_with_pool;
use crate::commands::session_runs::append_session_run_event_with_pool;
use crate::commands::skills::DbState;
use crate::session_journal::{SessionJournalStateHandle, SessionJournalStore, SessionRunEvent};
use anyhow::{anyhow, Result};
use runtime_executor_core::{
    estimate_tokens, extract_tool_call_parse_error, micro_compact, split_error_code_and_message,
    trim_messages, truncate_tool_output, update_tool_failure_streak, ToolFailureStreak,
    DEFAULT_TOKEN_BUDGET, MAX_TOOL_OUTPUT_CHARS,
};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

const TOOL_CONFIRM_TIMEOUT_SECS: u64 = 15;

#[derive(serde::Serialize, Clone, Debug)]
pub struct ToolCallEvent {
    pub session_id: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub tool_output: Option<String>,
    pub status: String, // "started" | "completed" | "error"
}

#[derive(Debug, PartialEq, Eq)]
enum ToolConfirmationDecision {
    Confirmed,
    Rejected,
    TimedOut,
}

fn wait_for_tool_confirmation(
    rx: &std::sync::mpsc::Receiver<bool>,
    timeout: std::time::Duration,
) -> ToolConfirmationDecision {
    match rx.recv_timeout(timeout) {
        Ok(true) => ToolConfirmationDecision::Confirmed,
        Ok(false) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            ToolConfirmationDecision::Rejected
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => ToolConfirmationDecision::TimedOut,
    }
}

async fn resolve_current_session_run_id(app: &AppHandle, session_id: &str) -> Option<String> {
    let journal_state = app.try_state::<SessionJournalStateHandle>()?;
    journal_state
        .0
        .read_state(session_id)
        .await
        .ok()?
        .current_run_id
}

async fn append_tool_run_event(
    app: &AppHandle,
    session_id: &str,
    event: SessionRunEvent,
) -> Result<()> {
    let db_state = app
        .try_state::<DbState>()
        .ok_or_else(|| anyhow!("DbState unavailable"))?;
    let journal_state = app
        .try_state::<SessionJournalStateHandle>()
        .ok_or_else(|| anyhow!("SessionJournalStateHandle unavailable"))?;

    append_session_run_event_with_pool(&db_state.0, journal_state.0.as_ref(), session_id, event)
        .await
        .map_err(|err| anyhow!(err))
}

async fn append_run_guard_warning_event(
    app: &AppHandle,
    session_id: &str,
    warning: &RunGuardWarning,
) -> Result<()> {
    let Some(run_id) = resolve_current_session_run_id(app, session_id).await else {
        return Ok(());
    };

    append_tool_run_event(
        app,
        session_id,
        SessionRunEvent::RunGuardWarning {
            run_id,
            warning_kind: warning.kind.as_key().to_string(),
            title: warning.title.clone(),
            message: warning.message.clone(),
            detail: warning.detail.clone(),
            last_completed_step: warning.last_completed_step.clone(),
        },
    )
    .await
}

#[derive(Clone)]
struct ApprovalWaitRuntime {
    pool: sqlx::SqlitePool,
    journal: Arc<SessionJournalStore>,
    approval_manager: Arc<ApprovalManager>,
    pending_bridge: Arc<std::sync::Mutex<Option<String>>>,
}

fn resolve_approval_wait_runtime(app: &AppHandle) -> Result<ApprovalWaitRuntime> {
    let db_state = app
        .try_state::<DbState>()
        .ok_or_else(|| anyhow!("DbState unavailable"))?;
    let journal_state = app
        .try_state::<SessionJournalStateHandle>()
        .ok_or_else(|| anyhow!("SessionJournalStateHandle unavailable"))?;
    let approval_state = app
        .try_state::<ApprovalManagerState>()
        .ok_or_else(|| anyhow!("ApprovalManagerState unavailable"))?;
    let pending_bridge = app
        .try_state::<PendingApprovalBridgeState>()
        .ok_or_else(|| anyhow!("PendingApprovalBridgeState unavailable"))?;

    Ok(ApprovalWaitRuntime {
        pool: db_state.0.clone(),
        journal: journal_state.0.clone(),
        approval_manager: approval_state.0.clone(),
        pending_bridge: pending_bridge.0.clone(),
    })
}

async fn request_tool_approval_and_wait(
    runtime: &ApprovalWaitRuntime,
    app_handle: Option<&AppHandle>,
    session_id: &str,
    run_id: Option<&str>,
    tool_name: &str,
    call_id: &str,
    input: &Value,
    work_dir: Option<&Path>,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> Result<ApprovalDecision> {
    let approval_id = Uuid::new_v4().to_string();
    let receiver = runtime
        .approval_manager
        .register_waiter(approval_id.clone());
    let (title, summary, impact, irreversible) =
        critical_action_summary(tool_name, input, work_dir);

    let record = runtime
        .approval_manager
        .create_pending_with_pool(
            &runtime.pool,
            Some(runtime.journal.as_ref()),
            CreateApprovalRequest {
                approval_id: approval_id.clone(),
                session_id: session_id.to_string(),
                run_id: run_id.map(str::to_string),
                call_id: call_id.to_string(),
                tool_name: tool_name.to_string(),
                input: input.clone(),
                summary: summary.clone(),
                impact: Some(impact.clone()),
                irreversible,
                work_dir: work_dir.map(|dir| dir.to_string_lossy().to_string()),
            },
        )
        .await
        .map_err(|err| anyhow!(err))?;

    if let Ok(mut guard) = runtime.pending_bridge.lock() {
        *guard = Some(approval_id.clone());
    }

    if let Some(app) = app_handle {
        let _ = app.emit(
            "approval-created",
            serde_json::json!({
                "approval_id": record.approval_id,
                "session_id": record.session_id,
                "run_id": record.run_id,
                "call_id": record.call_id,
                "tool_name": record.tool_name,
                "tool_input": record.input,
                "title": title,
                "summary": record.summary,
                "impact": record.impact,
                "irreversible": record.irreversible,
                "status": record.status,
            }),
        );
        let _ = app.emit(
            "tool-confirm-event",
            serde_json::json!({
                "approval_id": approval_id,
                "session_id": session_id,
                "tool_name": tool_name,
                "tool_input": input,
                "title": title,
                "summary": summary,
                "impact": impact,
                "irreversible": irreversible,
            }),
        );
    }

    let _ =
        notify_feishu_approval_requested_with_pool(&runtime.pool, session_id, &record, None).await;

    let resolution = runtime
        .approval_manager
        .wait_for_resolution(receiver, cancel_flag)
        .await
        .map_err(|err| anyhow!(err))?;

    if let Ok(mut guard) = runtime.pending_bridge.lock() {
        if guard.as_deref() == Some(resolution.approval_id.as_str()) {
            *guard = None;
        }
    }

    Ok(resolution.decision)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileDeleteTargetKind {
    File,
    Directory,
    Unknown,
}

fn resolve_delete_target_path(path: &str, work_dir: Option<&Path>) -> Option<PathBuf> {
    if path.trim().is_empty() {
        return None;
    }

    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        return Some(candidate);
    }

    match work_dir {
        Some(dir) => Some(dir.join(candidate)),
        None => Some(candidate),
    }
}

fn detect_file_delete_target_kind(path: &str, work_dir: Option<&Path>) -> FileDeleteTargetKind {
    let Some(resolved) = resolve_delete_target_path(path, work_dir) else {
        return FileDeleteTargetKind::Unknown;
    };

    if resolved.is_file() {
        FileDeleteTargetKind::File
    } else if resolved.is_dir() {
        FileDeleteTargetKind::Directory
    } else {
        FileDeleteTargetKind::Unknown
    }
}

fn critical_action_summary(
    tool_name: &str,
    input: &Value,
    work_dir: Option<&Path>,
) -> (String, String, String, bool) {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    match tool_name {
        "file_delete" => {
            let recursive = input
                .get("recursive")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let target_kind = detect_file_delete_target_kind(&path, work_dir);

            match (target_kind, recursive) {
                (FileDeleteTargetKind::File, _) => (
                    "删除文件".to_string(),
                    format!(
                        "将删除文件 {}",
                        if path.is_empty() {
                            "目标文件"
                        } else {
                            &path
                        }
                    ),
                    "该操作不可逆，删除后无法自动恢复。".to_string(),
                    true,
                ),
                (FileDeleteTargetKind::Directory, false) => (
                    "删除文件夹".to_string(),
                    format!(
                        "将删除文件夹 {}",
                        if path.is_empty() {
                            "目标文件夹"
                        } else {
                            &path
                        }
                    ),
                    "该操作不可逆，删除后无法自动恢复。".to_string(),
                    true,
                ),
                (FileDeleteTargetKind::Directory, true) => (
                    "递归删除文件夹".to_string(),
                    format!(
                        "将递归删除文件夹 {}",
                        if path.is_empty() {
                            "目标文件夹"
                        } else {
                            &path
                        }
                    ),
                    "该操作不可逆，文件夹及其内容删除后无法自动恢复。".to_string(),
                    true,
                ),
                (FileDeleteTargetKind::Unknown, true) => (
                    "递归删除目标".to_string(),
                    format!(
                        "将递归删除 {}",
                        if path.is_empty() {
                            "目标文件或文件夹"
                        } else {
                            &path
                        }
                    ),
                    "该操作不可逆，目标及其内容删除后无法自动恢复。".to_string(),
                    true,
                ),
                (FileDeleteTargetKind::Unknown, false) => (
                    "删除目标".to_string(),
                    format!(
                        "将删除 {}",
                        if path.is_empty() {
                            "目标文件或文件夹"
                        } else {
                            &path
                        }
                    ),
                    "该操作不可逆，删除后无法自动恢复。".to_string(),
                    true,
                ),
            }
        }
        "write_file" => (
            "写入文件".to_string(),
            format!(
                "将写入 {}",
                if path.is_empty() {
                    "目标文件"
                } else {
                    &path
                }
            ),
            "该操作可能覆盖现有内容，请确认影响范围。".to_string(),
            false,
        ),
        "edit" => (
            "修改文件".to_string(),
            format!(
                "将修改 {}",
                if path.is_empty() {
                    "目标文件"
                } else {
                    &path
                }
            ),
            "这可能改变现有文件内容，请确认替换目标正确。".to_string(),
            false,
        ),
        "bash" => {
            let command = input
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("命令");
            (
                "执行高危命令".to_string(),
                format!("将执行命令：{}", command),
                "该命令可能删除文件、重置环境或影响系统状态。".to_string(),
                true,
            )
        }
        "browser_click" | "browser_type" | "browser_press_key" | "browser_evaluate"
        | "browser_act" => (
            "提交网页操作".to_string(),
            "将执行可能触发提交、发送或状态变更的浏览器动作".to_string(),
            "这可能在外部系统中创建、修改或删除真实数据。".to_string(),
            true,
        ),
        _ => (
            "高危操作确认".to_string(),
            format!("将执行工具 {}", tool_name),
            "该操作具有较高风险，请确认后继续。".to_string(),
            false,
        ),
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason_last_completed_step: Option<String>,
}

impl AgentStateEvent {
    fn basic(
        session_id: impl Into<String>,
        state: impl Into<String>,
        detail: Option<String>,
        iteration: usize,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            state: state.into(),
            detail,
            iteration,
            stop_reason_kind: None,
            stop_reason_title: None,
            stop_reason_message: None,
            stop_reason_last_completed_step: None,
        }
    }

    fn stopped(session_id: impl Into<String>, iteration: usize, reason: &RunStopReason) -> Self {
        Self {
            session_id: session_id.into(),
            state: "stopped".to_string(),
            detail: reason
                .detail
                .clone()
                .or_else(|| Some(reason.message.clone())),
            iteration,
            stop_reason_kind: Some(reason.kind.as_key().to_string()),
            stop_reason_title: Some(reason.title.clone()),
            stop_reason_message: Some(reason.message.clone()),
            stop_reason_last_completed_step: reason.last_completed_step.clone(),
        }
    }
}

fn text_progress_signature(raw: &str) -> String {
    let mut hasher = DefaultHasher::new();
    raw.trim().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn json_progress_signature(value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    text_progress_signature(&serialized)
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

#[cfg(test)]
mod delete_confirmation_tests {
    use super::critical_action_summary;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn unique_temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("workclaw-{}-{}", label, Uuid::new_v4()))
    }

    #[test]
    fn file_delete_confirmation_describes_file_targets() {
        let file_path = unique_temp_path("delete-file.txt");
        let path_text = file_path.display().to_string();
        fs::write(&file_path, "danger").expect("create temp file");

        let (title, summary, impact, irreversible) =
            critical_action_summary("file_delete", &json!({ "path": path_text }), None);

        assert_eq!(title, "删除文件");
        assert_eq!(summary, format!("将删除文件 {}", file_path.display()));
        assert_eq!(impact, "该操作不可逆，删除后无法自动恢复。");
        assert!(irreversible);

        fs::remove_file(&file_path).expect("cleanup temp file");
    }

    #[test]
    fn file_delete_confirmation_describes_folder_targets() {
        let dir_path = unique_temp_path("delete-folder");
        let path_text = dir_path.display().to_string();
        fs::create_dir_all(&dir_path).expect("create temp folder");

        let (title, summary, impact, irreversible) =
            critical_action_summary("file_delete", &json!({ "path": path_text }), None);

        assert_eq!(title, "删除文件夹");
        assert_eq!(summary, format!("将删除文件夹 {}", dir_path.display()));
        assert_eq!(impact, "该操作不可逆，删除后无法自动恢复。");
        assert!(irreversible);

        fs::remove_dir(&dir_path).expect("cleanup temp folder");
    }

    #[test]
    fn file_delete_confirmation_describes_recursive_folder_targets() {
        let dir_path = unique_temp_path("delete-folder-recursive");
        let nested_file = dir_path.join("nested.txt");
        let path_text = dir_path.display().to_string();
        fs::create_dir_all(&dir_path).expect("create temp folder");
        fs::write(&nested_file, "nested").expect("create nested file");

        let (title, summary, impact, irreversible) = critical_action_summary(
            "file_delete",
            &json!({ "path": path_text, "recursive": true }),
            None,
        );

        assert_eq!(title, "递归删除文件夹");
        assert_eq!(summary, format!("将递归删除文件夹 {}", dir_path.display()));
        assert_eq!(impact, "该操作不可逆，文件夹及其内容删除后无法自动恢复。");
        assert!(irreversible);

        fs::remove_dir_all(&dir_path).expect("cleanup recursive temp folder");
    }

    #[test]
    fn file_delete_confirmation_falls_back_for_unknown_targets() {
        let missing_path = unique_temp_path("missing-target");
        let path_text = missing_path.display().to_string();

        let (title, summary, impact, irreversible) =
            critical_action_summary("file_delete", &json!({ "path": path_text }), None);

        assert_eq!(title, "删除目标");
        assert_eq!(summary, format!("将删除 {}", missing_path.display()));
        assert_eq!(impact, "该操作不可逆，删除后无法自动恢复。");
        assert!(irreversible);
    }
}

impl AgentExecutor {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            max_iterations: RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat).max_turns,
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
        on_token: impl Fn(StreamDelta) + Send + Clone,
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
        let mut run_budget_policy = RunBudgetPolicy::for_scope(RunBudgetScope::GeneralChat);
        run_budget_policy.max_turns = max_iterations;
        let route_node_timeout_secs = route_node_timeout_secs.unwrap_or(60).clamp(5, 600);
        let route_retry_count = route_retry_count.unwrap_or(0).clamp(0, 2);
        let mut iteration = 0;
        let route_run_id = Uuid::new_v4().to_string();
        let persisted_run_id = if let (Some(app), Some(sid)) = (app_handle, session_id) {
            resolve_current_session_run_id(app, sid).await
        } else {
            None
        };
        let mut tool_failure_streak: Option<ToolFailureStreak> = None;
        let mut progress_history: Vec<ProgressFingerprint> = Vec::new();
        let mut latest_browser_progress: Option<BrowserProgressSnapshot> = None;

        loop {
            // 检查取消标志
            if let Some(ref flag) = cancel_flag {
                if flag.load(Ordering::SeqCst) {
                    eprintln!("[agent] 任务被用户取消");
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let _ = app.emit(
                            "agent-state-event",
                            AgentStateEvent::basic(
                                sid,
                                "finished",
                                Some("用户取消".to_string()),
                                iteration,
                            ),
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
                let stop_reason = RunStopReason::max_turns(max_iterations);
                if let (Some(app), Some(sid)) = (app_handle, session_id) {
                    let _ = app.emit(
                        "agent-state-event",
                        AgentStateEvent::stopped(sid, iteration, &stop_reason),
                    );
                }
                return Err(anyhow!(encode_run_stop_reason(&stop_reason)));
            }
            iteration += 1;

            eprintln!("[agent] Iteration {}/{}", iteration, max_iterations);

            // 发射 thinking 状态事件
            if let (Some(app), Some(sid)) = (app_handle, session_id) {
                let _ = app.emit(
                    "agent-state-event",
                    AgentStateEvent::basic(sid, "thinking", None, iteration),
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
            let response_result = if api_format == "anthropic" {
                adapters::anthropic::chat_stream_with_tools(
                    base_url,
                    api_key,
                    model,
                    &system_prompt,
                    trimmed.clone(),
                    tools,
                    on_token.clone(),
                )
                .await
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
                .await
            };

            let response = match response_result {
                Ok(response) => response,
                Err(err) => {
                    if let (Some(app), Some(sid)) = (app_handle, session_id) {
                        let _ = app.emit(
                            "agent-state-event",
                            AgentStateEvent::basic(sid, "error", Some(err.to_string()), iteration),
                        );
                    }
                    return Err(err);
                }
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
                            AgentStateEvent::basic(sid, "finished", None, iteration),
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
                            AgentStateEvent::basic(
                                sid,
                                "tool_calling",
                                Some(tool_names.join(", ")),
                                iteration,
                            ),
                        );
                    }

                    // 执行所有工具调用
                    let mut tool_results = vec![];
                    let mut repeated_failure_summary: Option<String> = None;
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
                                        AgentStateEvent::basic(
                                            sid,
                                            "finished",
                                            Some("用户取消".to_string()),
                                            iteration,
                                        ),
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
                            if let Some(run_id) = persisted_run_id.as_ref() {
                                let _ = append_tool_run_event(
                                    app,
                                    sid,
                                    SessionRunEvent::ToolStarted {
                                        run_id: run_id.clone(),
                                        tool_name: call.name.clone(),
                                        call_id: call.id.clone(),
                                        input: call.input.clone(),
                                    },
                                )
                                .await;
                            }
                        }

                        // 权限确认检查：在执行工具前判断是否需要用户确认
                        if permission_mode.needs_confirmation(
                            &call.name,
                            &call.input,
                            tool_ctx.work_dir.as_deref(),
                        ) {
                            let approval_decision = if let (Some(app), Some(sid)) =
                                (app_handle, session_id)
                            {
                                let runtime = match resolve_approval_wait_runtime(app) {
                                    Ok(runtime) => runtime,
                                    Err(err) => {
                                        let rejection_message = err.to_string();
                                        let _ = app.emit(
                                            "tool-call-event",
                                            ToolCallEvent {
                                                session_id: sid.to_string(),
                                                tool_name: call.name.clone(),
                                                tool_input: call.input.clone(),
                                                tool_output: Some(rejection_message.clone()),
                                                status: "error".to_string(),
                                            },
                                        );
                                        if let Some(run_id) = persisted_run_id.as_ref() {
                                            let _ = append_tool_run_event(
                                                app,
                                                sid,
                                                SessionRunEvent::ToolCompleted {
                                                    run_id: run_id.clone(),
                                                    tool_name: call.name.clone(),
                                                    call_id: call.id.clone(),
                                                    input: call.input.clone(),
                                                    output: rejection_message.clone(),
                                                    is_error: true,
                                                },
                                            )
                                            .await;
                                        }
                                        tool_results.push(ToolResult {
                                            tool_use_id: call.id.clone(),
                                            content: rejection_message,
                                        });
                                        continue;
                                    }
                                };
                                let approval_bus_enabled =
                                    approval_bus_rollout_enabled_with_pool(&runtime.pool)
                                        .await
                                        .unwrap_or(true);

                                if approval_bus_enabled {
                                    match find_matching_approval_rule_with_pool(
                                        &runtime.pool,
                                        &call.name,
                                        &call.input,
                                    )
                                    .await
                                    {
                                        Ok(Some(_)) => Some(ApprovalDecision::AllowAlways),
                                        Ok(None) | Err(_) => {
                                            match request_tool_approval_and_wait(
                                                &runtime,
                                                Some(app),
                                                sid,
                                                persisted_run_id.as_deref(),
                                                &call.name,
                                                &call.id,
                                                &call.input,
                                                tool_ctx.work_dir.as_deref(),
                                                cancel_flag.clone(),
                                            )
                                            .await
                                            {
                                                Ok(decision) => Some(decision),
                                                Err(err) => {
                                                    let rejection_message = err.to_string();
                                                    let _ = app.emit(
                                                        "tool-call-event",
                                                        ToolCallEvent {
                                                            session_id: sid.to_string(),
                                                            tool_name: call.name.clone(),
                                                            tool_input: call.input.clone(),
                                                            tool_output: Some(
                                                                rejection_message.clone(),
                                                            ),
                                                            status: "error".to_string(),
                                                        },
                                                    );
                                                    if let Some(run_id) = persisted_run_id.as_ref()
                                                    {
                                                        let _ = append_tool_run_event(
                                                            app,
                                                            sid,
                                                            SessionRunEvent::ToolCompleted {
                                                                run_id: run_id.clone(),
                                                                tool_name: call.name.clone(),
                                                                call_id: call.id.clone(),
                                                                input: call.input.clone(),
                                                                output: rejection_message.clone(),
                                                                is_error: true,
                                                            },
                                                        )
                                                        .await;
                                                    }
                                                    tool_results.push(ToolResult {
                                                        tool_use_id: call.id.clone(),
                                                        content: rejection_message,
                                                    });
                                                    None
                                                }
                                            }
                                        }
                                    }
                                } else if let Some(ref confirm_state) = tool_confirm_tx {
                                    let (tx, rx) = std::sync::mpsc::channel::<bool>();
                                    if let Ok(mut guard) = confirm_state.lock() {
                                        *guard = Some(tx);
                                    }

                                    let confirmation = wait_for_tool_confirmation(
                                        &rx,
                                        std::time::Duration::from_secs(TOOL_CONFIRM_TIMEOUT_SECS),
                                    );

                                    if let Ok(mut guard) = confirm_state.lock() {
                                        *guard = None;
                                    }

                                    match confirmation {
                                        ToolConfirmationDecision::Confirmed => {
                                            Some(ApprovalDecision::AllowOnce)
                                        }
                                        ToolConfirmationDecision::Rejected => {
                                            Some(ApprovalDecision::Deny)
                                        }
                                        ToolConfirmationDecision::TimedOut => {
                                            tool_results.push(ToolResult {
                                                tool_use_id: call.id.clone(),
                                                content: "工具确认超时，已取消此操作".to_string(),
                                            });
                                            None
                                        }
                                    }
                                } else {
                                    Some(ApprovalDecision::AllowOnce)
                                }
                            } else if let Some(ref confirm_state) = tool_confirm_tx {
                                let (tx, rx) = std::sync::mpsc::channel::<bool>();
                                if let Ok(mut guard) = confirm_state.lock() {
                                    *guard = Some(tx);
                                }

                                let confirmation = wait_for_tool_confirmation(
                                    &rx,
                                    std::time::Duration::from_secs(TOOL_CONFIRM_TIMEOUT_SECS),
                                );

                                if let Ok(mut guard) = confirm_state.lock() {
                                    *guard = None;
                                }

                                match confirmation {
                                    ToolConfirmationDecision::Confirmed => {
                                        Some(ApprovalDecision::AllowOnce)
                                    }
                                    ToolConfirmationDecision::Rejected => {
                                        Some(ApprovalDecision::Deny)
                                    }
                                    ToolConfirmationDecision::TimedOut => {
                                        tool_results.push(ToolResult {
                                            tool_use_id: call.id.clone(),
                                            content: "工具确认超时，已取消此操作".to_string(),
                                        });
                                        None
                                    }
                                }
                            } else {
                                Some(ApprovalDecision::AllowOnce)
                            };

                            let Some(approval_decision) = approval_decision else {
                                continue;
                            };

                            if approval_decision == ApprovalDecision::Deny {
                                let rejection_message = "用户拒绝了此操作";
                                if let (Some(app), Some(sid)) = (app_handle, session_id) {
                                    let _ = app.emit(
                                        "tool-call-event",
                                        ToolCallEvent {
                                            session_id: sid.to_string(),
                                            tool_name: call.name.clone(),
                                            tool_input: call.input.clone(),
                                            tool_output: Some(rejection_message.to_string()),
                                            status: "error".to_string(),
                                        },
                                    );
                                    if let Some(run_id) = persisted_run_id.as_ref() {
                                        let _ = append_tool_run_event(
                                            app,
                                            sid,
                                            SessionRunEvent::ToolCompleted {
                                                run_id: run_id.clone(),
                                                tool_name: call.name.clone(),
                                                call_id: call.id.clone(),
                                                input: call.input.clone(),
                                                output: rejection_message.to_string(),
                                                is_error: true,
                                            },
                                        )
                                        .await;
                                    }
                                }
                                tool_results.push(ToolResult {
                                    tool_use_id: call.id.clone(),
                                    content: rejection_message.to_string(),
                                });
                                continue;
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
                            let (result, is_error) = if let Some(parse_error) =
                                extract_tool_call_parse_error(&call.input)
                            {
                                (
                                    format!(
                                        "工具参数错误: {}。请提供完整且合法的 JSON 参数后再重试。",
                                        parse_error
                                    ),
                                    true,
                                )
                            } else {
                                match self.registry.get(&call.name) {
                                    Some(tool) => {
                                        // 检查白名单：若设置了白名单但工具不在其中，拒绝执行
                                        if let Some(whitelist) = allowed_tools {
                                            if !whitelist.iter().any(|w| w == &call.name) {
                                                (
                                                    format!(
                                                        "此 Skill 不允许使用工具: {}",
                                                        call.name
                                                    ),
                                                    true,
                                                )
                                            } else {
                                                let tool_clone = Arc::clone(&tool);
                                                let input_clone = call.input.clone();
                                                let ctx_clone = tool_ctx.clone();
                                                let handle =
                                                    tokio::task::spawn_blocking(move || {
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
                                                            "TIMEOUT: 子 Skill 执行超时"
                                                                .to_string(),
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
                                                    Err(_) => (
                                                        "TIMEOUT: 子 Skill 执行超时".to_string(),
                                                        true,
                                                    ),
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
                                }
                            };
                            if !is_error || attempt >= max_attempts {
                                break (result, is_error);
                            }
                        };
                        // 截断过长的工具输出，防止超出上下文窗口
                        let result = truncate_tool_output(&result, MAX_TOOL_OUTPUT_CHARS);

                        if is_error {
                            if let Some(summary) = update_tool_failure_streak(
                                &mut tool_failure_streak,
                                &call.name,
                                &call.input,
                                &result,
                            ) {
                                repeated_failure_summary = Some(summary);
                            }
                        } else {
                            tool_failure_streak = None;
                        }

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
                            if let Some(run_id) = persisted_run_id.as_ref() {
                                let _ = append_tool_run_event(
                                    app,
                                    sid,
                                    SessionRunEvent::ToolCompleted {
                                        run_id: run_id.clone(),
                                        tool_name: call.name.clone(),
                                        call_id: call.id.clone(),
                                        input: call.input.clone(),
                                        output: result.clone(),
                                        is_error,
                                    },
                                )
                                .await;
                            }
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

                        if !is_error {
                            let browser_progress_snapshot =
                                BrowserProgressSnapshot::from_tool_output(&call.name, &result);
                            let input_signature = json_progress_signature(&call.input);
                            let output_signature =
                                if let Some(snapshot) = browser_progress_snapshot.as_ref() {
                                    snapshot.progress_signature()
                                } else {
                                    text_progress_signature(&result)
                                };
                            if let Some(snapshot) = browser_progress_snapshot {
                                latest_browser_progress = Some(snapshot);
                            }
                            progress_history.push(ProgressFingerprint::tool_result(
                                call.name.clone(),
                                input_signature,
                                output_signature,
                            ));
                        }

                        tool_results.push(ToolResult {
                            tool_use_id: call.id.clone(),
                            content: result,
                        });

                        if repeated_failure_summary.is_some() {
                            break;
                        }
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

                    if let Some(summary) = repeated_failure_summary {
                        messages.push(json!({
                            "role": "assistant",
                            "content": summary
                        }));
                        if let (Some(app), Some(sid)) = (app_handle, session_id) {
                            let _ = app.emit(
                                "agent-state-event",
                                AgentStateEvent::basic(
                                    sid,
                                    "finished",
                                    Some("重复工具失败已熔断".to_string()),
                                    iteration,
                                ),
                            );
                        }
                        return Ok(messages);
                    }

                    let progress_evaluation =
                        ProgressGuard::evaluate(&run_budget_policy, &progress_history);
                    if let Some(mut warning) = progress_evaluation.warning {
                        if let Some(last_completed_step) = latest_browser_progress
                            .as_ref()
                            .and_then(BrowserProgressSnapshot::last_completed_step)
                        {
                            warning = warning.with_last_completed_step(last_completed_step);
                        }
                        if let (Some(app), Some(sid)) = (app_handle, session_id) {
                            let _ = append_run_guard_warning_event(app, sid, &warning).await;
                        }
                    }
                    if let Some(mut stop_reason) = progress_evaluation.stop_reason {
                        if let Some(last_completed_step) = latest_browser_progress
                            .as_ref()
                            .and_then(BrowserProgressSnapshot::last_completed_step)
                        {
                            stop_reason = stop_reason.with_last_completed_step(last_completed_step);
                        }
                        if let (Some(app), Some(sid)) = (app_handle, session_id) {
                            let _ = app.emit(
                                "agent-state-event",
                                AgentStateEvent::stopped(sid, iteration, &stop_reason),
                            );
                        }
                        return Err(anyhow!(encode_run_stop_reason(&stop_reason)));
                    }

                    // 继续下一轮迭代
                    continue;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::request_tool_approval_and_wait;
    use super::wait_for_tool_confirmation;
    use super::ApprovalWaitRuntime;
    use super::ToolConfirmationDecision;
    use crate::agent::{FileDeleteTool, Tool, ToolContext};
    use crate::approval_bus::{ApprovalDecision, ApprovalManager};
    use crate::session_journal::SessionJournalStore;
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn tool_confirmation_timeout_is_treated_as_rejection() {
        let (_tx, rx) = mpsc::channel::<bool>();
        let decision = wait_for_tool_confirmation(&rx, Duration::from_millis(5));
        assert_eq!(decision, ToolConfirmationDecision::TimedOut);
    }

    #[test]
    fn tool_confirmation_false_is_rejected() {
        let (tx, rx) = mpsc::channel::<bool>();
        tx.send(false).expect("send");
        let decision = wait_for_tool_confirmation(&rx, Duration::from_millis(5));
        assert_eq!(decision, ToolConfirmationDecision::Rejected);
    }

    #[tokio::test]
    async fn approval_bus_blocks_file_delete_until_resolved() {
        let db_dir = tempdir().expect("create db dir");
        let db_url = format!(
            "sqlite://{}?mode=rwc",
            db_dir.path().join("approval-test.db").to_string_lossy()
        );
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .expect("connect sqlite");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_message_id TEXT NOT NULL DEFAULT '',
                assistant_message_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'queued',
                buffered_text TEXT NOT NULL DEFAULT '',
                error_kind TEXT NOT NULL DEFAULT '',
                error_message TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_runs");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session_run_events (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_run_events");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                call_id TEXT NOT NULL DEFAULT '',
                tool_name TEXT NOT NULL,
                input_json TEXT NOT NULL DEFAULT '{}',
                summary TEXT NOT NULL DEFAULT '',
                impact TEXT NOT NULL DEFAULT '',
                irreversible INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',
                decision TEXT NOT NULL DEFAULT '',
                notify_targets_json TEXT NOT NULL DEFAULT '[]',
                resume_payload_json TEXT NOT NULL DEFAULT '{}',
                resolved_by_surface TEXT NOT NULL DEFAULT '',
                resolved_by_user TEXT NOT NULL DEFAULT '',
                resolved_at TEXT,
                resumed_at TEXT,
                expires_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create approvals");

        let journal_dir = tempdir().expect("create journal dir");
        let runtime = ApprovalWaitRuntime {
            pool: pool.clone(),
            journal: Arc::new(SessionJournalStore::new(journal_dir.path().to_path_buf())),
            approval_manager: Arc::new(ApprovalManager::default()),
            pending_bridge: Arc::new(Mutex::new(None)),
        };

        let work_dir = tempdir().expect("create work dir");
        let target_dir = work_dir.path().join("danger");
        std::fs::create_dir_all(target_dir.join("nested")).expect("create target tree");
        std::fs::write(target_dir.join("nested").join("file.txt"), "danger")
            .expect("write nested file");

        let input = json!({
            "path": target_dir.to_string_lossy().to_string(),
            "recursive": true,
        });
        let tool_ctx = ToolContext {
            work_dir: Some(PathBuf::from(work_dir.path())),
            allowed_tools: None,
        };

        let runtime_clone = runtime.clone();
        let manager = runtime.approval_manager.clone();
        let pool_clone = pool.clone();
        let input_clone = input.clone();
        let tool_ctx_clone = tool_ctx.clone();
        let work_dir_path = work_dir.path().to_path_buf();

        let handle = tokio::spawn(async move {
            let decision = request_tool_approval_and_wait(
                &runtime_clone,
                None,
                "sess-approval",
                Some("run-approval"),
                "file_delete",
                "call-approval",
                &input_clone,
                Some(work_dir_path.as_path()),
                None,
            )
            .await
            .expect("approval should resolve");
            assert_eq!(decision, ApprovalDecision::AllowOnce);

            let tool = FileDeleteTool;
            tool.execute(input_clone, &tool_ctx_clone)
        });

        let mut pending_row: Option<(String, String)> = None;
        for _ in 0..20 {
            if let Some(row) = sqlx::query_as::<_, (String, String)>(
                "SELECT id, status FROM approvals WHERE session_id = ?",
            )
            .bind("sess-approval")
            .fetch_optional(&pool)
            .await
            .expect("query pending approval")
            {
                pending_row = Some(row);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(target_dir.exists(), "directory must remain before approval");

        let (approval_id, status) = pending_row.expect("load pending approval");
        assert_eq!(status, "pending");

        manager
            .resolve_with_pool(
                &pool_clone,
                &approval_id,
                ApprovalDecision::AllowOnce,
                "desktop",
                "tester",
            )
            .await
            .expect("resolve pending approval");

        let result = handle
            .await
            .expect("join task")
            .expect("file delete success");
        assert!(result.contains("成功删除"));
        assert!(
            !target_dir.exists(),
            "directory should be removed after approval"
        );
    }
}
