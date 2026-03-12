use super::employee_agents::list_agent_employees_with_pool;
use super::models::{
    get_capability_routing_policy_from_pool, get_chat_routing_policy_from_pool,
    load_routing_settings_from_pool,
};
use super::runtime_preferences::resolve_default_work_dir_with_pool;
use async_trait::async_trait;
use runtime_chat_app::{
    ChatEmployeeDirectory, ChatEmployeeSnapshot, ChatRoutePolicySnapshot, ChatRoutingSnapshot,
    ChatSessionContextRepository, ChatSettingsRepository, ProviderConnectionSnapshot,
    RoutingSettingsSnapshot, SessionExecutionContextSnapshot, SessionModelSnapshot,
};
use sqlx::SqlitePool;

pub struct PoolChatSettingsRepository<'a> {
    db: &'a SqlitePool,
}

pub struct PoolChatEmployeeDirectory<'a> {
    db: &'a SqlitePool,
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

#[async_trait]
impl ChatSettingsRepository for PoolChatSettingsRepository<'_> {
    async fn load_routing_settings(&self) -> Result<RoutingSettingsSnapshot, String> {
        let settings = load_routing_settings_from_pool(self.db).await?;
        Ok(RoutingSettingsSnapshot {
            max_call_depth: settings.max_call_depth,
            node_timeout_seconds: settings.node_timeout_seconds,
            retry_count: settings.retry_count,
        })
    }

    async fn load_chat_routing(&self) -> Result<Option<ChatRoutingSnapshot>, String> {
        Ok(get_chat_routing_policy_from_pool(self.db)
            .await?
            .map(|policy| ChatRoutingSnapshot {
                primary_provider_id: policy.primary_provider_id,
                primary_model: policy.primary_model,
                fallback_chain_json: policy.fallback_chain_json,
                timeout_ms: policy.timeout_ms,
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
        Ok(get_capability_routing_policy_from_pool(self.db, capability)
            .await?
            .map(|policy| ChatRoutePolicySnapshot {
                primary_provider_id: policy.primary_provider_id,
                primary_model: policy.primary_model,
                fallback_chain_json: policy.fallback_chain_json,
                retry_count: policy.retry_count,
                enabled: policy.enabled,
            }))
    }

    async fn get_provider_connection(
        &self,
        provider_id: &str,
    ) -> Result<Option<ProviderConnectionSnapshot>, String> {
        let row = sqlx::query_as::<_, (String, String, String)>(
            "SELECT protocol_type, base_url, api_key_encrypted FROM provider_configs WHERE id = ? AND enabled = 1 LIMIT 1",
        )
        .bind(provider_id)
        .fetch_optional(self.db)
        .await
        .map_err(|e| format!("读取 Provider 配置失败: {e}"))?;

        Ok(row.map(
            |(protocol_type, base_url, api_key)| ProviderConnectionSnapshot {
                provider_id: provider_id.to_string(),
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
        resolve_default_work_dir_with_pool(self.db).await.map(Some)
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
        let employees = list_agent_employees_with_pool(self.db).await?;
        Ok(employees
            .into_iter()
            .map(|employee| ChatEmployeeSnapshot {
                id: employee.id,
                employee_id: employee.employee_id,
                name: employee.name,
                role_id: employee.role_id,
                feishu_open_id: employee.feishu_open_id,
                enabled: employee.enabled,
            })
            .collect())
    }
}
