use super::scenario::EvalWorkspaceFile;
use super::{CapabilityMapping, EvalScenario, LocalEvalConfig, ModelProviderProfile};
use crate::agent::runtime::{
    RunRegistry, RunRegistryState, RuntimeObservability, RuntimeObservabilityState,
    SearchCacheState, SessionAdmissionGate, SessionAdmissionGateState, ToolConfirmResponder,
};
use crate::agent::tools::new_responder;
use crate::agent::tools::search_providers::cache::SearchCache;
use crate::agent::{AgentExecutor, ToolRegistry};
use crate::commands::chat::{
    create_session, send_message, AskUserPendingSessionState, AskUserState, CancelFlagState,
    SendMessagePart, SendMessageRequest, ToolConfirmState,
};
use crate::commands::chat_session_commands::export_session_markdown_with_pool;
use crate::commands::chat_session_io::get_messages_with_pool;
use crate::commands::models::RouteAttemptLog;
use crate::commands::models::{
    save_model_config_with_pool, set_default_model_with_pool, ModelConfig,
};
use crate::commands::session_runs::{
    export_session_run_trace_with_pool, list_session_runs_with_pool, SessionRunProjection,
};
use crate::commands::skills::{import_local_skills_to_pool, DbState};
use crate::runtime_paths::RuntimePaths;
use crate::session_journal::{SessionJournalState, SessionJournalStateHandle, SessionJournalStore};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::test::{mock_context, noop_assets};
use tauri::Manager;
use tauri::Wry;

pub struct RealAgentEvalRunner {
    app: tauri::App<Wry>,
    pool: sqlx::SqlitePool,
    journal: Arc<SessionJournalStore>,
    cancel_flag: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct HeadlessEvalRun {
    pub scenario_id: String,
    pub capability_id: String,
    pub session_id: String,
    pub skill_id: String,
    pub model_id: String,
    pub work_dir: PathBuf,
    pub imported_skill_count: usize,
    pub missing_mcp: Vec<String>,
    pub execution_error: Option<String>,
    pub session_runs: Vec<SessionRunProjection>,
    pub route_attempt_logs: Vec<RouteAttemptLog>,
    pub trace: Option<crate::agent::runtime::SessionRunTrace>,
    pub messages: Vec<Value>,
    pub session_markdown: String,
    pub journal_state: SessionJournalState,
    pub final_output: String,
}

impl RealAgentEvalRunner {
    pub async fn new(config: &LocalEvalConfig) -> Result<Self, String> {
        let output_root = PathBuf::from(&config.artifacts.output_dir);
        std::fs::create_dir_all(&output_root).map_err(|e| format!("创建评测输出目录失败: {e}"))?;

        let app = tauri::Builder::default()
            .build(mock_context(noop_assets()))
            .map_err(|e| format!("创建 headless runtime app 失败: {e}"))?;

        let runtime_paths = RuntimePaths::new(output_root.join("runtime-state"));
        let pool = crate::db::init_db_at_runtime_paths(&runtime_paths)
            .await
            .map_err(|e| format!("初始化评测数据库失败: {e}"))?;
        app.manage(DbState(pool.clone()));

        let registry = Arc::new(ToolRegistry::with_standard_tools());
        let agent_executor = Arc::new(AgentExecutor::new(Arc::clone(&registry)));
        app.manage(agent_executor.clone());
        app.manage(registry);

        let ask_user_responder = new_responder();
        app.manage(AskUserState(ask_user_responder));
        app.manage(AskUserPendingSessionState(Arc::new(std::sync::Mutex::new(
            None,
        ))));

        let tool_confirm_responder: ToolConfirmResponder = Arc::new(std::sync::Mutex::new(None));
        app.manage(ToolConfirmState(tool_confirm_responder.clone()));

        let cancel_flag = Arc::new(AtomicBool::new(false));
        app.manage(CancelFlagState(cancel_flag.clone()));

        let search_cache = Arc::new(SearchCache::new(std::time::Duration::from_secs(900), 100));
        app.manage(SearchCacheState(search_cache));

        let runtime_observability = Arc::new(RuntimeObservability::default());
        let session_admission_gate = Arc::new(SessionAdmissionGate::with_observability(
            runtime_observability.clone(),
        ));
        app.manage(SessionAdmissionGateState(session_admission_gate));
        let run_registry = Arc::new(RunRegistry::default());
        app.manage(RunRegistryState(run_registry.clone()));
        app.manage(RuntimeObservabilityState(runtime_observability.clone()));

        let journal_root = runtime_paths.sessions_dir.clone();
        std::fs::create_dir_all(&journal_root)
            .map_err(|e| format!("创建评测 journal 目录失败: {e}"))?;
        let journal = Arc::new(SessionJournalStore::with_registry_and_observability(
            journal_root,
            run_registry,
            runtime_observability,
        ));
        app.manage(SessionJournalStateHandle(journal.clone()));

        Ok(Self {
            app,
            pool,
            journal,
            cancel_flag,
        })
    }

