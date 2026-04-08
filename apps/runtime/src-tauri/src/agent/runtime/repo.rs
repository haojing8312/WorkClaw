use crate::agent::tool_manifest::{ToolCategory, ToolSource};
use async_trait::async_trait;
use runtime_chat_app::{
    ChatEmployeeDirectory, ChatEmployeeSnapshot, ChatRoutePolicySnapshot, ChatRoutingSnapshot,
    ChatSessionContextRepository, ChatSettingsRepository, ProviderConnectionSnapshot,
    RoutingSettingsSnapshot, SessionExecutionContextSnapshot, SessionModelSnapshot,
};
use sqlx::{Row, SqlitePool};

pub struct PoolChatSettingsRepository<'a> {
    db: &'a SqlitePool,
}

pub struct PoolChatEmployeeDirectory<'a> {
    db: &'a SqlitePool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeToolPolicyDefaultsSnapshot {
    pub label: String,
    pub denied_tool_names: Vec<String>,
    pub denied_categories: Vec<ToolCategory>,
    pub allowed_sources: Option<Vec<ToolSource>>,
    pub allowed_mcp_servers: Option<Vec<String>>,
}

impl<'a> PoolChatSettingsRepository<'a> {
    pub fn new(db: &'a SqlitePool) -> Self {
        Self { db }
    }
}

impl<'a> PoolChatEmployeeDirectory<'a> {
    pub fn new(db: &'a SqlitePool) -> Self {
        Self { db }
    }
}

fn compute_default_work_dir_with_home() -> String {
    crate::runtime_paths::resolve_default_work_dir_with_home_env(
        std::env::var_os("USERPROFILE"),
        std::env::var_os("HOME"),
    )
    .to_string_lossy()
    .to_string()
}

async fn load_runtime_setting(db: &SqlitePool, key: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM app_settings WHERE key = ? LIMIT 1")
        .bind(key)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("读取运行时设置失败 (key={key}): {e}"))
}

fn parse_list_setting(raw: Option<String>) -> Vec<String> {
    let Some(raw) = raw.map(|value| value.trim().to_string()) else {
        return Vec::new();
    };
    if raw.is_empty() {
        return Vec::new();
    }
    if raw.starts_with('[') {
        if let Ok(values) = serde_json::from_str::<Vec<String>>(&raw) {
            return values
                .into_iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
        }
    }
    raw.split([',', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn parse_tool_category_name(raw: &str) -> Option<ToolCategory> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "file" => Some(ToolCategory::File),
        "shell" => Some(ToolCategory::Shell),
        "web" => Some(ToolCategory::Web),
        "browser" => Some(ToolCategory::Browser),
        "system" => Some(ToolCategory::System),
        "planning" => Some(ToolCategory::Planning),
        "agent" => Some(ToolCategory::Agent),
        "memory" => Some(ToolCategory::Memory),
        "search" => Some(ToolCategory::Search),
        "integration" => Some(ToolCategory::Integration),
        "other" => Some(ToolCategory::Other),
        _ => None,
    }
}

fn parse_tool_source_name(raw: &str) -> Option<ToolSource> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "native" => Some(ToolSource::Native),
        "runtime" => Some(ToolSource::Runtime),
        "sidecar" => Some(ToolSource::Sidecar),
        "mcp" => Some(ToolSource::Mcp),
        "plugin" => Some(ToolSource::Plugin),
        "alias" => Some(ToolSource::Alias),
        _ => None,
    }
}

pub(crate) async fn load_runtime_tool_policy_defaults(
    db: &SqlitePool,
) -> Result<RuntimeToolPolicyDefaultsSnapshot, String> {
    let denied_tool_names =
        parse_list_setting(load_runtime_setting(db, "runtime_tool_policy_denied_tools").await?);
    let denied_categories = parse_list_setting(
        load_runtime_setting(db, "runtime_tool_policy_denied_categories").await?,
    )
    .into_iter()
    .filter_map(|name| parse_tool_category_name(&name))
    .collect::<Vec<_>>();
    let allowed_sources = {
        let sources = parse_list_setting(
            load_runtime_setting(db, "runtime_tool_policy_allowed_sources").await?,
        )
        .into_iter()
        .filter_map(|name| parse_tool_source_name(&name))
        .collect::<Vec<_>>();
        (!sources.is_empty()).then_some(sources)
    };
    let allowed_mcp_servers = {
        let servers = parse_list_setting(
            load_runtime_setting(db, "runtime_tool_policy_allowed_mcp_servers").await?,
        );
        (!servers.is_empty()).then_some(servers)
    };
    let operation_permission_mode = load_runtime_setting(db, "runtime_operation_permission_mode")
        .await?
        .unwrap_or_else(|| "standard".to_string());

    Ok(RuntimeToolPolicyDefaultsSnapshot {
        label: format!("runtime_preferences:{}", operation_permission_mode.trim()),
        denied_tool_names,
        denied_categories,
        allowed_sources,
        allowed_mcp_servers,
    })
}

