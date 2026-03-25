use crate::prompt_assembly::build_employee_collaboration_guidance;
use crate::routing::parse_fallback_chain_targets;
use crate::traits::{ChatEmployeeDirectory, ChatSessionContextRepository, ChatSettingsRepository};
use crate::types::{
    ChatExecutionContext, ChatExecutionGuidance, ChatExecutionPreparationRequest,
    ChatPermissionMode, ChatPreparationRequest, PreparedChatExecution,
    PreparedChatExecutionAssembly, PreparedSessionCreation, SessionCreationRequest,
    SessionExecutionContextSnapshot,
};
use chrono::{Datelike, Duration, Local, NaiveDate};
use serde_json::Value;

pub(crate) fn prepare_session_creation(request: SessionCreationRequest) -> PreparedSessionCreation {
    let permission_mode_storage =
        normalize_permission_mode_for_storage(request.permission_mode.as_deref()).to_string();
    let session_mode_storage =
        normalize_session_mode_for_storage(request.session_mode.as_deref()).to_string();
    let normalized_team_id =
        normalize_team_id_for_storage(&session_mode_storage, request.team_id.as_deref());
    let normalized_title = {
        let title = request.title.unwrap_or_default().trim().to_string();
        if title.is_empty() {
            "New Chat".to_string()
        } else {
            title
        }
    };

    PreparedSessionCreation {
        permission_mode_storage,
        session_mode_storage,
        normalized_team_id,
        normalized_title,
        normalized_work_dir: request.work_dir.unwrap_or_default().trim().to_string(),
        normalized_employee_id: request.employee_id.unwrap_or_default().trim().to_string(),
    }
}

pub(crate) async fn prepare_chat_execution<R: ChatSettingsRepository>(
    repo: &R,
    request: ChatPreparationRequest,
) -> Result<PreparedChatExecution, String> {
    let routing = repo.load_routing_settings().await?;
    let chat_route = repo.load_chat_routing().await?;
    let capability = infer_capability_from_message_parts(
        request.user_message_parts.as_deref().unwrap_or(&[]),
        &request.user_message,
    )
    .to_string();
    let permission_mode_storage =
        normalize_permission_mode_for_storage(request.permission_mode.as_deref()).to_string();
    let session_mode_storage =
        normalize_session_mode_for_storage(request.session_mode.as_deref()).to_string();
    let normalized_team_id =
        normalize_team_id_for_storage(&session_mode_storage, request.team_id.as_deref());
    let permission_label = permission_mode_label(&permission_mode_storage).to_string();

    let (primary_provider_id, primary_model, fallback_targets) = match chat_route {
        Some(route) if route.enabled => (
            Some(route.primary_provider_id),
            Some(route.primary_model),
            parse_fallback_chain_targets(&route.fallback_chain_json),
        ),
        _ => (None, None, Vec::new()),
    };

    Ok(PreparedChatExecution {
        capability,
        permission_mode_storage,
        session_mode_storage,
        normalized_team_id,
        permission_label,
        max_call_depth: routing.max_call_depth,
        node_timeout_seconds: routing.node_timeout_seconds,
        retry_count: routing.retry_count,
        primary_provider_id,
        primary_model,
        fallback_targets,
        default_model_id: repo.resolve_default_model_id().await?,
        default_usable_model_id: repo.resolve_default_usable_model_id().await?,
        execution_context: ChatExecutionContext {
            session_id: String::new(),
            session_mode_storage: "general".to_string(),
            normalized_team_id: String::new(),
            employee_id: String::new(),
            work_dir: String::new(),
            imported_mcp_server_ids: Vec::new(),
        },
    })
}

pub(crate) async fn prepare_execution<R>(
    repo: &R,
    model_id: &str,
    request: &ChatExecutionPreparationRequest,
) -> Result<PreparedChatExecutionAssembly, String>
where
    R: ChatSettingsRepository + ChatSessionContextRepository,
{
    let chat_preparation = prepare_chat_execution(repo, request.clone().into()).await?;
    let execution_context = prepare_execution_context(repo, request).await?;
    let mut guidance_request = request.clone();
    if guidance_request
        .work_dir
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
        && !execution_context.work_dir.trim().is_empty()
    {
        guidance_request.work_dir = Some(execution_context.work_dir.clone());
    }
    if guidance_request.imported_mcp_server_ids.is_empty()
        && !execution_context.imported_mcp_server_ids.is_empty()
    {
        guidance_request.imported_mcp_server_ids =
            execution_context.imported_mcp_server_ids.clone();
    }
    let execution_guidance = prepare_execution_guidance(repo, &guidance_request).await?;
    let route_decisions = crate::routing::prepare_route_decisions(repo, model_id, request).await?;

    Ok(PreparedChatExecutionAssembly {
        chat_preparation,
        execution_context,
        execution_guidance,
        route_decisions,
        employee_collaboration_guidance: None,
    })
}

