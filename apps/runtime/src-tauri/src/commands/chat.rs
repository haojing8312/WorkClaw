use super::chat_compaction;
use super::chat_runtime_io as chat_io;
use super::chat_send_message_flow::{self, PrepareSendMessageParams};
use super::chat_session_io;
use super::skills::DbState;
use crate::agent::tools::search_providers::cache::SearchCache;
use crate::agent::tools::AskUserResponder;
use crate::agent::AgentExecutor;
use crate::session_journal::SessionJournalStateHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

/// 全局 AskUser 响应通道（用于 answer_user_question command）
pub struct AskUserState(pub AskUserResponder);

/// 工具确认通道（用于 confirm_tool_execution command）
pub type ToolConfirmResponder =
    std::sync::Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<bool>>>>;
pub struct ToolConfirmState(pub ToolConfirmResponder);

/// 全局搜索缓存（跨会话共享，在 lib.rs 中创建）
pub struct SearchCacheState(pub Arc<SearchCache>);

/// Agent 取消标志（用于 cancel_agent command 停止正在执行的 Agent）
pub struct CancelFlagState(pub Arc<AtomicBool>);

#[cfg(test)]
pub(crate) fn build_group_orchestrator_report_preview(
    request: crate::agent::group_orchestrator::GroupRunRequest,
) -> String {
    let outcome = crate::agent::group_orchestrator::simulate_group_run(request);
    outcome.final_report
}

#[derive(serde::Serialize, Clone)]
pub(crate) struct StreamToken {
    pub(crate) session_id: String,
    pub(crate) token: String,
    pub(crate) done: bool,
    #[serde(default)]
    pub(crate) sub_agent: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct SkillRouteEvent {
    pub session_id: String,
    pub route_run_id: String,
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub skill_name: String,
    pub depth: usize,
    pub status: String,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

pub fn emit_skill_route_event(app: &AppHandle, event: SkillRouteEvent) {
    let _ = app.emit("skill-route-node-updated", event);
}

#[tauri::command]
pub async fn create_session(
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
    create_session_with_pool(
        &db.0,
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
    session_id: String,
    user_message: String,
    db: State<'_, DbState>,
    agent_executor: State<'_, Arc<AgentExecutor>>,
    journal: State<'_, SessionJournalStateHandle>,
    cancel_flag: State<'_, CancelFlagState>,
) -> Result<(), String> {
    // 重置取消标志
    cancel_flag.0.store(false, Ordering::SeqCst);
    let cancel_flag_clone = cancel_flag.0.clone();

    // 保存用户消息
    let msg_id =
        chat_io::insert_session_message_with_pool(&db.0, &session_id, "user", &user_message)
            .await?;
    chat_io::maybe_update_session_title_from_first_user_message_with_pool(
        &db.0,
        &session_id,
        &user_message,
    )
        .await?;

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

    let prepared_context =
        chat_send_message_flow::prepare_send_message_context(PrepareSendMessageParams {
            app: &app,
            db: &db.0,
            agent_executor: agent_executor.inner(),
            session_id: &session_id,
            user_message: &user_message,
        })
        .await?;

    // 使用全局工具确认通道（在 lib.rs 中创建）
    let tool_confirm_responder = app.state::<ToolConfirmState>().0.clone();
    let run_id = Uuid::new_v4().to_string();
    chat_io::append_run_started_with_pool(
        &db.0,
        journal.0.as_ref(),
        &session_id,
        &run_id,
        &msg_id,
    )
    .await?;

    let route_execution = chat_send_message_flow::execute_send_message_route(
        &app,
        agent_executor.as_ref(),
        &db.0,
        &session_id,
        &prepared_context,
        cancel_flag_clone.clone(),
        tool_confirm_responder,
    )
    .await;

    chat_send_message_flow::finalize_send_message_execution(
        &app,
        &db.0,
        journal.0.as_ref(),
        &session_id,
        &run_id,
        route_execution,
        prepared_context.messages.len(),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{build_group_orchestrator_report_preview, chat_runtime_io};
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