async fn load_routing_policy_snapshot(
    db: &SqlitePool,
    capability: &str,
) -> Result<Option<(ChatRoutePolicySnapshot, i64)>, String> {
    let row = sqlx::query_as::<_, (String, String, String, i64, i64, bool)>(
        "SELECT primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, CAST(enabled AS BOOLEAN)
         FROM routing_policies
         WHERE capability = ?
         LIMIT 1",
    )
    .bind(capability)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("读取路由策略失败 (capability={capability}): {e}"))?;

    Ok(row.map(
        |(
            primary_provider_id,
            primary_model,
            fallback_chain_json,
            timeout_ms,
            retry_count,
            enabled,
        )| {
            (
                ChatRoutePolicySnapshot {
                    primary_provider_id,
                    primary_model,
                    fallback_chain_json,
                    retry_count,
                    enabled,
                },
                timeout_ms,
            )
        },
    ))
}

#[async_trait]
impl ChatSettingsRepository for PoolChatSettingsRepository<'_> {
    async fn load_routing_settings(&self) -> Result<RoutingSettingsSnapshot, String> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT key, value
             FROM app_settings
             WHERE key IN ('route_max_call_depth', 'route_node_timeout_seconds', 'route_retry_count')",
        )
        .fetch_all(self.db)
        .await
        .map_err(|e| format!("读取路由设置失败: {e}"))?;

        let mut settings = RoutingSettingsSnapshot {
            max_call_depth: 4,
            node_timeout_seconds: 60,
            retry_count: 0,
        };

        for (key, value) in rows {
            match key.as_str() {
                "route_max_call_depth" => {
                    settings.max_call_depth = value.parse::<usize>().unwrap_or(4).clamp(2, 8);
                }
                "route_node_timeout_seconds" => {
                    settings.node_timeout_seconds =
                        value.parse::<u64>().unwrap_or(60).clamp(5, 600);
                }
                "route_retry_count" => {
                    settings.retry_count = value.parse::<usize>().unwrap_or(0).clamp(0, 2);
                }
                _ => {}
            }
        }

        Ok(settings)
    }

    async fn load_chat_routing(&self) -> Result<Option<ChatRoutingSnapshot>, String> {
        Ok(load_routing_policy_snapshot(self.db, "chat")
            .await?
            .map(|(policy, timeout_ms)| ChatRoutingSnapshot {
                primary_provider_id: policy.primary_provider_id,
                primary_model: policy.primary_model,
                fallback_chain_json: policy.fallback_chain_json,
                timeout_ms,
                retry_count: policy.retry_count,
                enabled: policy.enabled,
            }))
    }

    async fn resolve_default_model_id(&self) -> Result<Option<String>, String> {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM model_configs WHERE api_format NOT LIKE 'search_%' AND is_default = 1 LIMIT 1",
        )
        .fetch_optional(self.db)
        .await
        .map_err(|e| e.to_string())
    }

    async fn resolve_default_usable_model_id(&self) -> Result<Option<String>, String> {
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM model_configs WHERE api_format NOT LIKE 'search_%' AND is_default = 1 AND TRIM(api_key) != '' LIMIT 1",
        )
        .fetch_optional(self.db)
        .await
        .map_err(|e| e.to_string())?
        {
            return Ok(Some(id));
        }

        sqlx::query_scalar::<_, String>(
            "SELECT id FROM model_configs WHERE api_format NOT LIKE 'search_%' AND TRIM(api_key) != '' ORDER BY rowid ASC LIMIT 1",
        )
        .fetch_optional(self.db)
        .await
        .map_err(|e| e.to_string())
    }

    async fn load_route_policy(
        &self,
        capability: &str,
    ) -> Result<Option<ChatRoutePolicySnapshot>, String> {
        Ok(load_routing_policy_snapshot(self.db, capability)
            .await?
            .map(|(policy, _)| policy))
    }

    async fn get_provider_connection(
        &self,
        provider_id: &str,
    ) -> Result<Option<ProviderConnectionSnapshot>, String> {
        let row = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT provider_key, protocol_type, base_url, api_key_encrypted FROM provider_configs WHERE id = ? AND enabled = 1 LIMIT 1",
        )
        .bind(provider_id)
        .fetch_optional(self.db)
        .await
        .map_err(|e| format!("读取 Provider 配置失败: {e}"))?;

        Ok(row.map(
            |(provider_key, protocol_type, base_url, api_key)| ProviderConnectionSnapshot {
                provider_id: provider_id.to_string(),
                provider_key,
                protocol_type,
                base_url,
                api_key,
            },
        ))
    }

    async fn load_session_model(&self, model_id: &str) -> Result<SessionModelSnapshot, String> {
        let (api_format, base_url, model_name, api_key) =
            sqlx::query_as::<_, (String, String, String, String)>(
                "SELECT api_format, base_url, model_name, api_key FROM model_configs WHERE id = ?",
            )
            .bind(model_id)
            .fetch_one(self.db)
            .await
            .map_err(|e| format!("模型配置不存在 (model_id={model_id}): {e}"))?;

        Ok(SessionModelSnapshot {
            model_id: model_id.to_string(),
            api_format,
            base_url,
            model_name,
            api_key,
        })
    }

    async fn load_default_work_dir(&self) -> Result<Option<String>, String> {
        let dir = load_runtime_setting(self.db, "runtime_default_work_dir")
            .await?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(compute_default_work_dir_with_home);

        if dir.trim().is_empty() {
            return Err("default work dir is empty".to_string());
        }

        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("failed to create default work dir: {e}"))?;
        Ok(Some(dir))
    }
}