pub(crate) async fn prepare_execution_with_directory<R, D>(
    repo: &R,
    directory: &D,
    model_id: &str,
    request: &ChatExecutionPreparationRequest,
) -> Result<PreparedChatExecutionAssembly, String>
where
    R: ChatSettingsRepository + ChatSessionContextRepository,
    D: ChatEmployeeDirectory,
{
    let mut prepared = prepare_execution(repo, model_id, request).await?;
    prepared.employee_collaboration_guidance =
        prepare_employee_collaboration_guidance(directory, &prepared.execution_context).await?;
    Ok(prepared)
}

pub(crate) async fn prepare_employee_collaboration_guidance<D: ChatEmployeeDirectory>(
    directory: &D,
    execution_context: &ChatExecutionContext,
) -> Result<Option<String>, String> {
    if execution_context.employee_id.trim().is_empty() {
        return Ok(None);
    }

    let employees = directory.list_collaboration_candidates().await?;
    Ok(build_employee_collaboration_guidance(
        &execution_context.employee_id,
        &employees,
    ))
}

pub(crate) fn resolve_memory_bucket_employee_id<'a>(
    execution_context: &'a ChatExecutionContext,
) -> &'a str {
    execution_context.employee_id.as_str()
}

pub(crate) fn resolve_skill_root_work_dir<'a>(guidance: &'a ChatExecutionGuidance) -> &'a str {
    guidance.effective_work_dir.as_str()
}

pub(crate) fn resolve_executor_work_dir(guidance: &ChatExecutionGuidance) -> Option<String> {
    let work_dir = guidance.effective_work_dir.trim();
    if work_dir.is_empty() {
        None
    } else {
        Some(work_dir.to_string())
    }
}

pub(crate) async fn prepare_execution_context<R: ChatSessionContextRepository>(
    repo: &R,
    request: &ChatExecutionPreparationRequest,
) -> Result<ChatExecutionContext, String> {
    let snapshot = repo
        .load_session_execution_context(request.session_id.as_deref())
        .await?;
    Ok(merge_execution_context(snapshot, request))
}

pub(crate) async fn prepare_execution_guidance<R: ChatSettingsRepository>(
    repo: &R,
    request: &ChatExecutionPreparationRequest,
) -> Result<ChatExecutionGuidance, String> {
    let effective_work_dir = if request.work_dir.as_deref().unwrap_or("").trim().is_empty() {
        repo.load_default_work_dir()
            .await?
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        request.work_dir.as_deref().unwrap_or("").trim().to_string()
    };

    let now = Local::now();
    let today = now.date_naive();
    let tomorrow = today + Duration::days(1);
    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .ok_or_else(|| "failed to resolve month start".to_string())?;
    let next_month_start = if today.month() == 12 {
        NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
    }
    .ok_or_else(|| "failed to resolve next month start".to_string())?;
    let month_end = next_month_start - Duration::days(1);

    Ok(ChatExecutionGuidance {
        effective_work_dir,
        local_timezone: format!("UTC{}", now.format("%:z")),
        local_date: today.format("%Y-%m-%d").to_string(),
        local_tomorrow: tomorrow.format("%Y-%m-%d").to_string(),
        local_month_range: format!(
            "{} ~ {}",
            month_start.format("%Y-%m-%d"),
            month_end.format("%Y-%m-%d")
        ),
    })
}

pub fn normalize_permission_mode_for_storage(permission_mode: Option<&str>) -> &'static str {
    match permission_mode.unwrap_or("").trim() {
        "standard" | "default" | "accept_edits" => "standard",
        "full_access" | "unrestricted" => "full_access",
        _ => "standard",
    }
}

pub fn normalize_session_mode_for_storage(session_mode: Option<&str>) -> &'static str {
    match session_mode.unwrap_or("").trim() {
        "employee_direct" => "employee_direct",
        "team_entry" => "team_entry",
        "general" => "general",
        _ => "general",
    }
}

pub fn normalize_team_id_for_storage(session_mode: &str, team_id: Option<&str>) -> String {
    if session_mode == "team_entry" {
        team_id.unwrap_or("").trim().to_string()
    } else {
        String::new()
    }
}

