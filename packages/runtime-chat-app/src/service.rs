use crate::traits::{
    ChatEmployeeDirectory, ChatSessionContextRepository, ChatSettingsRepository,
};
use crate::types::{
    ChatEmployeeSnapshot, ChatExecutionContext, ChatExecutionGuidance,
    ChatExecutionPreparationRequest, ChatPermissionMode, ChatPreparationRequest,
    ModelRouteErrorKind, PreparedChatExecution, PreparedChatExecutionAssembly,
    PreparedSessionCreation, SessionCreationRequest, SessionExecutionContextSnapshot,
    SessionModelSnapshot,
};
use serde_json::Value;

pub struct ChatPreparationService;
pub struct ChatExecutionPreparationService;

impl ChatPreparationService {
    pub fn new() -> Self {
        Self
    }

    pub fn prepare_session_creation(
        &self,
        request: SessionCreationRequest,
    ) -> PreparedSessionCreation {
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

    pub async fn prepare_chat_execution<R: ChatSettingsRepository>(
        &self,
        repo: &R,
        request: ChatPreparationRequest,
    ) -> Result<PreparedChatExecution, String> {
        let routing = repo.load_routing_settings().await?;
        let chat_route = repo.load_chat_routing().await?;
        let capability = infer_capability_from_user_message(&request.user_message).to_string();
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

    pub async fn prepare_route_candidates<R: ChatSettingsRepository>(
        &self,
        repo: &R,
        model_id: &str,
        request: &ChatPreparationRequest,
    ) -> Result<crate::types::PreparedRouteCandidates, String> {
        let session_model = resolve_session_model_with_fallback(repo, model_id).await?;
        let requested_capability = infer_capability_from_user_message(&request.user_message);

        let mut retry_count_per_candidate = 0usize;
        let mut route_policy = repo
            .load_route_policy(requested_capability)
            .await?
            .filter(|policy| policy.enabled);
        if route_policy.is_none() && requested_capability != "chat" {
            route_policy = repo
                .load_route_policy("chat")
                .await?
                .filter(|policy| policy.enabled);
        }

        let mut candidates = Vec::new();
        if let Some(policy) = route_policy {
            retry_count_per_candidate = policy.retry_count.clamp(0, 3) as usize;
            let mut provider_targets =
                vec![(policy.primary_provider_id, policy.primary_model.clone())];
            provider_targets.extend(parse_fallback_chain_targets(&policy.fallback_chain_json));

            for (provider_id, preferred_model) in provider_targets {
                if let Some(provider) = repo.get_provider_connection(&provider_id).await? {
                    if is_supported_protocol(&provider.protocol_type)
                        && !provider.api_key.trim().is_empty()
                    {
                        candidates.push(crate::types::PreparedRouteCandidate {
                            protocol_type: provider.protocol_type,
                            base_url: provider.base_url,
                            model_name: if preferred_model.trim().is_empty() {
                                session_model.model_name.clone()
                            } else {
                                preferred_model
                            },
                            api_key: provider.api_key,
                        });
                    }
                }
            }
        }

        if !session_model.api_key.trim().is_empty() {
            candidates.push(crate::types::PreparedRouteCandidate {
                protocol_type: session_model.api_format,
                base_url: session_model.base_url,
                model_name: session_model.model_name,
                api_key: session_model.api_key,
            });
        }

        Ok(crate::types::PreparedRouteCandidates {
            candidates,
            retry_count_per_candidate,
        })
    }
}

impl ChatExecutionPreparationService {
    pub fn new() -> Self {
        Self
    }

    pub async fn prepare_execution<R>(
        &self,
        repo: &R,
        model_id: &str,
        request: &ChatExecutionPreparationRequest,
    ) -> Result<PreparedChatExecutionAssembly, String>
    where
        R: ChatSettingsRepository + ChatSessionContextRepository,
    {
        let chat_preparation = ChatPreparationService::new()
            .prepare_chat_execution(repo, request.clone().into())
            .await?;
        let execution_context = self.prepare_execution_context(repo, request).await?;
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
        let execution_guidance = self
            .prepare_execution_guidance(repo, &guidance_request)
            .await?;
        let route_decisions = self.prepare_route_decisions(repo, model_id, request).await?;

        Ok(PreparedChatExecutionAssembly {
            chat_preparation,
            execution_context,
            execution_guidance,
            route_decisions,
            employee_collaboration_guidance: None,
        })
    }

    pub async fn prepare_execution_with_directory<R, D>(
        &self,
        repo: &R,
        directory: &D,
        model_id: &str,
        request: &ChatExecutionPreparationRequest,
    ) -> Result<PreparedChatExecutionAssembly, String>
    where
        R: ChatSettingsRepository + ChatSessionContextRepository,
        D: ChatEmployeeDirectory,
    {
        let mut prepared = self.prepare_execution(repo, model_id, request).await?;
        prepared.employee_collaboration_guidance = self
            .prepare_employee_collaboration_guidance(directory, &prepared.execution_context)
            .await?;
        Ok(prepared)
    }

    pub async fn prepare_employee_collaboration_guidance<D: ChatEmployeeDirectory>(
        &self,
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

    pub fn resolve_memory_bucket_employee_id<'a>(
        &self,
        execution_context: &'a ChatExecutionContext,
    ) -> &'a str {
        execution_context.employee_id.as_str()
    }

    pub fn resolve_skill_root_work_dir<'a>(
        &self,
        guidance: &'a ChatExecutionGuidance,
    ) -> &'a str {
        guidance.effective_work_dir.as_str()
    }

    pub fn resolve_executor_work_dir(
        &self,
        guidance: &ChatExecutionGuidance,
    ) -> Option<String> {
        let work_dir = guidance.effective_work_dir.trim();
        if work_dir.is_empty() {
            None
        } else {
            Some(work_dir.to_string())
        }
    }

    pub fn resolve_imported_mcp_guidance<'a>(
        &self,
        guidance: &'a ChatExecutionGuidance,
    ) -> Option<&'a str> {
        guidance.imported_mcp_guidance.as_deref()
    }

    pub async fn prepare_execution_context<R: ChatSessionContextRepository>(
        &self,
        repo: &R,
        request: &ChatExecutionPreparationRequest,
    ) -> Result<ChatExecutionContext, String> {
        let snapshot = repo
            .load_session_execution_context(request.session_id.as_deref())
            .await?;
        Ok(merge_execution_context(snapshot, request))
    }

    pub async fn prepare_execution_guidance<R: ChatSettingsRepository>(
        &self,
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

        let imported_mcp_guidance = repo
            .load_imported_mcp_guidance(&request.imported_mcp_server_ids)
            .await?;

        Ok(ChatExecutionGuidance {
            effective_work_dir,
            imported_mcp_guidance,
        })
    }

    pub async fn prepare_route_decisions<R: ChatSettingsRepository>(
        &self,
        repo: &R,
        model_id: &str,
        request: &ChatExecutionPreparationRequest,
    ) -> Result<crate::types::PreparedRouteCandidates, String> {
        let session_model = resolve_session_model_with_fallback(repo, model_id).await?;
        let requested_capability = request
            .requested_capability
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| infer_capability_from_user_message(&request.user_message));

        let mut retry_count_per_candidate = 0usize;
        let mut route_policy = repo
            .load_route_policy(requested_capability)
            .await?
            .filter(|policy| policy.enabled);
        if route_policy.is_none() && requested_capability != "chat" {
            route_policy = repo
                .load_route_policy("chat")
                .await?
                .filter(|policy| policy.enabled);
        }

        let mut candidates = Vec::new();
        if let Some(policy) = route_policy {
            retry_count_per_candidate = policy.retry_count.clamp(0, 3) as usize;
            let mut provider_targets =
                vec![(policy.primary_provider_id, policy.primary_model.clone())];
            provider_targets.extend(parse_fallback_chain_targets(&policy.fallback_chain_json));

            for (provider_id, preferred_model) in provider_targets {
                if let Some(provider) = repo.get_provider_connection(&provider_id).await? {
                    if is_supported_protocol(&provider.protocol_type)
                        && !provider.api_key.trim().is_empty()
                    {
                        candidates.push(crate::types::PreparedRouteCandidate {
                            protocol_type: provider.protocol_type,
                            base_url: provider.base_url,
                            model_name: if preferred_model.trim().is_empty() {
                                session_model.model_name.clone()
                            } else {
                                preferred_model
                            },
                            api_key: provider.api_key,
                        });
                    }
                }
            }
        }

        if !session_model.api_key.trim().is_empty() {
            candidates.push(crate::types::PreparedRouteCandidate {
                protocol_type: session_model.api_format,
                base_url: session_model.base_url,
                model_name: session_model.model_name,
                api_key: session_model.api_key,
            });
        }

        Ok(crate::types::PreparedRouteCandidates {
            candidates,
            retry_count_per_candidate,
        })
    }
}