    pub async fn run_scenario(
        &self,
        config: &LocalEvalConfig,
        scenario: &EvalScenario,
    ) -> Result<HeadlessEvalRun, String> {
        self.reset_eval_state().await?;

        let capability = config
            .capabilities
            .get(&scenario.capability_id)
            .ok_or_else(|| format!("未找到 capability_id 对应映射: {}", scenario.capability_id))?;
        let model_id = self.ensure_model_profile(config).await?;
        let skill_selection = resolve_scenario_skill_selection(capability, &self.pool).await?;

        let work_dir = PathBuf::from(&config.artifacts.output_dir)
            .join("workspaces")
            .join(&scenario.id);
        std::fs::create_dir_all(&work_dir).map_err(|e| format!("创建场景工作目录失败: {e}"))?;
        materialize_workspace_files(&work_dir, &scenario.input.workspace_files)?;

        let session_id = create_session(
            self.app.handle().clone(),
            skill_selection.skill_id.clone(),
            model_id.clone(),
            Some(work_dir.to_string_lossy().to_string()),
            None,
            Some(scenario.title.clone()),
            Some("standard".to_string()),
            Some("general".to_string()),
            None,
            self.app.state::<DbState>(),
        )
        .await?;

        self.cancel_flag
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let execution_error = send_message(
            self.app.handle().clone(),
            SendMessageRequest {
                session_id: session_id.clone(),
                parts: vec![SendMessagePart::Text {
                    text: scenario.input.user_text.clone(),
                }],
                max_iterations: None,
            },
            self.app.state::<DbState>(),
            self.app.state::<Arc<AgentExecutor>>(),
            self.app.state::<SessionJournalStateHandle>(),
            self.app.state::<CancelFlagState>(),
        )
        .await
        .err();

        let session_runs = list_session_runs_with_pool(&self.pool, &session_id).await?;
        let route_attempt_logs = load_route_attempt_logs(&self.pool, &session_id).await?;
        let trace = if let Some(run) = session_runs.last() {
            Some(export_session_run_trace_with_pool(&self.pool, &session_id, &run.id).await?)
        } else {
            None
        };
        let messages = get_messages_with_pool(&self.pool, &session_id).await?;
        let session_markdown =
            export_session_markdown_with_pool(&self.pool, &session_id, Some(self.journal.as_ref()))
                .await?;
        let journal_state = self
            .journal
            .read_state(&session_id)
            .await
            .map_err(|e| format!("读取 session journal 失败: {e}"))?;
        let final_output = extract_final_output(&messages);

        Ok(HeadlessEvalRun {
            scenario_id: scenario.id.clone(),
            capability_id: scenario.capability_id.clone(),
            session_id,
            skill_id: skill_selection.skill_id,
            model_id,
            work_dir,
            imported_skill_count: skill_selection.imported_skill_count,
            missing_mcp: skill_selection.missing_mcp,
            execution_error,
            session_runs,
            route_attempt_logs,
            trace,
            messages,
            session_markdown,
            journal_state,
            final_output,
        })
    }

    async fn ensure_model_profile(&self, config: &LocalEvalConfig) -> Result<String, String> {
        let profile_id = &config.models.default_profile;
        let profile = config
            .providers
            .get(profile_id)
            .ok_or_else(|| format!("默认模型 profile 未在 providers 中定义: {profile_id}"))?;

        validate_api_key_env_name(&profile.api_key_env, profile_id)?;
        let api_key = std::env::var(&profile.api_key_env)
            .map_err(|_| format!("缺少真实评测所需环境变量: {}", profile.api_key_env))?;
        let (api_format, base_url) = resolve_model_connection_defaults(profile)?;

        let model_id = format!("eval-{}", sanitize_id_component(profile_id));
        let supports_vision = api_format.trim().eq_ignore_ascii_case("openai");
        let model_config = ModelConfig {
            id: model_id.clone(),
            name: format!("Real Eval {}", profile_id),
            api_format: api_format.clone(),
            base_url: base_url.clone(),
            model_name: profile.model.clone(),
            is_default: true,
            supports_vision,
        };

        save_model_config_with_pool(&self.pool, model_config, api_key).await?;
        ensure_eval_provider_route(
            &self.pool,
            profile_id,
            profile,
            &api_format,
            &base_url,
            supports_vision,
        )
        .await?;
        set_default_model_with_pool(&self.pool, &model_id).await?;
        Ok(model_id)
    }

