use super::models::load_routing_settings_from_pool;
use super::runtime_preferences::resolve_default_work_dir_with_pool;
use super::skills::DbState;
use crate::agent::compactor;
use crate::agent::executor::estimate_tokens;
use crate::agent::permissions::PermissionMode;
use crate::agent::tools::search_providers::cache::SearchCache;
use crate::agent::tools::{
    browser_tools::register_browser_tools, AskUserResponder, AskUserTool, BashKillTool,
    BashOutputTool, BashTool, ClawhubRecommendTool, ClawhubSearchTool, CompactTool,
    EmployeeManageTool, MemoryTool, ProcessManager, SkillInvokeTool, TaskTool, WebSearchTool,
};
use crate::agent::AgentExecutor;
use chrono::Utc;
use serde_json::{json, Value};
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

fn normalize_permission_mode_for_storage(permission_mode: Option<&str>) -> &'static str {
    match permission_mode.unwrap_or("").trim() {
        "default" => "default",
        "unrestricted" => "unrestricted",
        "accept_edits" => "accept_edits",
        _ => "accept_edits",
    }
}

fn parse_permission_mode(permission_mode: &str) -> PermissionMode {
    match permission_mode {
        "default" => PermissionMode::Default,
        "unrestricted" => PermissionMode::Unrestricted,
        _ => PermissionMode::AcceptEdits,
    }
}

fn permission_mode_label_for_display(permission_mode: &str) -> &'static str {
    match permission_mode {
        "default" => "谨慎模式",
        "unrestricted" => "全自动模式（高风险）",
        _ => "推荐模式",
    }
}

fn is_supported_protocol(protocol: &str) -> bool {
    matches!(protocol, "openai" | "anthropic")
}

fn infer_capability_from_user_message(message: &str) -> &'static str {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelRouteErrorKind {
    Auth,
    RateLimit,
    Timeout,
    Network,
    Unknown,
}

fn classify_model_route_error(error_message: &str) -> ModelRouteErrorKind {
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

fn should_retry_same_candidate(kind: ModelRouteErrorKind) -> bool {
    matches!(
        kind,
        ModelRouteErrorKind::RateLimit
            | ModelRouteErrorKind::Timeout
            | ModelRouteErrorKind::Network
    )
}

fn retry_budget_for_error(kind: ModelRouteErrorKind, configured_retry_count: usize) -> usize {
    if kind == ModelRouteErrorKind::Network {
        configured_retry_count.max(1)
    } else {
        configured_retry_count
    }
}

fn retry_backoff_ms(kind: ModelRouteErrorKind, attempt_idx: usize) -> u64 {
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

fn parse_fallback_chain_targets(raw: &str) -> Vec<(String, String)> {
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

fn extract_skill_prompt_from_decrypted_files(
    files: &std::collections::HashMap<String, Vec<u8>>,
) -> Option<String> {
    for key in ["SKILL.md", "skill.md"] {
        if let Some(bytes) = files.get(key) {
            return Some(String::from_utf8_lossy(bytes).to_string());
        }
    }

    let candidate = files
        .iter()
        .find(|(path, _)| path.eq_ignore_ascii_case("SKILL.md"))
        .or_else(|| {
            files.iter().find(|(path, _)| {
                path.rsplit('/')
                    .next()
                    .map(|name| name.eq_ignore_ascii_case("skill.md"))
                    .unwrap_or(false)
            })
        });

    candidate.map(|(_, bytes)| String::from_utf8_lossy(bytes).to_string())
}

fn read_local_skill_prompt(pack_path: &str) -> Option<String> {
    let base = std::path::Path::new(pack_path);

    for file_name in ["SKILL.md", "skill.md"] {
        let candidate = base.join(file_name);
        if let Ok(content) = std::fs::read_to_string(&candidate) {
            return Some(content);
        }
    }

    let entries = std::fs::read_dir(base).ok()?;
    for entry in entries.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case("skill.md")
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                return Some(content);
            }
        }
    }

    None
}

