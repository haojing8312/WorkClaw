use super::{CapabilityMapping, EvalScenario, LocalEvalConfig, ModelProviderProfile};
use crate::agent::runtime::{
    RunRegistry, RunRegistryState, RuntimeObservability, RuntimeObservabilityState,
    SearchCacheState, SessionAdmissionGate, SessionAdmissionGateState, ToolConfirmResponder,
};
use crate::agent::tools::new_responder;
use crate::agent::tools::search_providers::cache::SearchCache;
use crate::agent::{AgentExecutor, ToolRegistry};
use crate::commands::chat::{
    AskUserState, CancelFlagState, SendMessagePart, SendMessageRequest, ToolConfirmState,
    create_session, send_message,
};
use crate::commands::models::{
    ModelConfig, save_model_config_with_pool, set_default_model_with_pool,
};
use crate::commands::session_runs::{
    SessionRunProjection, export_session_run_trace_with_pool, list_session_runs_with_pool,
};
use crate::commands::skills::{DbState, import_local_skills_to_pool};
use crate::commands::chat_session_commands::export_session_markdown_with_pool;
use crate::commands::chat_session_io::get_messages_with_pool;
use crate::session_journal::{SessionJournalState, SessionJournalStateHandle, SessionJournalStore};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tauri::Manager;
use tauri::test::{mock_context, noop_assets};
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
    pub trace: Option<crate::agent::runtime::SessionRunTrace>,
    pub messages: Vec<Value>,
    pub session_markdown: String,
    pub journal_state: SessionJournalState,
    pub final_output: String,
}

impl RealAgentEvalRunner {
    pub async fn new(config: &LocalEvalConfig) -> Result<Self, String> {
        let output_root = PathBuf::from(&config.artifacts.output_dir);
        std::fs::create_dir_all(&output_root)
            .map_err(|e| format!("创建评测输出目录失败: {e}"))?;

        let app = tauri::Builder::default()
            .build(mock_context(noop_assets()))
            .map_err(|e| format!("创建 headless runtime app 失败: {e}"))?;

        let pool = crate::db::init_db(&app.handle())
            .await
            .map_err(|e| format!("初始化评测数据库失败: {e}"))?;
        app.manage(DbState(pool.clone()));

        let registry = Arc::new(ToolRegistry::with_standard_tools());
        let agent_executor = Arc::new(AgentExecutor::new(Arc::clone(&registry)));
        app.manage(agent_executor.clone());
        app.manage(registry);

        let ask_user_responder = new_responder();
        app.manage(AskUserState(ask_user_responder));

        let tool_confirm_responder: ToolConfirmResponder =
            Arc::new(std::sync::Mutex::new(None));
        app.manage(ToolConfirmState(tool_confirm_responder.clone()));

        let cancel_flag = Arc::new(AtomicBool::new(false));
        app.manage(CancelFlagState(cancel_flag.clone()));

        let search_cache = Arc::new(SearchCache::new(std::time::Duration::from_secs(900), 100));
        app.manage(SearchCacheState(search_cache));

        let runtime_observability = Arc::new(RuntimeObservability::default());
        let session_admission_gate =
            Arc::new(SessionAdmissionGate::with_observability(runtime_observability.clone()));
        app.manage(SessionAdmissionGateState(session_admission_gate));
        let run_registry = Arc::new(RunRegistry::default());
        app.manage(RunRegistryState(run_registry.clone()));
        app.manage(RuntimeObservabilityState(runtime_observability.clone()));

        let journal_root = output_root.join("runtime-state").join("sessions");
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
        let import_root = resolve_skill_import_root(capability)?;
        let import_batch = import_local_skills_to_pool(
            import_root.to_string_lossy().to_string(),
            &self.pool,
            &[],
        )
        .await?;
        let skill_id = resolve_capability_skill_id(capability, &import_batch)?;

        let work_dir = PathBuf::from(&config.artifacts.output_dir)
            .join("workspaces")
            .join(&scenario.id);
        std::fs::create_dir_all(&work_dir)
            .map_err(|e| format!("创建场景工作目录失败: {e}"))?;

        let session_id = create_session(
            self.app.handle().clone(),
            skill_id.clone(),
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
            skill_id,
            model_id,
            work_dir,
            imported_skill_count: import_batch.installed.len(),
            missing_mcp: import_batch.missing_mcp,
            execution_error,
            session_runs,
            trace,
            messages,
            session_markdown,
            journal_state,
            final_output,
        })
    }

    async fn ensure_model_profile(&self, config: &LocalEvalConfig) -> Result<String, String> {
        let profile_id = &config.models.default_profile;
        let profile = config.providers.get(profile_id).ok_or_else(|| {
            format!("默认模型 profile 未在 providers 中定义: {profile_id}")
        })?;

        let api_key = std::env::var(&profile.api_key_env).map_err(|_| {
            format!("缺少真实评测所需环境变量: {}", profile.api_key_env)
        })?;
        let (api_format, base_url) = resolve_model_connection_defaults(profile)?;

        let model_id = format!("eval-{}", sanitize_id_component(profile_id));
        let model_config = ModelConfig {
            id: model_id.clone(),
            name: format!("Real Eval {}", profile_id),
            api_format,
            base_url,
            model_name: profile.model.clone(),
            is_default: true,
        };

        save_model_config_with_pool(&self.pool, model_config, api_key).await?;
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
        "anthropic" => "anthropic".to_string(),
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
        _ => String::new(),
    };

    Ok((
        profile
            .api_format
            .clone()
            .unwrap_or(inferred_api_format),
        profile.base_url.clone().unwrap_or(inferred_base_url),
    ))
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
        resolve_capability_skill_id, resolve_model_connection_defaults, resolve_skill_import_root,
        sanitize_id_component,
    };
    use crate::agent::evals::config::ModelProviderProfile;
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
    fn resolve_model_connection_defaults_uses_common_provider_defaults() {
        let profile = ModelProviderProfile {
            provider: "openrouter".to_string(),
            model: "gpt-5.4".to_string(),
            api_key_env: "OPENROUTER_API_KEY".to_string(),
            api_format: None,
            base_url: None,
        };

        let (api_format, base_url) =
            resolve_model_connection_defaults(&profile).expect("defaults");
        assert_eq!(api_format, "openai");
        assert_eq!(base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn sanitize_id_component_normalizes_profile_names() {
        assert_eq!(sanitize_id_component("Local Real Eval"), "local-real-eval");
        assert_eq!(sanitize_id_component(" eval_profile "), "eval-profile");
    }
}
