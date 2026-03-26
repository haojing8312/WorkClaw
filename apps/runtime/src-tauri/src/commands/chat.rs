use super::chat_compaction;
use super::chat_runtime_io as chat_io;
use super::chat_session_io;
use super::skills::DbState;
use crate::agent::AgentExecutor;
use crate::agent::runtime::{SessionAdmissionGateState, SessionRuntime};
use crate::approval_bus::ApprovalManager;
use crate::diagnostics::{self, ManagedDiagnosticsState};
use crate::session_journal::SessionJournalStateHandle;
use serde::Deserialize;
use serde_json::Value;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

/// 全局 AskUser 响应通道（用于 answer_user_question command）
pub use crate::agent::runtime::AskUserState;

/// 工具确认通道（用于 confirm_tool_execution command）
pub use crate::agent::runtime::ToolConfirmResponder;
pub struct ToolConfirmState(pub ToolConfirmResponder);

/// 通用审批管理器（高风险审批总线）
pub struct ApprovalManagerState(pub Arc<ApprovalManager>);

/// 旧版桌面确认对话框与审批总线之间的过渡桥接（仅保留最近一条待确认 approval）
pub struct PendingApprovalBridgeState(pub Arc<std::sync::Mutex<Option<String>>>);

/// 全局搜索缓存（跨会话共享，在 lib.rs 中创建）
pub use crate::agent::runtime::SearchCacheState;

/// Agent 取消标志（用于 cancel_agent command 停止正在执行的 Agent）
pub use crate::agent::runtime::CancelFlagState;

pub use crate::agent::runtime::{SkillRouteEvent, StreamToken};

#[cfg(test)]
pub(crate) fn build_group_orchestrator_report_preview(
    request: crate::agent::group_orchestrator::GroupRunRequest,
) -> String {
    let outcome = crate::agent::group_orchestrator::simulate_group_run(request);
    outcome.final_report
}

pub fn emit_skill_route_event(app: &AppHandle, event: SkillRouteEvent) {
    let _ = app.emit("skill-route-node-updated", event);
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageRequest {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub parts: Vec<SendMessagePart>,
    #[serde(rename = "maxIterations", default)]
    pub max_iterations: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum SendMessagePart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        name: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
        size: usize,
        data: String,
    },
    #[serde(rename = "file_text")]
    FileText {
        name: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
        size: usize,
        text: String,
        truncated: Option<bool>,
    },
}

impl SendMessageRequest {
    fn summary_text(&self) -> String {
        let mut summary_parts = Vec::new();
        let text = self
            .parts
            .iter()
            .filter_map(|part| match part {
                SendMessagePart::Text { text } => Some(text.trim()),
                _ => None,
            })
            .find(|text| !text.is_empty())
            .unwrap_or("");
        if !text.is_empty() {
            summary_parts.push(text.to_string());
        }
        let image_count = self
            .parts
            .iter()
            .filter(|part| matches!(part, SendMessagePart::Image { .. }))
            .count();
        let text_file_count = self
            .parts
            .iter()
            .filter(|part| matches!(part, SendMessagePart::FileText { .. }))
            .count();
        if image_count > 0 {
            summary_parts.push(format!("[图片 {} 张]", image_count));
        }
        if text_file_count > 0 {
            summary_parts.push(format!("[文本文件 {} 个]", text_file_count));
        }
        summary_parts.join(" ")
    }

    fn parts_as_json(&self) -> Result<Vec<Value>, String> {
        self.parts
            .iter()
            .map(|part| serde_json::to_value(part).map_err(|err| err.to_string()))
            .collect()
    }
}

#[tauri::command]
pub async fn create_session(
    app: AppHandle,
    skill_id: String,
    model_id: String,
    work_dir: Option<String>,
    employee_id: Option<String>,
    title: Option<String>,
    permission_mode: Option<String>,
    session_mode: Option<String>,
    team_id: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let session_id = create_session_with_pool(
        &db.0,
        skill_id.clone(),
        model_id.clone(),
        work_dir.clone(),
        employee_id.clone(),
        title.clone(),
        permission_mode.clone(),
        session_mode.clone(),
        team_id.clone(),
    )
    .await?;

    if let Some(diagnostics_state) = app.try_state::<ManagedDiagnosticsState>() {
        let storage = super::desktop_lifecycle::collect_database_storage_snapshot(&app);
        let counts = super::desktop_lifecycle::collect_database_counts(&db.0).await;
        let _ = diagnostics::write_audit_record(
            &diagnostics_state.0.paths,
            "session",
            "create_session",
            "session created",
            Some(serde_json::json!({
                "session_id": session_id,
                "skill_id": skill_id,
                "model_id": model_id,
                "work_dir": work_dir,
                "employee_id": employee_id,
                "title": title,
                "permission_mode": permission_mode,
                "session_mode": session_mode,
                "team_id": team_id,
                "counts": counts,
                "storage": storage,
            })),
        );
    }

    Ok(session_id)
}