#[async_trait]
impl ChatSessionContextRepository for PoolChatSettingsRepository<'_> {
    async fn load_session_execution_context(
        &self,
        session_id: Option<&str>,
    ) -> Result<SessionExecutionContextSnapshot, String> {
        let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) else {
            return Ok(SessionExecutionContextSnapshot {
                session_id: String::new(),
                session_mode: "general".to_string(),
                team_id: String::new(),
                employee_id: String::new(),
                work_dir: String::new(),
                imported_mcp_server_ids: Vec::new(),
            });
        };

        let row = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT COALESCE(session_mode, 'general'), COALESCE(team_id, ''), COALESCE(employee_id, ''), COALESCE(work_dir, '')
             FROM sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(self.db)
        .await
        .map_err(|e| format!("读取会话执行上下文失败 (session_id={session_id}): {e}"))?;

        let (session_mode, team_id, employee_id, work_dir) = row.unwrap_or_else(|| {
            (
                "general".to_string(),
                String::new(),
                String::new(),
                String::new(),
            )
        });

        Ok(SessionExecutionContextSnapshot {
            session_id: session_id.to_string(),
            session_mode,
            team_id,
            employee_id,
            work_dir,
            imported_mcp_server_ids: Vec::new(),
        })
    }
}

#[async_trait]
impl ChatEmployeeDirectory for PoolChatEmployeeDirectory<'_> {
    async fn list_collaboration_candidates(&self) -> Result<Vec<ChatEmployeeSnapshot>, String> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                COALESCE(NULLIF(TRIM(employee_id), ''), role_id) AS employee_id,
                name,
                role_id,
                COALESCE(feishu_open_id, '') AS feishu_open_id,
                enabled
            FROM agent_employees
            ORDER BY is_default DESC, updated_at DESC
            "#,
        )
        .fetch_all(self.db)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows
            .into_iter()
            .map(|row| ChatEmployeeSnapshot {
                id: row.try_get("id").expect("employee row id"),
                employee_id: row
                    .try_get("employee_id")
                    .expect("employee row employee_id"),
                name: row.try_get("name").expect("employee row name"),
                role_id: row.try_get("role_id").expect("employee row role_id"),
                feishu_open_id: row
                    .try_get("feishu_open_id")
                    .expect("employee row feishu_open_id"),
                enabled: row
                    .try_get::<i64, _>("enabled")
                    .expect("employee row enabled")
                    != 0,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::load_runtime_tool_policy_defaults;
    use crate::agent::tool_manifest::{ToolCategory, ToolSource};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::query(
            "CREATE TABLE app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create app_settings");
        pool
    }

    #[tokio::test]
    async fn load_runtime_tool_policy_defaults_parses_runtime_app_settings() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES (?, ?), (?, ?), (?, ?), (?, ?), (?, ?)",
        )
        .bind("runtime_operation_permission_mode")
        .bind("full_access")
        .bind("runtime_tool_policy_denied_tools")
        .bind("[\"bash\", \"edit\"]")
        .bind("runtime_tool_policy_denied_categories")
        .bind("shell, browser")
        .bind("runtime_tool_policy_allowed_sources")
        .bind("native,mcp")
        .bind("runtime_tool_policy_allowed_mcp_servers")
        .bind("repo-files\nbrave-search")
        .execute(&pool)
        .await
        .expect("seed app_settings");

        let snapshot = load_runtime_tool_policy_defaults(&pool)
            .await
            .expect("load policy defaults");

        assert_eq!(snapshot.label, "runtime_preferences:full_access");
        assert_eq!(snapshot.denied_tool_names, vec!["bash", "edit"]);
        assert_eq!(
            snapshot.denied_categories,
            vec![ToolCategory::Shell, ToolCategory::Browser]
        );
        assert_eq!(
            snapshot.allowed_sources,
            Some(vec![ToolSource::Native, ToolSource::Mcp])
        );
        assert_eq!(
            snapshot.allowed_mcp_servers,
            Some(vec!["repo-files".to_string(), "brave-search".to_string()])
        );
    }
}