    async fn reset_eval_state(&self) -> Result<(), String> {
        for statement in [
            "DELETE FROM approvals",
            "DELETE FROM session_run_events",
            "DELETE FROM session_runs",
            "DELETE FROM messages",
            "DELETE FROM sessions",
            "DELETE FROM route_attempt_logs",
            "DELETE FROM installed_skills WHERE source_type = 'local'",
            "DELETE FROM model_configs WHERE id LIKE 'eval-%'",
        ] {
            sqlx::query(statement)
                .execute(&self.pool)
                .await
                .map_err(|e| format!("重置评测数据库失败: {e}"))?;
        }
        Ok(())
    }
}

struct ScenarioSkillSelection {
    skill_id: String,
    imported_skill_count: usize,
    missing_mcp: Vec<String>,
}

async fn resolve_scenario_skill_selection(
    capability: &CapabilityMapping,
    pool: &sqlx::SqlitePool,
) -> Result<ScenarioSkillSelection, String> {
    match capability.entry_kind.trim().to_ascii_lowercase().as_str() {
        "builtin" | "builtin_skill" => {
            let skill_id = capability.entry_name.trim();
            if skill_id.is_empty() {
                return Err("builtin capability mapping requires entry_name".to_string());
            }
            Ok(ScenarioSkillSelection {
                skill_id: skill_id.to_string(),
                imported_skill_count: 0,
                missing_mcp: Vec::new(),
            })
        }
        "workspace_skill" | "local_skill" => {
            let import_root = resolve_skill_import_root(capability)?;
            let import_batch =
                import_local_skills_to_pool(import_root.to_string_lossy().to_string(), pool, &[])
                    .await?;
            let skill_id = resolve_capability_skill_id(capability, &import_batch)?;
            Ok(ScenarioSkillSelection {
                skill_id,
                imported_skill_count: import_batch.installed.len(),
                missing_mcp: import_batch.missing_mcp,
            })
        }
        other => Err(format!(
            "unsupported capability entry_kind for real-agent eval: {other}"
        )),
    }
}

fn materialize_workspace_files(work_dir: &Path, files: &[EvalWorkspaceFile]) -> Result<(), String> {
    for file in files {
        let relative_path = validate_workspace_fixture_path(&file.path)?;
        let target = work_dir.join(relative_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建场景工作区文件目录失败 ({}): {e}", parent.display()))?;
        }
        let bytes = BASE64
            .decode(file.data_base64.trim())
            .map_err(|e| format!("解析场景工作区文件 base64 失败 ({}): {e}", file.path))?;
        std::fs::write(&target, bytes)
            .map_err(|e| format!("写入场景工作区文件失败 ({}): {e}", target.display()))?;
    }
    Ok(())
}

fn validate_workspace_fixture_path(raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("workspace fixture path cannot be empty".to_string());
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(format!("workspace fixture path must be relative: {raw}"));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(format!("workspace fixture path escapes work dir: {raw}"));
    }
    Ok(path.to_path_buf())
}

async fn ensure_eval_provider_route(
    pool: &sqlx::SqlitePool,
    profile_id: &str,
    profile: &ModelProviderProfile,
    api_format: &str,
    base_url: &str,
    supports_vision: bool,
) -> Result<(), String> {
    let api_key = std::env::var(&profile.api_key_env)
        .map_err(|_| format!("缺少真实评测所需环境变量: {}", profile.api_key_env))?;
    let provider_id = format!("eval-{}-provider", sanitize_id_component(profile_id));
    sqlx::query(
        "INSERT OR REPLACE INTO provider_configs
         (id, provider_key, display_name, protocol_type, base_url, auth_type, api_key_encrypted, org_id, extra_json, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 'bearer', ?, '', '{}', 1, datetime('now'), datetime('now'))",
    )
    .bind(&provider_id)
    .bind(&profile.provider)
    .bind(format!("Real Eval {}", profile_id))
    .bind(api_format)
    .bind(base_url)
    .bind(api_key)
    .execute(pool)
    .await
    .map_err(|e| format!("写入真实评测 provider 配置失败: {e}"))?;

    if supports_vision {
        sqlx::query(
            "INSERT OR REPLACE INTO routing_policies
             (capability, primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, enabled)
             VALUES ('vision', ?, ?, '[]', 120000, 0, 1)",
        )
        .bind(&provider_id)
        .bind(&profile.model)
        .execute(pool)
        .await
        .map_err(|e| format!("写入真实评测 vision 路由失败: {e}"))?;
    }

    Ok(())
}