pub async fn create_session_with_pool(
    pool: &sqlx::SqlitePool,
    skill_id: String,
    model_id: String,
    work_dir: Option<String>,
    employee_id: Option<String>,
    title: Option<String>,
    permission_mode: Option<String>,
    session_mode: Option<String>,
    team_id: Option<String>,
) -> Result<String, String> {
    chat_session_io::create_session_with_pool(
        pool,
        skill_id,
        model_id,
        work_dir,
        employee_id,
        title,
        permission_mode,
        session_mode,
        team_id,
    )
    .await
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    request: SendMessageRequest,
    db: State<'_, DbState>,
    agent_executor: State<'_, Arc<AgentExecutor>>,
    journal: State<'_, SessionJournalStateHandle>,
    cancel_flag: State<'_, CancelFlagState>,
) -> Result<(), String> {
    let session_id = request.session_id.clone();
    let user_message = request.summary_text();
    let user_message_parts = request.parts_as_json()?;

    let admission_gate = app
        .try_state::<SessionAdmissionGateState>()
        .ok_or_else(|| "SessionAdmissionGateState unavailable".to_string())?;
    let _admission_lease = admission_gate
        .0
        .try_acquire(&session_id)
        .map_err(|conflict| conflict.to_string())?;

    if let Some(diagnostics_state) = app.try_state::<ManagedDiagnosticsState>() {
        let _ = diagnostics::write_log_record(
            &diagnostics_state.0.paths,
            diagnostics::LogLevel::Info,
            "chat",
            "send_message",
            "chat send_message invoked",
            Some(serde_json::json!({
                "session_id": session_id,
                "user_message_preview": user_message.chars().take(80).collect::<String>(),
            })),
        );
    }

    // 重置取消标志
    cancel_flag.0.store(false, Ordering::SeqCst);
    let cancel_flag_clone = cancel_flag.0.clone();

    // 保存用户消息
    let user_message_parts_json = serde_json::to_string(&user_message_parts)
        .map_err(|err| format!("序列化附件消息失败: {err}"))?;
    let msg_id = chat_io::insert_session_message_with_pool(
        &db.0,
        &session_id,
        "user",
        &user_message,
        Some(&user_message_parts_json),
    )
    .await?;
    if let Some(diagnostics_state) = app.try_state::<ManagedDiagnosticsState>() {
        let counts = super::desktop_lifecycle::collect_database_counts(&db.0).await;
        let storage = super::desktop_lifecycle::collect_database_storage_snapshot(&app);
        let _ = diagnostics::write_audit_record(
            &diagnostics_state.0.paths,
            "message",
            "message_inserted",
            "user message inserted",
            Some(serde_json::json!({
                "session_id": session_id,
                "message_id": msg_id,
                "role": "user",
                "content_preview": user_message.chars().take(120).collect::<String>(),
                "content_parts_count": user_message_parts.len(),
                "counts": counts,
                "storage": storage,
            })),
        );
    }
    chat_io::maybe_update_session_title_from_first_user_message_with_pool(
        &db.0,
        &session_id,
        &user_message,
    )
    .await?;
    if let Some(diagnostics_state) = app.try_state::<ManagedDiagnosticsState>() {
        let title_row = sqlx::query_scalar::<_, String>(
            "SELECT COALESCE(title, '') FROM sessions WHERE id = ?",
        )
        .bind(&session_id)
        .fetch_optional(&db.0)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
        if !title_row.trim().is_empty() {
            let counts = super::desktop_lifecycle::collect_database_counts(&db.0).await;
            let storage = super::desktop_lifecycle::collect_database_storage_snapshot(&app);
            let _ = diagnostics::write_audit_record(
                &diagnostics_state.0.paths,
                "session",
                "session_title_updated",
                "session title evaluated after first user message",
                Some(serde_json::json!({
                    "session_id": session_id,
                    "title": title_row,
                    "counts": counts,
                    "storage": storage,
                })),
            );
        }
    }

    if chat_io::maybe_handle_team_entry_pre_execution_with_pool(
        &app,
        &db.0,
        journal.0.as_ref(),
        &session_id,
        &msg_id,
        &user_message,
    )
    .await?
    {
        return Ok(());
    }

    // 使用全局工具确认通道（在 lib.rs 中创建）
    let tool_confirm_responder = app.state::<ToolConfirmState>().0.clone();
    SessionRuntime::run_send_message(
        &app,
        agent_executor.inner(),
        &db.0,
        journal.0.as_ref(),
        &session_id,
        &msg_id,
        &user_message,
        &user_message_parts,
        request.max_iterations,
        cancel_flag_clone.clone(),
        tool_confirm_responder,
    )
    .await
    .map_err(|error| {
        if let Some(diagnostics_state) = app.try_state::<ManagedDiagnosticsState>() {
            let _ = diagnostics::write_log_record(
                &diagnostics_state.0.paths,
                diagnostics::LogLevel::Error,
                "chat",
                "send_message_finalize_failed",
                &error,
                Some(serde_json::json!({
                    "session_id": session_id,
                    "message_id": msg_id,
                })),
            );
        }
        error
    })
}

