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