#[derive(serde::Serialize, Clone)]
struct StreamToken {
    session_id: String,
    token: String,
    done: bool,
    #[serde(default)]
    sub_agent: bool,
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
    permission_mode: Option<String>,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let permission_mode = normalize_permission_mode_for_storage(permission_mode.as_deref());
    let normalized_work_dir = work_dir.unwrap_or_default().trim().to_string();
    let normalized_employee_id = employee_id.unwrap_or_default().trim().to_string();
    let resolved_work_dir = if normalized_work_dir.is_empty() {
        resolve_default_work_dir_with_pool(&db.0).await?
    } else {
        normalized_work_dir
    };
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&session_id)
    .bind(&skill_id)
    .bind("New Chat")
    .bind(&now)
    .bind(&model_id)
    .bind(permission_mode)
    .bind(&resolved_work_dir)
    .bind(&normalized_employee_id)
    .execute(&db.0)
    .await
    .map_err(|e| e.to_string())?;
    Ok(session_id)
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    session_id: String,
    user_message: String,
    db: State<'_, DbState>,
    agent_executor: State<'_, Arc<AgentExecutor>>,
    cancel_flag: State<'_, CancelFlagState>,
) -> Result<(), String> {
    // 重置取消标志
    cancel_flag.0.store(false, Ordering::SeqCst);
    let cancel_flag_clone = cancel_flag.0.clone();

    // 保存用户消息
    let msg_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&msg_id)
    .bind(&session_id)
    .bind("user")
    .bind(&user_message)
    .bind(&now)
    .execute(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    // 如果是第一条消息，用消息前 20 个字符更新会话标题
    let msg_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM messages WHERE session_id = ?")
        .bind(&session_id)
        .fetch_one(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    if msg_count.0 <= 1 {
        let title: String = user_message.chars().take(20).collect();
        sqlx::query("UPDATE sessions SET title = ? WHERE id = ?")
            .bind(&title)
            .bind(&session_id)
            .execute(&db.0)
            .await
            .map_err(|e| e.to_string())?;
    }

    // 加载会话信息（含权限模式和工作目录）
    let (skill_id, model_id, perm_str, work_dir, session_employee_id) = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT skill_id, model_id, permission_mode, COALESCE(work_dir, ''), COALESCE(employee_id, '') FROM sessions WHERE id = ?"
    )
    .bind(&session_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| format!("会话不存在 (session_id={session_id}): {e}"))?;

    let permission_mode = parse_permission_mode(&perm_str);
    let routing_settings = load_routing_settings_from_pool(&db.0).await?;

    // 加载 Skill 信息（含 pack_path 和 source_type，用 COALESCE 兼容旧数据）
    let (manifest_json, username, pack_path, source_type) = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT manifest, username, pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?"
    )
    .bind(&skill_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| format!("Skill 不存在 (skill_id={skill_id}): {e}"))?;

    // 根据 source_type 决定如何读取 SKILL.md 内容
    let raw_prompt = if source_type == "builtin" {
        // 内置 Skill：从独立的 builtin-skills 目录读取，便于后续迭代
        crate::builtin_skills::builtin_skill_markdown(&skill_id)
            .unwrap_or(crate::builtin_skills::builtin_general_skill_markdown())
            .to_string()
    } else if source_type == "local" {
        // 本地 Skill：直接从目录读取 SKILL.md
        read_local_skill_prompt(&pack_path).unwrap_or_else(|| {
            // 读取失败时回退到 manifest 描述
            serde_json::from_str::<skillpack_rs::SkillManifest>(&manifest_json)
                .map(|m| m.description)
                .unwrap_or_default()
        })
    } else {
        // 加密 Skill：重新解包获取 SKILL.md
        match skillpack_rs::verify_and_unpack(&pack_path, &username) {
            Ok(unpacked) => extract_skill_prompt_from_decrypted_files(&unpacked.files)
                .unwrap_or_else(|| {
                    serde_json::from_str::<skillpack_rs::SkillManifest>(&manifest_json)
                        .map(|m| m.description)
                        .unwrap_or_default()
                }),
            Err(_) => {
                // 解包失败时回退到 manifest 描述
                let manifest: skillpack_rs::SkillManifest =
                    serde_json::from_str(&manifest_json).map_err(|e| e.to_string())?;
                manifest.description
            }
        }
    };

    // 加载消息历史
    let history = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    // 加载会话模型配置（含 api_key）作为最后兜底候选
    let (session_api_format, session_base_url, session_model_name, session_api_key) =
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, model_name, api_key FROM model_configs WHERE id = ?",
        )
        .bind(&model_id)
        .fetch_one(&db.0)
        .await
        .map_err(|e| format!("模型配置不存在 (model_id={model_id}): {e}"))?;

    // 构建候选链：routing policy primary/fallbacks + 会话 model 配置兜底
    let mut route_candidates: Vec<(String, String, String, String)> = Vec::new();

    let mut per_candidate_retry_count: usize = 0;
    let requested_capability = infer_capability_from_user_message(&user_message);
    if let Ok(Some((primary_provider_id, primary_model, fallback_chain_json, policy_retry_count))) = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT primary_provider_id, primary_model, fallback_chain_json, retry_count FROM routing_policies WHERE capability = ? AND enabled = 1 LIMIT 1"
    )
    .bind(requested_capability)
    .fetch_optional(&db.0)
    .await
    {
        per_candidate_retry_count = policy_retry_count.clamp(0, 3) as usize;
        let mut candidates: Vec<(String, String)> = vec![(primary_provider_id, primary_model)];
        candidates.extend(parse_fallback_chain_targets(&fallback_chain_json));

        for (provider_id, preferred_model) in candidates {
            if let Ok(Some((protocol_type, routed_base_url, routed_api_key))) = sqlx::query_as::<_, (String, String, String)>(
                "SELECT protocol_type, base_url, api_key_encrypted FROM provider_configs WHERE id = ? AND enabled = 1 LIMIT 1"
            )
            .bind(&provider_id)
            .fetch_optional(&db.0)
            .await
            {
                if is_supported_protocol(&protocol_type) && !routed_api_key.trim().is_empty() {
                    let candidate_model = if preferred_model.trim().is_empty() {
                        session_model_name.clone()
                    } else {
                        preferred_model
                    };
                    route_candidates.push((
                        protocol_type,
                        routed_base_url,
                        candidate_model,
                        routed_api_key,
                    ));
                }
            }
        }
    } else if requested_capability != "chat" {
        // 能力未配置时退回 chat 路由
        if let Ok(Some((primary_provider_id, primary_model, fallback_chain_json, policy_retry_count))) = sqlx::query_as::<_, (String, String, String, i64)>(
            "SELECT primary_provider_id, primary_model, fallback_chain_json, retry_count FROM routing_policies WHERE capability = 'chat' AND enabled = 1 LIMIT 1"
        )
        .fetch_optional(&db.0)
        .await
        {
            per_candidate_retry_count = policy_retry_count.clamp(0, 3) as usize;
            let mut candidates: Vec<(String, String)> = vec![(primary_provider_id, primary_model)];
            candidates.extend(parse_fallback_chain_targets(&fallback_chain_json));

            for (provider_id, preferred_model) in candidates {
                if let Ok(Some((protocol_type, routed_base_url, routed_api_key))) = sqlx::query_as::<_, (String, String, String)>(
                    "SELECT protocol_type, base_url, api_key_encrypted FROM provider_configs WHERE id = ? AND enabled = 1 LIMIT 1"
                )
                .bind(&provider_id)
                .fetch_optional(&db.0)
                .await
                {
                    if is_supported_protocol(&protocol_type) && !routed_api_key.trim().is_empty() {
                        let candidate_model = if preferred_model.trim().is_empty() {
                            session_model_name.clone()
                        } else {
                            preferred_model
                        };
                        route_candidates.push((
                            protocol_type,
                            routed_base_url,
                            candidate_model,
                            routed_api_key,
                        ));
                    }
                }
            }
        }
    }

    if !session_api_key.trim().is_empty() {
        route_candidates.push((
            session_api_format.clone(),
            session_base_url.clone(),
            session_model_name.clone(),
            session_api_key.clone(),
        ));
    }

    if route_candidates.is_empty() {
        return Err(format!(
            "模型 API Key 为空，请在设置中重新配置 (model_id={model_id})"
        ));
    }

    // 去重，避免 fallback 与会话配置重复
    route_candidates.dedup();
    eprintln!(
        "[routing] capability={}, candidates={}, retry_per_candidate={}",
        requested_capability,
        route_candidates.len(),
        per_candidate_retry_count
    );

    // 当前回合默认使用首个候选的 api_format 做消息重建
    let (api_format, base_url, model_name, api_key) = route_candidates[0].clone();

    // 重建 LLM 历史消息：将 JSON 包装的 assistant content 还原为 tool_use/tool_result 消息对
    let messages: Vec<Value> = history
        .iter()
        .flat_map(|(role, content)| {
            if role == "assistant" {
                if let Ok(parsed) = serde_json::from_str::<Value>(content) {
                    if parsed.get("text").is_some() && parsed.get("items").is_some() {
                        return reconstruct_llm_messages(&parsed, &api_format);
                    }
                }
            }
            vec![json!({"role": role, "content": content})]
        })
        .collect();

    // 解析 Skill 元数据（frontmatter + system prompt）
    let skill_config = crate::agent::skill_config::SkillConfig::parse(&raw_prompt);

    // 确定工具白名单
    let allowed_tools = skill_config.allowed_tools.clone();

    let max_iter = skill_config.max_iterations.unwrap_or(10);

    // 动态注册运行时工具（在计算 tool_names 之前完成，确保列表完整）

    // L3: 注册后台进程管理工具
    let process_manager = Arc::new(ProcessManager::new());
    agent_executor
        .registry()
        .register(Arc::new(BashOutputTool::new(Arc::clone(&process_manager))));
    agent_executor
        .registry()
        .register(Arc::new(BashKillTool::new(Arc::clone(&process_manager))));
    // 替换默认 bash 工具为支持后台模式的版本
    agent_executor.registry().unregister("bash");
    agent_executor
        .registry()
        .register(Arc::new(BashTool::with_process_manager(Arc::clone(
            &process_manager,
        ))));

    // L4: 注册浏览器自动化工具（通过 Sidecar 桥接）
    register_browser_tools(agent_executor.registry(), "http://localhost:8765");

    let task_tool = TaskTool::new(
        agent_executor.registry_arc(),
        api_format.clone(),
        base_url.clone(),
        api_key.clone(),
        model_name.clone(),
    )
    .with_app_handle(app.clone(), session_id.clone());
    agent_executor.registry().register(Arc::new(task_tool));
    agent_executor
        .registry()
        .register(Arc::new(ClawhubSearchTool));
    agent_executor
        .registry()
        .register(Arc::new(ClawhubRecommendTool));
    agent_executor
        .registry()
        .register(Arc::new(EmployeeManageTool::new(db.0.clone())));

    // 注册 WebSearch 工具（从 DB 加载搜索 Provider 配置，使用全局缓存）
    {
        use crate::agent::tools::search_providers::create_provider;

        let search_cache = app.state::<SearchCacheState>().0.clone();

        let search_config = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, api_key, model_name FROM model_configs WHERE api_format LIKE 'search_%' AND is_default = 1 LIMIT 1"
        )
        .fetch_optional(&db.0)
        .await
        .map_err(|e| e.to_string())?;

        if let Some((search_api_format, search_base_url, search_api_key, search_model_name)) =
            search_config
        {
            match create_provider(
                &search_api_format,
                &search_base_url,
                &search_api_key,
                &search_model_name,
            ) {
                Ok(provider) => {
                    let web_search = WebSearchTool::with_provider(provider, search_cache);
                    agent_executor.registry().register(Arc::new(web_search));
                }
                Err(e) => {
                    eprintln!("[search] 创建搜索 Provider 失败: {}", e);
                }
            }
        }
        // 无搜索配置时不注册 web_search 工具，Agent 不调用搜索
    }

    // 注册 Memory 工具（基于 Skill ID 的持久存储）
    let app_data_dir = app.path().app_data_dir().unwrap_or_default();
    let memory_dir = build_memory_dir_for_session(&app_data_dir, &skill_id, &session_employee_id);
    let memory_tool = MemoryTool::new(memory_dir.clone());
    agent_executor.registry().register(Arc::new(memory_tool));

    // 注册 Skill 调用工具（支持 Skill 之间按需互调）
    let mut skill_roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(wd) = tool_ctx_from_work_dir(&work_dir) {
        skill_roots.push(wd.join(".claude").join("skills"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        skill_roots.push(cwd.join(".claude").join("skills"));
    }
    if source_type == "local" {
        let skill_path = std::path::Path::new(&pack_path);
        if let Some(parent) = skill_path.parent() {
            skill_roots.push(parent.to_path_buf());
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        skill_roots.push(
            std::path::PathBuf::from(profile)
                .join(".claude")
                .join("skills"),
        );
    }
    skill_roots.sort();
    skill_roots.dedup();
    let skill_tool = SkillInvokeTool::new(session_id.clone(), skill_roots)
        .with_max_depth(routing_settings.max_call_depth);
    agent_executor.registry().register(Arc::new(skill_tool));

    // 注册 Compact 工具（手动触发上下文压缩）
    let compact_tool = CompactTool::new();
    agent_executor.registry().register(Arc::new(compact_tool));

    // 注册 AskUser 工具（使用全局响应通道，在 lib.rs 中创建）
    let ask_user_responder = app.state::<AskUserState>().0.clone();
    let ask_user_tool = AskUserTool::new(app.clone(), session_id.clone(), ask_user_responder);
    agent_executor.registry().register(Arc::new(ask_user_tool));

    // 获取工具名称列表（在所有工具注册完成后计算，确保列表完整）
    let tool_names = match &allowed_tools {
        Some(whitelist) => whitelist.join(", "),
        None => agent_executor
            .registry()
            .get_tool_definitions()
            .iter()
            .filter_map(|t| t["name"].as_str().map(String::from))
            .collect::<Vec<_>>()
            .join(", "),
    };

    // 构建完整 system prompt（含运行环境信息）
    let system_prompt = if work_dir.is_empty() {
        format!(
            "{}\n\n---\n运行环境:\n- 可用工具: {}\n- 模型: {}\n- 最大迭代次数: {}",
            skill_config.system_prompt, tool_names, model_name, max_iter,
        )
    } else {
        format!(
            "{}\n\n---\n运行环境:\n- 工作目录: {}\n- 可用工具: {}\n- 模型: {}\n- 最大迭代次数: {}\n\n注意: 所有文件操作必须限制在工作目录范围内。",
            skill_config.system_prompt, work_dir, tool_names, model_name, max_iter,
        )
    };

    // 如果存在 MEMORY.md，注入到 system prompt
    let memory_content = {
        let memory_file = memory_dir.join("MEMORY.md");
        if memory_file.exists() {
            std::fs::read_to_string(&memory_file).unwrap_or_default()
        } else {
            String::new()
        }
    };
    let system_prompt = if memory_content.is_empty() {
        system_prompt
    } else {
        format!("{}\n\n---\n持久内存:\n{}", system_prompt, memory_content)
    };

    // 使用全局工具确认通道（在 lib.rs 中创建）
    let tool_confirm_responder = app.state::<ToolConfirmState>().0.clone();

    // 始终走 Agent 模式；失败时按候选链重试
    let mut final_messages_opt: Option<Vec<Value>> = None;
    let mut last_error: Option<String> = None;
    for (candidate_api_format, candidate_base_url, candidate_model_name, candidate_api_key) in
        &route_candidates
    {
        let mut attempt_idx = 0usize;
        loop {
            let app_clone = app.clone();
            let session_id_clone = session_id.clone();
            let attempt = agent_executor
                .execute_turn(
                    candidate_api_format,
                    candidate_base_url,
                    candidate_api_key,
                    candidate_model_name,
                    &system_prompt,
                    messages.clone(),
                    move |token: String| {
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
                    Some(&app),
                    Some(&session_id),
                    allowed_tools.as_deref(),
                    permission_mode,
                    Some(tool_confirm_responder.clone()),
                    if work_dir.is_empty() {
                        None
                    } else {
                        Some(work_dir.clone())
                    },
                    skill_config.max_iterations,
                    Some(cancel_flag_clone.clone()),
                    Some(routing_settings.node_timeout_seconds),
                    Some(routing_settings.retry_count),
                )
                .await;

            match attempt {
                Ok(messages_out) => {
                    let _ = sqlx::query(
                        "INSERT INTO route_attempt_logs (id, session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, success, error_message, created_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, '', ?)",
                    )
                    .bind(Uuid::new_v4().to_string())
                    .bind(&session_id)
                    .bind(requested_capability)
                    .bind(candidate_api_format)
                    .bind(candidate_model_name)
                    .bind((attempt_idx + 1) as i64)
                    .bind(attempt_idx as i64)
                    .bind("ok")
                    .bind(Utc::now().to_rfc3339())
                    .execute(&db.0)
                    .await;
                    final_messages_opt = Some(messages_out);
                    break;
                }
                Err(err) => {
                    let err_text = err.to_string();
                    let kind = classify_model_route_error(&err_text);
                    let kind_text = match kind {
                        ModelRouteErrorKind::Auth => "auth",
                        ModelRouteErrorKind::RateLimit => "rate_limit",
                        ModelRouteErrorKind::Timeout => "timeout",
                        ModelRouteErrorKind::Network => "network",
                        ModelRouteErrorKind::Unknown => "unknown",
                    };
                    let _ = sqlx::query(
                        "INSERT INTO route_attempt_logs (id, session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, success, error_message, created_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
                    )
                    .bind(Uuid::new_v4().to_string())
                    .bind(&session_id)
                    .bind(requested_capability)
                    .bind(candidate_api_format)
                    .bind(candidate_model_name)
                    .bind((attempt_idx + 1) as i64)
                    .bind(attempt_idx as i64)
                    .bind(kind_text)
                    .bind(err_text.clone())
                    .bind(Utc::now().to_rfc3339())
                    .execute(&db.0)
                    .await;
                    last_error = Some(err_text.clone());
                    eprintln!(
                        "[routing] 候选模型执行失败: format={}, model={}, attempt={}, kind={:?}, err={}",
                        candidate_api_format,
                        candidate_model_name,
                        attempt_idx + 1,
                        kind,
                        err_text
                    );

                    let retry_budget = retry_budget_for_error(kind, per_candidate_retry_count);
                    if should_retry_same_candidate(kind) && attempt_idx < retry_budget {
                        let backoff_ms = retry_backoff_ms(kind, attempt_idx);
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

    let final_messages = final_messages_opt
        .ok_or_else(|| last_error.unwrap_or_else(|| "所有候选模型执行失败".to_string()))?;

    // 发送结束事件
    let _ = app.emit(
        "stream-token",
        StreamToken {
            session_id: session_id.clone(),
            token: String::new(),
            done: true,
            sub_agent: false,
        },
    );

    // 从新消息中按顺序提取有序项（文字和工具调用交替排列）
    let new_messages: Vec<&Value> = final_messages.iter().skip(history.len()).collect();

    let mut ordered_items: Vec<Value> = Vec::new();
    let mut final_text = String::new();

    for msg in &new_messages {
        let role = msg["role"].as_str().unwrap_or("");

        if role == "assistant" {
            // Anthropic 格式：content 数组含 text blocks 和 tool_use blocks
            if let Some(content_arr) = msg["content"].as_array() {
                for block in content_arr {
                    match block["type"].as_str() {
                        Some("text") => {
                            // 捕获 Anthropic assistant content 中的伴随文本
                            let text = block["text"].as_str().unwrap_or("");
                            if !text.is_empty() {
                                ordered_items.push(json!({"type": "text", "content": text}));
                            }
                        }
                        Some("tool_use") => {
                            // 使用前端期望的嵌套 toolCall 格式
                            ordered_items.push(json!({
                                "type": "tool_call",
                                "toolCall": {
                                    "id": block["id"],
                                    "name": block["name"],
                                    "input": block["input"],
                                    "status": "completed"
                                }
                            }));
                        }
                        _ => {}
                    }
                }
            }
            // Anthropic 格式：纯文本 content
            else if let Some(text) = msg["content"].as_str() {
                if !text.is_empty() {
                    final_text = text.to_string();
                    ordered_items.push(json!({
                        "type": "text",
                        "content": text
                    }));
                }
            }
            // OpenAI 格式：assistant 含 tool_calls 数组
            if let Some(tool_calls_arr) = msg["tool_calls"].as_array() {
                // 捕获 OpenAI 伴随文本
                if let Some(text) = msg["content"].as_str() {
                    if !text.is_empty() {
                        ordered_items.push(json!({"type": "text", "content": text}));
                    }
                }
                for tc in tool_calls_arr {
                    let func = &tc["function"];
                    let input_val =
                        serde_json::from_str::<Value>(func["arguments"].as_str().unwrap_or("{}"))
                            .unwrap_or(json!({}));
                    ordered_items.push(json!({
                        "type": "tool_call",
                        "toolCall": {
                            "id": tc["id"],
                            "name": func["name"],
                            "input": input_val,
                            "status": "completed"
                        }
                    }));
                }
            }
        }

        // Anthropic 格式：user 消息含 tool_result blocks → 匹配对应的工具调用
        if role == "user" {
            if let Some(content_arr) = msg["content"].as_array() {
                for block in content_arr {
                    if block["type"].as_str() == Some("tool_result") {
                        let tool_use_id = block["tool_use_id"].as_str().unwrap_or("");
                        let output = block["content"].as_str().unwrap_or("");
                        // 反向查找匹配的 tool_call 并填充 output
                        for item in ordered_items.iter_mut().rev() {
                            if item["type"].as_str() == Some("tool_call") {
                                let tc = &item["toolCall"];
                                if tc["id"].as_str() == Some(tool_use_id)
                                    && tc.get("output").map_or(true, |v| v.is_null())
                                {
                                    item["toolCall"]["output"] = Value::String(output.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // OpenAI 格式：tool 角色消息 → 匹配对应的工具调用
        if role == "tool" {
            let tool_call_id = msg["tool_call_id"].as_str().unwrap_or("");
            let output = msg["content"].as_str().unwrap_or("");
            for item in ordered_items.iter_mut().rev() {
                if item["type"].as_str() == Some("tool_call") {
                    let tc = &item["toolCall"];
                    if tc["id"].as_str() == Some(tool_call_id)
                        && tc.get("output").map_or(true, |v| v.is_null())
                    {
                        item["toolCall"]["output"] = Value::String(output.to_string());
                        break;
                    }
                }
            }
        }
    }

    // 组装最终 content：包含有序 items 列表
    let has_tool_calls = ordered_items
        .iter()
        .any(|i| i["type"].as_str() == Some("tool_call"));
    let content = if has_tool_calls {
        serde_json::to_string(&json!({
            "text": final_text,
            "items": ordered_items,
        }))
        .unwrap_or(final_text.clone())
    } else {
        final_text.clone()
    };

    // 只有存在 assistant 回复时才保存
    if !final_text.is_empty() || has_tool_calls {
        let msg_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&msg_id)
        .bind(&session_id)
        .bind("assistant")
        .bind(&content)
        .bind(&now)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn sanitize_memory_bucket_component(raw: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for ch in raw.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_sep = false;
            continue;
        }
        if !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }
    let normalized = out.trim_matches('_').to_string();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

fn build_memory_dir_for_session(
    app_data_dir: &std::path::Path,
    skill_id: &str,
    employee_id: &str,
) -> std::path::PathBuf {
    let root = app_data_dir.join("memory");
    if employee_id.trim().is_empty() {
        // Keep legacy layout for non-employee sessions.
        return root.join(skill_id);
    }
    let employee_bucket = sanitize_memory_bucket_component(employee_id, "employee");
    root.join("employees")
        .join(employee_bucket)
        .join("skills")
        .join(skill_id)
}

fn tool_ctx_from_work_dir(work_dir: &str) -> Option<std::path::PathBuf> {
    if work_dir.trim().is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(work_dir))
    }
}

/// 从 JSON 包装的 assistant content 重建 LLM 可理解的消息序列
///
/// 将 `{"text":"最终回复","items":[...]}` 格式还原为：
/// 1. assistant 消息（含 tool_use blocks + 伴随文本）
/// 2. user 消息（含 tool_result blocks）
/// 3. assistant 消息（最终文本回复）
fn reconstruct_llm_messages(parsed: &Value, api_format: &str) -> Vec<Value> {
    let final_text = parsed["text"].as_str().unwrap_or("");
    let items = match parsed["items"].as_array() {
        Some(arr) => arr,
        None => return vec![json!({"role": "assistant", "content": final_text})],
    };

    let mut result = Vec::new();

    // 收集工具调用及其结果
    let mut tool_calls: Vec<(&Value, Option<&str>)> = Vec::new(); // (item, output)
    let mut companion_texts: Vec<String> = Vec::new();

    for item in items {
        match item["type"].as_str() {
            Some("text") => {
                let text = item["content"].as_str().unwrap_or("");
                if !text.is_empty() {
                    companion_texts.push(text.to_string());
                }
            }
            Some("tool_call") => {
                // 兼容新旧格式：嵌套 toolCall 或扁平结构
                let tc = if item.get("toolCall").is_some() {
                    &item["toolCall"]
                } else {
                    item
                };
                let output = tc["output"].as_str();
                tool_calls.push((tc, output));
            }
            _ => {}
        }
    }

    if !tool_calls.is_empty() {
        if api_format == "anthropic" {
            // 构建 assistant 消息：text blocks + tool_use blocks
            let mut content_blocks: Vec<Value> = Vec::new();
            for text in &companion_texts {
                content_blocks.push(json!({"type": "text", "text": text}));
            }
            for (tc, _) in &tool_calls {
                content_blocks.push(json!({
                    "type": "tool_use",
                    "id": tc["id"],
                    "name": tc["name"],
                    "input": tc["input"],
                }));
            }
            result.push(json!({"role": "assistant", "content": content_blocks}));

            // 构建 user 消息：tool_result blocks
            let tool_results: Vec<Value> = tool_calls
                .iter()
                .map(|(tc, output)| {
                    json!({
                        "type": "tool_result",
                        "tool_use_id": tc["id"],
                        "content": output.unwrap_or("[已执行]"),
                    })
                })
                .collect();
            result.push(json!({"role": "user", "content": tool_results}));
        } else {
            // OpenAI 格式：assistant 消息含 tool_calls 数组
            let companion = companion_texts.join("\n");
            let content_val = if companion.is_empty() {
                Value::Null
            } else {
                Value::String(companion)
            };
            let tc_arr: Vec<Value> = tool_calls
                .iter()
                .map(|(tc, _)| {
                    json!({
                        "id": tc["id"],
                        "type": "function",
                        "function": {
                            "name": tc["name"],
                            "arguments": serde_json::to_string(&tc["input"]).unwrap_or_default(),
                        }
                    })
                })
                .collect();
            result.push(json!({"role": "assistant", "content": content_val, "tool_calls": tc_arr}));

            // 每个工具结果独立的 tool 消息
            for (tc, output) in &tool_calls {
                result.push(json!({
                    "role": "tool",
                    "tool_call_id": tc["id"],
                    "content": output.unwrap_or("[已执行]"),
                }));
            }
        }
    }

    // 最终文本回复
    if !final_text.is_empty() {
        result.push(json!({"role": "assistant", "content": final_text}));
    }

    // 如果没有任何有效内容，返回空消息避免丢失
    if result.is_empty() {
        result.push(json!({"role": "assistant", "content": ""}));
    }

    result
}

/// 将旧格式扁平 tool_call items 转换为前端期望的嵌套 toolCall 格式
///
/// 旧格式：`{"type":"tool_call","id":"...","name":"...","input":{...},"output":"...","status":"completed"}`
/// 新格式：`{"type":"tool_call","toolCall":{"id":"...","name":"...","input":{...},"output":"...","status":"completed"}}`
fn normalize_stream_items(items: &Value) -> Value {
    if let Some(arr) = items.as_array() {
        Value::Array(
            arr.iter()
                .map(|item| {
                    if item["type"].as_str() == Some("tool_call") && item.get("toolCall").is_none()
                    {
                        // 旧格式：扁平结构 → 包装为嵌套格式
                        json!({
                            "type": "tool_call",
                            "toolCall": {
                                "id": item["id"],
                                "name": item["name"],
                                "input": item["input"],
                                "output": item["output"],
                                "status": item["status"]
                            }
                        })
                    } else {
                        item.clone()
                    }
                })
                .collect(),
        )
    } else {
        items.clone()
    }
}

#[tauri::command]
pub async fn get_messages(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT role, content, created_at FROM messages WHERE session_id = ? ORDER BY created_at ASC"
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|(role, content, created_at)| {
            // 对 assistant 消息尝试解析结构化 content
            if role == "assistant" {
                if let Ok(parsed) = serde_json::from_str::<Value>(content) {
                    if let Some(text) = parsed.get("text") {
                        // 包含有序 items 列表
                        if let Some(items) = parsed.get("items") {
                            // 向后兼容：将旧格式扁平 tool_call 转换为嵌套 toolCall 格式
                            let normalized = normalize_stream_items(items);
                            return json!({
                                "role": role,
                                "content": text,
                                "created_at": created_at,
                                "streamItems": normalized,
                            });
                        }
                        // 旧格式：包含 tool_calls 列表（向后兼容）
                        let tool_calls = parsed.get("tool_calls").cloned().unwrap_or(Value::Null);
                        return json!({
                            "role": role,
                            "content": text,
                            "created_at": created_at,
                            "tool_calls": tool_calls,
                        });
                    }
                }
            }
            // 其他情况直接返回原始 content
            json!({"role": role, "content": content, "created_at": created_at})
        })
        .collect())
}

#[tauri::command]
pub async fn get_sessions(
    skill_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, title, created_at, model_id, COALESCE(work_dir, ''), COALESCE(permission_mode, 'accept_edits') FROM sessions WHERE skill_id = ? ORDER BY created_at DESC"
    )
    .bind(&skill_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(
            |(id, title, created_at, model_id, work_dir, permission_mode)| {
                json!({
                    "id": id,
                    "title": title,
                    "created_at": created_at,
                    "model_id": model_id,
                    "work_dir": work_dir,
                    "permission_mode": permission_mode,
                    "permission_mode_label": permission_mode_label_for_display(permission_mode),
                })
            },
        )
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{
        classify_model_route_error, extract_skill_prompt_from_decrypted_files,
        infer_capability_from_user_message, is_supported_protocol,
        normalize_permission_mode_for_storage, parse_fallback_chain_targets, parse_permission_mode,
        permission_mode_label_for_display, retry_backoff_ms, retry_budget_for_error,
        should_retry_same_candidate, ModelRouteErrorKind,
    };
    use crate::agent::permissions::PermissionMode;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn normalize_permission_mode_defaults_to_accept_edits() {
        assert_eq!(normalize_permission_mode_for_storage(None), "accept_edits");
        assert_eq!(
            normalize_permission_mode_for_storage(Some("")),
            "accept_edits"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("invalid")),
            "accept_edits"
        );
    }

    #[test]
    fn normalize_permission_mode_keeps_supported_values() {
        assert_eq!(
            normalize_permission_mode_for_storage(Some("default")),
            "default"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("accept_edits")),
            "accept_edits"
        );
        assert_eq!(
            normalize_permission_mode_for_storage(Some("unrestricted")),
            "unrestricted"
        );
    }

    #[test]
    fn parse_permission_mode_defaults_to_accept_edits() {
        assert_eq!(parse_permission_mode(""), PermissionMode::AcceptEdits);
        assert_eq!(
            parse_permission_mode("invalid"),
            PermissionMode::AcceptEdits
        );
    }

    #[test]
    fn parse_permission_mode_keeps_supported_values() {
        assert_eq!(parse_permission_mode("default"), PermissionMode::Default);
        assert_eq!(
            parse_permission_mode("accept_edits"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(
            parse_permission_mode("unrestricted"),
            PermissionMode::Unrestricted
        );
    }

    #[test]
    fn permission_mode_label_is_user_friendly() {
        assert_eq!(
            permission_mode_label_for_display("accept_edits"),
            "推荐模式"
        );
        assert_eq!(permission_mode_label_for_display("default"), "谨慎模式");
        assert_eq!(
            permission_mode_label_for_display("unrestricted"),
            "全自动模式（高风险）"
        );
    }

    #[test]
    fn supported_protocols_are_openai_and_anthropic_only() {
        assert!(is_supported_protocol("openai"));
        assert!(is_supported_protocol("anthropic"));
        assert!(!is_supported_protocol("gemini"));
        assert!(!is_supported_protocol(""));
    }

    #[test]
    fn parse_fallback_chain_targets_handles_json_array() {
        let raw = r#"[{"provider_id":"p1","model":"m1"},{"provider_id":"p2","model":"m2"}]"#;
        let parsed = parse_fallback_chain_targets(raw);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "p1");
        assert_eq!(parsed[0].1, "m1");
        assert_eq!(parsed[1].0, "p2");
        assert_eq!(parsed[1].1, "m2");
    }

    #[test]
    fn classify_model_route_error_detects_auth() {
        let kind = classify_model_route_error("Unauthorized: invalid_api_key");
        assert_eq!(kind, ModelRouteErrorKind::Auth);
        assert!(!should_retry_same_candidate(kind));
    }

    #[test]
    fn classify_model_route_error_detects_retryable_kinds() {
        let rate = classify_model_route_error("429 Too Many Requests");
        let timeout = classify_model_route_error("request timeout while calling provider");
        let network = classify_model_route_error("network connection reset");
        assert_eq!(rate, ModelRouteErrorKind::RateLimit);
        assert_eq!(timeout, ModelRouteErrorKind::Timeout);
        assert_eq!(network, ModelRouteErrorKind::Network);
        assert!(should_retry_same_candidate(rate));
        assert!(should_retry_same_candidate(timeout));
        assert!(should_retry_same_candidate(network));
    }

    #[test]
    fn classify_model_route_error_detects_transport_send_failures_as_network() {
        let kind = classify_model_route_error(
            "error sending request for url (https://api.minimax.io/anthropic/v1/messages)",
        );
        assert_eq!(kind, ModelRouteErrorKind::Network);
    }

    #[test]
    fn retry_budget_for_error_guarantees_one_retry_for_network() {
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Network, 0), 1);
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::Network, 2), 2);
        assert_eq!(retry_budget_for_error(ModelRouteErrorKind::RateLimit, 0), 0);
    }

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Network, 0), 400);
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Network, 2), 1600);
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::RateLimit, 3), 5000);
        assert_eq!(retry_backoff_ms(ModelRouteErrorKind::Unknown, 1), 0);
    }

    #[test]
    fn infer_capability_from_user_message_detects_modalities() {
        assert_eq!(infer_capability_from_user_message("请帮我识图"), "vision");
        assert_eq!(
            infer_capability_from_user_message("帮我生成图片"),
            "image_gen"
        );
        assert_eq!(
            infer_capability_from_user_message("这段音频做语音转文字"),
            "audio_stt"
        );
        assert_eq!(
            infer_capability_from_user_message("这段文案做文字转语音"),
            "audio_tts"
        );
        assert_eq!(infer_capability_from_user_message("解释这个报错"), "chat");
    }

    #[test]
    fn build_memory_dir_for_session_keeps_legacy_skill_bucket_without_employee() {
        let base = Path::new("C:/workclaw/app-data");
        let dir = super::build_memory_dir_for_session(base, "builtin-general", "");
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
        let dir = super::build_memory_dir_for_session(base, "builtin-general", "Sales Lead/华东");
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
        let content = extract_skill_prompt_from_decrypted_files(&files);
        assert_eq!(content.as_deref(), Some("# lowercase skill"));
    }
}

