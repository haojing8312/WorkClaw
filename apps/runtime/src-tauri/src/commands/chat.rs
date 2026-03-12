use super::chat_compaction;
use super::chat_policy;
use super::chat_runtime_io as chat_io;
use super::chat_send_message_flow::{self, PrepareSendMessageParams};
use super::chat_session_io;
use super::skills::DbState;
use crate::agent::tools::search_providers::cache::SearchCache;
use crate::agent::tools::AskUserResponder;
use crate::agent::AgentExecutor;
use crate::session_journal::{
    SessionJournalStateHandle, SessionJournalStore,
};
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

#[tauri::command]
pub async fn get_messages(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    chat_session_io::get_messages_with_pool(&db.0, &session_id).await
}

#[tauri::command]
pub async fn list_sessions(db: State<'_, DbState>) -> Result<Vec<serde_json::Value>, String> {
    chat_session_io::list_sessions_with_pool(&db.0, chat_policy::permission_mode_label_for_display)
        .await
}

#[tauri::command]
pub async fn get_sessions(
    skill_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    // 兼容旧调用，skill_id 不再参与过滤
    let _ = &skill_id;
    list_sessions(db).await
}

#[cfg(test)]
mod tests {
    use super::{build_group_orchestrator_report_preview, chat_policy, chat_runtime_io};
    use crate::agent::permissions::PermissionMode;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn normalize_permission_mode_defaults_to_standard() {
        assert_eq!(chat_policy::normalize_permission_mode_for_storage(None), "standard");
        assert_eq!(chat_policy::normalize_permission_mode_for_storage(Some("")), "standard");
        assert_eq!(
            chat_policy::normalize_permission_mode_for_storage(Some("invalid")),
            "standard"
        );
    }

    #[test]
    fn normalize_permission_mode_maps_legacy_values_to_modern_storage() {
        assert_eq!(
            chat_policy::normalize_permission_mode_for_storage(Some("standard")),
            "standard"
        );
        assert_eq!(
            chat_policy::normalize_permission_mode_for_storage(Some("full_access")),
            "full_access"
        );
        assert_eq!(
            chat_policy::normalize_permission_mode_for_storage(Some("default")),
            "standard"
        );
        assert_eq!(
            chat_policy::normalize_permission_mode_for_storage(Some("accept_edits")),
            "standard"
        );
        assert_eq!(
            chat_policy::normalize_permission_mode_for_storage(Some("unrestricted")),
            "full_access"
        );
    }

    #[test]
    fn normalize_session_mode_defaults_to_general() {
        assert_eq!(chat_policy::normalize_session_mode_for_storage(None), "general");
        assert_eq!(chat_policy::normalize_session_mode_for_storage(Some("")), "general");
        assert_eq!(
            chat_policy::normalize_session_mode_for_storage(Some("invalid")),
            "general"
        );
    }

    #[test]
    fn normalize_session_mode_keeps_supported_values() {
        assert_eq!(
            chat_policy::normalize_session_mode_for_storage(Some("general")),
            "general"
        );
        assert_eq!(
            chat_policy::normalize_session_mode_for_storage(Some("employee_direct")),
            "employee_direct"
        );
        assert_eq!(
            chat_policy::normalize_session_mode_for_storage(Some("team_entry")),
            "team_entry"
        );
    }

    #[test]
    fn normalize_team_id_only_keeps_team_entry_values() {
        assert_eq!(
            chat_policy::normalize_team_id_for_storage("general", Some("group-1")),
            ""
        );
        assert_eq!(
            chat_policy::normalize_team_id_for_storage("employee_direct", Some("group-1")),
            ""
        );
        assert_eq!(
            chat_policy::normalize_team_id_for_storage("team_entry", Some(" group-1 ")),
            "group-1"
        );
    }

    #[test]
    fn parse_permission_mode_for_runtime_defaults_to_standard_behavior() {
        assert_eq!(
            chat_policy::parse_permission_mode_for_runtime(""),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            chat_policy::parse_permission_mode_for_runtime("invalid"),
            PermissionMode::AcceptEdits
        );
    }