#[cfg(test)]
mod tests {
    use super::build_group_orchestrator_report_preview;
    use crate::agent::runtime::SessionAdmissionConflict;
    use crate::commands::chat_runtime_io;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn build_memory_dir_for_session_keeps_legacy_skill_bucket_without_employee() {
        let base = Path::new("C:/workclaw/app-data");
        let dir = chat_runtime_io::build_memory_dir_for_session(base, "builtin-general", "");
        assert_eq!(
            dir,
            Path::new("C:/workclaw/app-data")
                .join("memory")
                .join("builtin-general")
        );
    }

    #[test]
    fn build_memory_dir_for_session_isolates_by_employee_when_provided() {
        let base = Path::new("C:/workclaw/app-data");
        let dir = chat_runtime_io::build_memory_dir_for_session(
            base,
            "builtin-general",
            "Sales Lead/华东",
        );
        assert_eq!(
            dir,
            Path::new("C:/workclaw/app-data")
                .join("memory")
                .join("employees")
                .join("sales_lead")
                .join("skills")
                .join("builtin-general")
        );
    }

    #[test]
    fn extract_skill_prompt_supports_lowercase_skill_md() {
        let mut files = HashMap::new();
        files.insert("skill.md".to_string(), b"# lowercase skill".to_vec());
        let content = chat_runtime_io::extract_skill_prompt_from_decrypted_files(&files);
        assert_eq!(content.as_deref(), Some("# lowercase skill"));
    }

    #[test]
    fn group_orchestrator_preview_contains_plan_execute_summary_sections() {
        let report = build_group_orchestrator_report_preview(
            crate::agent::group_orchestrator::GroupRunRequest {
                group_id: "group-1".to_string(),
                coordinator_employee_id: "project_manager".to_string(),
                planner_employee_id: None,
                reviewer_employee_id: None,
                member_employee_ids: vec![
                    "project_manager".to_string(),
                    "dev_team".to_string(),
                    "qa_team".to_string(),
                ],
                execute_targets: Vec::new(),
                user_goal: "实现群组协作编排".to_string(),
                execution_window: 3,
                timeout_employee_ids: Vec::new(),
                max_retry_per_step: 1,
            },
        );

        assert!(report.contains("计划"));
        assert!(report.contains("执行"));
        assert!(report.contains("汇报"));
    }

    #[test]
    fn session_run_conflict_error_is_stable() {
        let error = SessionAdmissionConflict::new("session-1").to_string();

        assert!(error.starts_with("SESSION_RUN_CONFLICT:"));
        assert!(error.contains("当前会话仍在执行中"));
    }
}

pub use super::chat_session_commands::export_session_markdown_with_pool;

pub use super::chat_compaction::CompactionResult;

/// 手动触发上下文压缩
#[tauri::command]
pub async fn compact_context(
    session_id: String,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<CompactionResult, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    chat_compaction::compact_context_with_pool(&db.0, &session_id, &app_data_dir).await
}