async fn resolve_session_model_with_fallback<R: ChatSettingsRepository>(
    repo: &R,
    model_id: &str,
) -> Result<SessionModelSnapshot, String> {
    match repo.load_session_model(model_id).await {
        Ok(model) => Ok(model),
        Err(primary_err) => {
            let normalized_error = primary_err.to_ascii_lowercase();
            let is_missing_model = primary_err.contains("模型配置不存在")
                || normalized_error.contains("no rows returned")
                || normalized_error.contains("rownotfound");
            if !is_missing_model {
                return Err(primary_err);
            }
            let fallback_model_id = repo
                .resolve_default_usable_model_id()
                .await?
                .filter(|fallback_id| fallback_id != model_id)
                .ok_or_else(|| primary_err.clone())?;
            repo.load_session_model(&fallback_model_id)
                .await
                .map_err(|_| primary_err)
        }
    }
}

impl From<ChatExecutionPreparationRequest> for ChatPreparationRequest {
    fn from(value: ChatExecutionPreparationRequest) -> Self {
        Self {
            user_message: value.user_message,
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

pub fn classify_model_route_error(error_message: &str) -> ModelRouteErrorKind {
    let lower = error_message.to_ascii_lowercase();
    if lower.contains("api key")
        || lower.contains("unauthorized")
        || lower.contains("invalid_api_key")
        || lower.contains("authentication")
        || lower.contains("permission denied")
        || lower.contains("forbidden")
    {
        return ModelRouteErrorKind::Auth;
    }
    if lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("429")
        || lower.contains("quota")
    {
        return ModelRouteErrorKind::RateLimit;
    }
    if lower.contains("timeout") || lower.contains("timed out") || lower.contains("deadline") {
        return ModelRouteErrorKind::Timeout;
    }
    if lower.contains("connection")
        || lower.contains("network")
        || lower.contains("dns")
        || lower.contains("connect")
        || lower.contains("socket")
        || lower.contains("error sending request for url")
        || lower.contains("sending request for url")
    {
        return ModelRouteErrorKind::Network;
    }
    ModelRouteErrorKind::Unknown
}

pub fn should_retry_same_candidate(kind: ModelRouteErrorKind) -> bool {
    matches!(
        kind,
        ModelRouteErrorKind::RateLimit
            | ModelRouteErrorKind::Timeout
            | ModelRouteErrorKind::Network
    )
}

pub fn retry_budget_for_error(kind: ModelRouteErrorKind, configured_retry_count: usize) -> usize {
    if kind == ModelRouteErrorKind::Network {
        configured_retry_count.max(1)
    } else {
        configured_retry_count
    }
}

pub fn retry_backoff_ms(kind: ModelRouteErrorKind, attempt_idx: usize) -> u64 {
    let base_ms = match kind {
        ModelRouteErrorKind::RateLimit => 1200u64,
        ModelRouteErrorKind::Timeout => 700u64,
        ModelRouteErrorKind::Network => 400u64,
        _ => 0u64,
    };
    if base_ms == 0 {
        return 0;
    }
    let exp = attempt_idx.min(3) as u32;
    base_ms.saturating_mul(1u64 << exp).min(5000)
}

pub fn parse_fallback_chain_targets(raw: &str) -> Vec<(String, String)> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|item| {
            let provider_id = item.get("provider_id")?.as_str()?.to_string();
            let model = item
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            Some((provider_id, model))
        })
        .collect()
}

fn is_supported_protocol(protocol: &str) -> bool {
    matches!(protocol, "openai" | "anthropic")
}

pub fn compose_system_prompt(
    base_prompt: &str,
    tool_names: &str,
    model_name: &str,
    max_iter: usize,
    guidance: &ChatExecutionGuidance,
    employee_collaboration_guidance: Option<&str>,
    imported_external_mcp_guidance: Option<&str>,
    memory_content: Option<&str>,
) -> String {
    let mut system_prompt = if guidance.effective_work_dir.trim().is_empty() {
        format!(
            "{}\n\n---\n运行环境:\n- 可用工具: {}\n- 模型: {}\n- 最大迭代次数: {}",
            base_prompt, tool_names, model_name, max_iter,
        )
    } else {
        format!(
            "{}\n\n---\n运行环境:\n- 工作目录: {}\n- 可用工具: {}\n- 模型: {}\n- 最大迭代次数: {}\n\n注意: 所有文件操作必须限制在工作目录范围内。",
            base_prompt, guidance.effective_work_dir, tool_names, model_name, max_iter,
        )
    };

    if let Some(collaboration) = employee_collaboration_guidance.filter(|value| !value.trim().is_empty()) {
        system_prompt = format!("{}\n\n---\n{}", system_prompt, collaboration);
    }
    if let Some(external_mcp_guidance) =
        imported_external_mcp_guidance.filter(|value| !value.trim().is_empty())
    {
        system_prompt = format!("{}\n\n---\n{}", system_prompt, external_mcp_guidance);
    }
    if let Some(memory_content) = memory_content.filter(|value| !value.trim().is_empty()) {
        system_prompt = format!("{}\n\n---\n持久内存:\n{}", system_prompt, memory_content);
    }

    system_prompt
}

fn employee_matches_session(session_employee_id: &str, employee: &ChatEmployeeSnapshot) -> bool {
    let target = session_employee_id.trim();
    if target.is_empty() {
        return false;
    }
    target.eq_ignore_ascii_case(employee.employee_id.trim())
        || target.eq_ignore_ascii_case(employee.role_id.trim())
        || target.eq_ignore_ascii_case(employee.id.trim())
}

fn build_employee_collaboration_guidance(
    session_employee_id: &str,
    employees: &[ChatEmployeeSnapshot],
) -> Option<String> {
    let current = employees
        .iter()
        .find(|employee| employee_matches_session(session_employee_id, employee))?;
    let collaborators = employees
        .iter()
        .filter(|employee| employee.enabled && employee.id != current.id)
        .collect::<Vec<_>>();
    if collaborators.is_empty() {
        return None;
    }

    let mut lines = vec![
        "员工协作协议:".to_string(),
        format!(
            "- 当前员工: {} (employee_id={})",
            current.name, current.employee_id
        ),
        "- 可委托员工清单:".to_string(),
    ];
    for employee in collaborators {
        lines.push(format!(
            "  - {} (employee_id={}, role_id={}, feishu_open_id={})",
            employee.name,
            employee.employee_id,
            employee.role_id,
            if employee.feishu_open_id.trim().is_empty() {
                "-"
            } else {
                employee.feishu_open_id.trim()
            }
        ));
    }
    lines.push(
        "- 当任务需要专项能力时，优先调用 task 工具委托，并在参数中填入 delegate_role_id / delegate_role_name。".to_string(),
    );
    lines.push(
        "- task.prompt 必须写清目标、输入上下文、输出格式、验收标准。收到子任务结果后再统一汇总回复用户。".to_string(),
    );
    lines.push(
        "- 如果在 IM/飞书场景需要转交某员工，先在回复中明确“已转交给谁”，再执行委托，不得只给笼统答复。".to_string(),
    );

    Some(lines.join("\n"))
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

    let employee_id = if request.employee_id.as_deref().unwrap_or("").trim().is_empty() {
        snapshot.employee_id.trim().to_string()
    } else {
        request.employee_id.as_deref().unwrap_or("").trim().to_string()
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