    #[test]
    fn parse_permission_mode_for_runtime_supports_modern_and_legacy_values() {
        assert_eq!(
            chat_policy::parse_permission_mode_for_runtime("standard"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            chat_policy::parse_permission_mode_for_runtime("full_access"),
            PermissionMode::Unrestricted
        );
        assert_eq!(
            chat_policy::parse_permission_mode_for_runtime("default"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            chat_policy::parse_permission_mode_for_runtime("accept_edits"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            chat_policy::parse_permission_mode_for_runtime("unrestricted"),
            PermissionMode::Unrestricted
        );
    }

    #[test]
    fn permission_mode_label_is_user_friendly() {
        assert_eq!(chat_policy::permission_mode_label_for_display("standard"), "标准模式");
        assert_eq!(
            chat_policy::permission_mode_label_for_display("full_access"),
            "全自动模式"
        );
        assert_eq!(
            chat_policy::permission_mode_label_for_display("accept_edits"),
            "标准模式"
        );
        assert_eq!(chat_policy::permission_mode_label_for_display("default"), "标准模式");
        assert_eq!(
            chat_policy::permission_mode_label_for_display("unrestricted"),
            "全自动模式"
        );
    }

    #[test]
    fn supported_protocols_are_openai_and_anthropic_only() {
        assert!(chat_policy::is_supported_protocol("openai"));
        assert!(chat_policy::is_supported_protocol("anthropic"));
        assert!(!chat_policy::is_supported_protocol("gemini"));
        assert!(!chat_policy::is_supported_protocol(""));
    }

    #[test]
    fn parse_fallback_chain_targets_handles_json_array() {
        let raw = r#"[{"provider_id":"p1","model":"m1"},{"provider_id":"p2","model":"m2"}]"#;
        let parsed = chat_policy::parse_fallback_chain_targets(raw);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "p1");
        assert_eq!(parsed[0].1, "m1");
        assert_eq!(parsed[1].0, "p2");
        assert_eq!(parsed[1].1, "m2");
    }

    #[test]
    fn classify_model_route_error_detects_auth() {
        let kind = chat_policy::classify_model_route_error("Unauthorized: invalid_api_key");
        assert_eq!(kind, chat_policy::ModelRouteErrorKind::Auth);
        assert!(!chat_policy::should_retry_same_candidate(kind));
    }

    #[test]
    fn classify_model_route_error_detects_billing() {
        let kind = chat_policy::classify_model_route_error("insufficient_balance: account balance too low");
        assert_eq!(kind, chat_policy::ModelRouteErrorKind::Billing);
        assert_eq!(chat_policy::model_route_error_kind_key(kind), "billing");
        assert!(!chat_policy::should_retry_same_candidate(kind));
    }

    #[test]
    fn classify_model_route_error_detects_retryable_kinds() {
        let rate = chat_policy::classify_model_route_error("429 Too Many Requests");
        let timeout = chat_policy::classify_model_route_error("request timeout while calling provider");
        let network = chat_policy::classify_model_route_error("network connection reset");
        assert_eq!(rate, chat_policy::ModelRouteErrorKind::RateLimit);
        assert_eq!(timeout, chat_policy::ModelRouteErrorKind::Timeout);
        assert_eq!(network, chat_policy::ModelRouteErrorKind::Network);
        assert!(chat_policy::should_retry_same_candidate(rate));
        assert!(chat_policy::should_retry_same_candidate(timeout));
        assert!(chat_policy::should_retry_same_candidate(network));
    }

    #[test]
    fn classify_model_route_error_detects_transport_send_failures_as_network() {
        let kind = chat_policy::classify_model_route_error(
            "error sending request for url (https://api.minimax.io/anthropic/v1/messages)",
        );
        assert_eq!(kind, chat_policy::ModelRouteErrorKind::Network);
    }

    #[test]
    fn retry_budget_for_error_guarantees_one_retry_for_network() {
        assert_eq!(chat_policy::retry_budget_for_error(chat_policy::ModelRouteErrorKind::Network, 0), 1);
        assert_eq!(chat_policy::retry_budget_for_error(chat_policy::ModelRouteErrorKind::Network, 2), 2);
        assert_eq!(chat_policy::retry_budget_for_error(chat_policy::ModelRouteErrorKind::RateLimit, 0), 0);
    }

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(chat_policy::retry_backoff_ms(chat_policy::ModelRouteErrorKind::Network, 0), 400);
        assert_eq!(chat_policy::retry_backoff_ms(chat_policy::ModelRouteErrorKind::Network, 2), 1600);
        assert_eq!(chat_policy::retry_backoff_ms(chat_policy::ModelRouteErrorKind::RateLimit, 3), 5000);
        assert_eq!(chat_policy::retry_backoff_ms(chat_policy::ModelRouteErrorKind::Unknown, 1), 0);
    }

    #[test]
    fn infer_capability_from_user_message_detects_modalities() {
        assert_eq!(chat_policy::infer_capability_from_user_message("请帮我识图"), "vision");
        assert_eq!(
            chat_policy::infer_capability_from_user_message("帮我生成图片"),
            "image_gen"
        );
        assert_eq!(
            chat_policy::infer_capability_from_user_message("这段音频做语音转文字"),
            "audio_stt"
        );
        assert_eq!(
            chat_policy::infer_capability_from_user_message("这段文案做文字转语音"),
            "audio_tts"
        );
        assert_eq!(chat_policy::infer_capability_from_user_message("解释这个报错"), "chat");
    }

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

#[tauri::command]
pub async fn update_session_workspace(
    session_id: String,
    workspace: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    chat_session_io::update_session_workspace_with_pool(&db.0, &session_id, &workspace).await
}

#[tauri::command]
pub async fn delete_session(session_id: String, db: State<'_, DbState>) -> Result<(), String> {
    chat_session_io::delete_session_with_pool(&db.0, &session_id).await
}

/// 搜索会话标题和消息内容
#[tauri::command]
pub async fn search_sessions_global(
    query: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    chat_session_io::search_sessions_global_with_pool(&db.0, &query).await
}

#[tauri::command]
pub async fn search_sessions(
    skill_id: String,
    query: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    // 兼容旧调用，skill_id 不再参与过滤
    let _ = &skill_id;
    search_sessions_global(query, db).await
}

/// 将会话消息导出为 Markdown 字符串
#[tauri::command]
pub async fn export_session(
    session_id: String,
    db: State<'_, DbState>,
    journal: State<'_, SessionJournalStateHandle>,
) -> Result<String, String> {
    chat_session_io::export_session_markdown_with_pool(&db.0, &session_id, Some(journal.0.as_ref()))
        .await
}

pub async fn export_session_markdown_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    journal: Option<&SessionJournalStore>,
) -> Result<String, String> {
    chat_session_io::export_session_markdown_with_pool(pool, session_id, journal).await
}

#[cfg(test)]
mod session_source_tests {
    use super::chat_session_io::resolve_im_session_source;

    #[test]
    fn resolve_im_session_source_maps_wecom_and_feishu_labels() {
        assert_eq!(
            resolve_im_session_source(Some("wecom")),
            ("wecom".to_string(), "企业微信".to_string())
        );
        assert_eq!(
            resolve_im_session_source(Some("feishu")),
            ("feishu".to_string(), "飞书".to_string())
        );
        assert_eq!(
            resolve_im_session_source(Some("")),
            ("local".to_string(), String::new())
        );
        assert_eq!(
            resolve_im_session_source(None),
            ("local".to_string(), String::new())
        );
    }
}

/// 写入导出文件
#[tauri::command]
pub async fn write_export_file(path: String, content: String) -> Result<(), String> {
    chat_session_io::write_export_file_to_path(&path, &content)
}

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