async fn load_route_attempt_logs(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<Vec<RouteAttemptLog>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64, i64, String, bool, String, String)>(
        "SELECT session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, CAST(success AS BOOLEAN), error_message, created_at
         FROM route_attempt_logs
         WHERE session_id = ?
         ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("读取评测路由尝试日志失败: {e}"))?;

    Ok(rows
        .into_iter()
        .map(
            |(
                session_id,
                capability,
                api_format,
                model_name,
                attempt_index,
                retry_index,
                error_kind,
                success,
                error_message,
                created_at,
            )| RouteAttemptLog {
                session_id,
                capability,
                api_format,
                model_name,
                attempt_index,
                retry_index,
                error_kind,
                success,
                error_message,
                created_at,
            },
        )
        .collect())
}

fn resolve_skill_import_root(mapping: &CapabilityMapping) -> Result<PathBuf, String> {
    let root = PathBuf::from(&mapping.workspace_root);
    let candidates = [
        root.join(".codex").join("skills"),
        root.join("skills"),
        root.clone(),
    ];

    candidates
        .into_iter()
        .find(|candidate| candidate.exists() && candidate.is_dir())
        .ok_or_else(|| format!("未找到可导入的本地 skills 目录: {}", root.display()))
}

fn resolve_capability_skill_id(
    mapping: &CapabilityMapping,
    import_batch: &crate::commands::skills::LocalImportBatchResult,
) -> Result<String, String> {
    let target = mapping.entry_name.trim();
    for item in &import_batch.installed {
        let dir_name = Path::new(&item.dir_path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if dir_name.eq_ignore_ascii_case(target)
            || item.manifest.id.eq_ignore_ascii_case(target)
            || item.manifest.name.eq_ignore_ascii_case(target)
        {
            return Ok(item.manifest.id.clone());
        }
    }

    Err(format!(
        "未在本地导入结果中解析到 capability 入口 skill: {}",
        mapping.entry_name
    ))
}

fn resolve_model_connection_defaults(
    profile: &ModelProviderProfile,
) -> Result<(String, String), String> {
    let inferred_api_format = match profile.provider.trim().to_ascii_lowercase().as_str() {
        "openai" | "openrouter" => "openai".to_string(),
        "anthropic" | "minimax" | "minimax_cn" => "anthropic".to_string(),
        other => {
            return Err(format!(
                "provider {} 需要在 config.local.yaml 中显式提供 api_format/base_url",
                other
            ))
        }
    };
    let inferred_base_url = match profile.provider.trim().to_ascii_lowercase().as_str() {
        "openai" => "https://api.openai.com/v1".to_string(),
        "openrouter" => "https://openrouter.ai/api/v1".to_string(),
        "anthropic" => "https://api.anthropic.com".to_string(),
        "minimax" => "https://api.minimax.io/anthropic".to_string(),
        "minimax_cn" => "https://api.minimaxi.com/anthropic".to_string(),
        _ => String::new(),
    };

    Ok((
        profile.api_format.clone().unwrap_or(inferred_api_format),
        profile.base_url.clone().unwrap_or(inferred_base_url),
    ))
}

fn validate_api_key_env_name(raw: &str, profile_id: &str) -> Result<(), String> {
    let trimmed = raw.trim();
    let is_valid_env_name = !trimmed.is_empty()
        && trimmed.chars().enumerate().all(|(idx, ch)| {
            if idx == 0 {
                ch == '_' || ch.is_ascii_alphabetic()
            } else {
                ch == '_' || ch.is_ascii_alphanumeric()
            }
        });

    if is_valid_env_name {
        Ok(())
    } else {
        Err(format!(
            "providers.{profile_id}.api_key_env 必须填写环境变量名（例如 MINIMAX_API_KEY），不要直接填写真实 API key"
        ))
    }
}

fn sanitize_id_component(raw: &str) -> String {
    let lowered = raw.trim().to_ascii_lowercase();
    let mut out = String::new();
    let mut previous_dash = false;
    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            out.push('-');
            previous_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn extract_final_output(messages: &[Value]) -> String {
    messages
        .iter()
        .rev()
        .find_map(|message| {
            if message.get("role").and_then(Value::as_str) != Some("assistant") {
                return None;
            }
            message
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        materialize_workspace_files, resolve_capability_skill_id,
        resolve_model_connection_defaults, resolve_skill_import_root, sanitize_id_component,
        validate_api_key_env_name, validate_workspace_fixture_path,
    };
    use crate::agent::evals::config::ModelProviderProfile;
    use crate::agent::evals::EvalWorkspaceFile;
    use crate::commands::skills::{LocalImportBatchResult, LocalImportInstalledItem};
    use chrono::Utc;
    use skillpack_rs::SkillManifest;
    use tempfile::tempdir;

    #[test]
    fn resolve_skill_import_root_prefers_codex_skills_dir() {
        let temp = tempdir().expect("tempdir");
        let codex_skills = temp.path().join(".codex").join("skills");
        std::fs::create_dir_all(&codex_skills).expect("create .codex skills");
        std::fs::create_dir_all(temp.path().join("skills")).expect("create root skills");

        let mapping = crate::agent::evals::CapabilityMapping {
            workspace_root: temp.path().to_string_lossy().to_string(),
            entry_kind: "workspace_skill".to_string(),
            entry_name: "feishu-pm-hub".to_string(),
        };

        let resolved = resolve_skill_import_root(&mapping).expect("resolve import root");
        assert_eq!(resolved, codex_skills);
    }

    #[test]
    fn resolve_capability_skill_id_matches_imported_dir_name() {
        let batch = LocalImportBatchResult {
            installed: vec![LocalImportInstalledItem {
                dir_path: r"E:\repo\.codex\skills\feishu-pm-hub".to_string(),
                manifest: SkillManifest {
                    id: "local-feishu-pm-hub".to_string(),
                    name: "feishu-pm-hub".to_string(),
                    description: String::new(),
                    version: "local".to_string(),
                    author: String::new(),
                    recommended_model: String::new(),
                    tags: Vec::new(),
                    created_at: Utc::now(),
                    username_hint: None,
                    encrypted_verify: String::new(),
                },
            }],
            failed: Vec::new(),
            missing_mcp: Vec::new(),
        };
        let mapping = crate::agent::evals::CapabilityMapping {
            workspace_root: r"E:\repo".to_string(),
            entry_kind: "workspace_skill".to_string(),
            entry_name: "feishu-pm-hub".to_string(),
        };

        assert_eq!(
            resolve_capability_skill_id(&mapping, &batch).as_deref(),
            Ok("local-feishu-pm-hub")
        );
    }

    #[test]
    fn workspace_fixture_paths_must_stay_relative() {
        assert!(validate_workspace_fixture_path("images/a.png").is_ok());
        assert!(validate_workspace_fixture_path("../a.png").is_err());
        assert!(validate_workspace_fixture_path(r"C:\tmp\a.png").is_err());
    }

    #[test]
    fn materialize_workspace_files_decodes_base64_into_work_dir() {
        let temp = tempdir().expect("tempdir");
        materialize_workspace_files(
            temp.path(),
            &[EvalWorkspaceFile {
                path: "images/a.png".to_string(),
                data_base64: "aGVsbG8=".to_string(),
            }],
        )
        .expect("materialize fixture");

        let bytes = std::fs::read(temp.path().join("images").join("a.png")).expect("read file");
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn resolve_model_connection_defaults_uses_common_provider_defaults() {
        let profile = ModelProviderProfile {
            provider: "openrouter".to_string(),
            model: "gpt-5.4".to_string(),
            api_key_env: "OPENROUTER_API_KEY".to_string(),
            api_format: None,
            base_url: None,
        };

        let (api_format, base_url) = resolve_model_connection_defaults(&profile).expect("defaults");
        assert_eq!(api_format, "openai");
        assert_eq!(base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn resolve_model_connection_defaults_supports_minimax_anthropic_defaults() {
        let profile = ModelProviderProfile {
            provider: "minimax".to_string(),
            model: "MiniMax-M2.5".to_string(),
            api_key_env: "MINIMAX_API_KEY".to_string(),
            api_format: None,
            base_url: None,
        };

        let (api_format, base_url) = resolve_model_connection_defaults(&profile).expect("defaults");
        assert_eq!(api_format, "anthropic");
        assert_eq!(base_url, "https://api.minimax.io/anthropic");
    }

    #[test]
    fn validate_api_key_env_name_rejects_literal_secret_values() {
        let err = validate_api_key_env_name("sk-demo-secret-value", "minimax_anthropic")
            .expect_err("literal secret should be rejected");

        assert!(err.contains("api_key_env"));
        assert!(!err.contains("sk-demo-secret-value"));
    }

    #[test]
    fn sanitize_id_component_normalizes_profile_names() {
        assert_eq!(sanitize_id_component("Local Real Eval"), "local-real-eval");
        assert_eq!(sanitize_id_component(" eval_profile "), "eval-profile");
    }
}