pub fn parse_permission_mode_for_runtime(permission_mode: &str) -> ChatPermissionMode {
    match permission_mode {
        "standard" | "default" | "accept_edits" => ChatPermissionMode::AcceptEdits,
        "full_access" | "unrestricted" => ChatPermissionMode::Unrestricted,
        _ => ChatPermissionMode::AcceptEdits,
    }
}

pub fn permission_mode_label(permission_mode: &str) -> &'static str {
    match permission_mode {
        "standard" => "标准模式",
        "full_access" => "全自动模式",
        "default" => "标准模式",
        "unrestricted" => "全自动模式",
        _ => "标准模式",
    }
}

pub fn infer_capability_from_user_message(message: &str) -> &'static str {
    let m = message.to_ascii_lowercase();
    if m.contains("识图")
        || m.contains("看图")
        || m.contains("图片理解")
        || m.contains("vision")
        || m.contains("analyze image")
    {
        return "vision";
    }
    if m.contains("生图")
        || m.contains("画图")
        || m.contains("生成图片")
        || m.contains("image generation")
        || m.contains("generate image")
    {
        return "image_gen";
    }
    if m.contains("语音转文字")
        || m.contains("语音识别")
        || m.contains("stt")
        || m.contains("transcribe")
        || m.contains("speech to text")
    {
        return "audio_stt";
    }
    if m.contains("文字转语音")
        || m.contains("tts")
        || m.contains("text to speech")
        || m.contains("语音合成")
    {
        return "audio_tts";
    }
    "chat"
}

pub fn infer_capability_from_message_parts(
    parts: &[Value],
    fallback_message: &str,
) -> &'static str {
    let has_image_part = parts.iter().any(|part| {
        part.get("type")
            .and_then(Value::as_str)
            .map(|part_type| part_type == "image")
            .unwrap_or(false)
    });
    if has_image_part {
        return "vision";
    }
    infer_capability_from_user_message(fallback_message)
}

fn merge_execution_context(
    snapshot: SessionExecutionContextSnapshot,
    request: &ChatExecutionPreparationRequest,
) -> ChatExecutionContext {
    let session_mode_storage = normalize_session_mode_for_storage(
        request
            .session_mode
            .as_deref()
            .or(Some(snapshot.session_mode.as_str())),
    )
    .to_string();

    let normalized_team_id = if request.team_id.as_deref().unwrap_or("").trim().is_empty() {
        normalize_team_id_for_storage(&session_mode_storage, Some(snapshot.team_id.as_str()))
    } else {
        normalize_team_id_for_storage(&session_mode_storage, request.team_id.as_deref())
    };

    let employee_id = if request
        .employee_id
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        snapshot.employee_id.trim().to_string()
    } else {
        request
            .employee_id
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_string()
    };

    let work_dir = if request.work_dir.as_deref().unwrap_or("").trim().is_empty() {
        snapshot.work_dir.trim().to_string()
    } else {
        request.work_dir.as_deref().unwrap_or("").trim().to_string()
    };

    let imported_mcp_server_ids = if request.imported_mcp_server_ids.is_empty() {
        snapshot.imported_mcp_server_ids
    } else {
        request.imported_mcp_server_ids.clone()
    };

    ChatExecutionContext {
        session_id: request
            .session_id
            .as_deref()
            .unwrap_or(snapshot.session_id.as_str())
            .trim()
            .to_string(),
        session_mode_storage,
        normalized_team_id,
        employee_id,
        work_dir,
        imported_mcp_server_ids,
    }
}

impl From<ChatExecutionPreparationRequest> for ChatPreparationRequest {
    fn from(value: ChatExecutionPreparationRequest) -> Self {
        Self {
            user_message: value.user_message,
            user_message_parts: value.user_message_parts,
            permission_mode: value.permission_mode,
            session_mode: value.session_mode,
            team_id: value.team_id,
        }
    }
}

impl From<ChatExecutionPreparationRequest> for ChatExecutionContext {
    fn from(value: ChatExecutionPreparationRequest) -> Self {
        let session_mode_storage =
            normalize_session_mode_for_storage(value.session_mode.as_deref()).to_string();
        Self {
            session_id: value.session_id.unwrap_or_default().trim().to_string(),
            session_mode_storage: session_mode_storage.clone(),
            normalized_team_id: normalize_team_id_for_storage(
                &session_mode_storage,
                value.team_id.as_deref(),
            ),
            employee_id: value.employee_id.unwrap_or_default().trim().to_string(),
            work_dir: value.work_dir.unwrap_or_default().trim().to_string(),
            imported_mcp_server_ids: value.imported_mcp_server_ids,
        }
    }
}