#[tauri::command]
pub async fn update_session_workspace(
    session_id: String,
    workspace: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET work_dir = ? WHERE id = ?")
        .bind(&workspace)
        .bind(&session_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_session(session_id: String, db: State<'_, DbState>) -> Result<(), String> {
    // 先删除该会话下的所有消息
    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(&session_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    // 再删除会话本身
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&session_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 搜索会话标题和消息内容
#[tauri::command]
pub async fn search_sessions(
    skill_id: String,
    query: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    let pattern = format!("%{}%", query);
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT DISTINCT s.id, s.title, s.created_at, s.model_id, COALESCE(s.work_dir, '')
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
         WHERE s.skill_id = ? AND (s.title LIKE ? OR m.content LIKE ?)
         ORDER BY s.created_at DESC",
    )
    .bind(&skill_id)
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.iter().map(|(id, title, created_at, model_id, work_dir)| {
        json!({"id": id, "title": title, "created_at": created_at, "model_id": model_id, "work_dir": work_dir})
    }).collect())
}

/// 将会话消息导出为 Markdown 字符串
#[tauri::command]
pub async fn export_session(session_id: String, db: State<'_, DbState>) -> Result<String, String> {
    let (title,): (String,) = sqlx::query_as("SELECT title FROM sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_one(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    let messages = sqlx::query_as::<_, (String, String, String)>(
        "SELECT role, content, created_at FROM messages WHERE session_id = ? ORDER BY created_at ASC"
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let mut md = format!("# {}\n\n", title);
    for (role, content, created_at) in &messages {
        let label = if role == "user" { "用户" } else { "助手" };
        md.push_str(&format!(
            "## {} ({})\n\n{}\n\n---\n\n",
            label, created_at, content
        ));
    }
    Ok(md)
}

/// 写入导出文件
#[tauri::command]
pub async fn write_export_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, &content).map_err(|e| format!("写入失败: {}", e))
}

/// 用户回答 AskUser 工具的问题
#[tauri::command]
pub async fn answer_user_question(
    answer: String,
    ask_user_state: State<'_, AskUserState>,
) -> Result<(), String> {
    let guard = ask_user_state
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?;

    if let Some(sender) = guard.as_ref() {
        sender
            .send(answer)
            .map_err(|e| format!("发送响应失败: {}", e))?;
        Ok(())
    } else {
        Err("没有等待中的用户问题".to_string())
    }
}

/// 用户确认或拒绝工具执行
#[tauri::command]
pub async fn confirm_tool_execution(
    confirmed: bool,
    tool_confirm_state: State<'_, ToolConfirmState>,
) -> Result<(), String> {
    let guard = tool_confirm_state
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?;
    if let Some(sender) = guard.as_ref() {
        sender
            .send(confirmed)
            .map_err(|e| format!("发送确认失败: {}", e))?;
        Ok(())
    } else {
        Err("没有等待中的工具确认请求".to_string())
    }
}

/// 取消正在执行的 Agent
#[tauri::command]
pub async fn cancel_agent(cancel_flag: State<'_, CancelFlagState>) -> Result<(), String> {
    cancel_flag.0.store(true, Ordering::SeqCst);
    eprintln!("[agent] 收到取消信号");
    Ok(())
}

/// 压缩结果
#[derive(serde::Serialize)]
pub struct CompactionResult {
    original_tokens: usize,
    new_tokens: usize,
    summary: String,
}

/// 手动触发上下文压缩
#[tauri::command]
pub async fn compact_context(
    session_id: String,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<CompactionResult, String> {
    // 1. 获取会话消息
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let messages: Vec<Value> = rows
        .iter()
        .map(|(role, content)| json!({ "role": role, "content": content }))
        .collect();

    // 2. 估算原始 token 数
    let original_tokens = estimate_tokens(&messages);

    // 3. 获取模型配置
    let (model_id,): (String,) = sqlx::query_as("SELECT model_id FROM sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_one(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    let (api_format, base_url, api_key, model_name) =
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, api_key, model_name FROM model_configs WHERE id = ?",
        )
        .bind(&model_id)
        .fetch_one(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    // 4. 创建 transcript 目录
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let transcript_dir = app_data_dir.join("transcripts");
    std::fs::create_dir_all(&transcript_dir).map_err(|e| e.to_string())?;

    // 5. 保存完整记录并压缩
    let transcript_path = compactor::save_transcript(&transcript_dir, &session_id, &messages)
        .map_err(|e| e.to_string())?;

    let compacted = compactor::auto_compact(
        &api_format,
        &base_url,
        &api_key,
        &model_name,
        &messages,
        &transcript_path.to_string_lossy(),
    )
    .await
    .map_err(|e| e.to_string())?;

    // 6. 更新会话消息（删除旧消息，插入压缩后的消息）
    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(&session_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    for msg in &compacted {
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(msg["role"].as_str().unwrap_or("user"))
        .bind(msg["content"].as_str().unwrap_or(""))
        .bind(&now)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    }

    // 7. 返回结果
    let new_tokens = estimate_tokens(&compacted);
    let summary = compacted
        .iter()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
        .to_string();

    Ok(CompactionResult {
        original_tokens,
        new_tokens,
        summary,
    })
}
