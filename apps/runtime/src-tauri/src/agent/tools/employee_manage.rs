use crate::agent::types::{Tool, ToolContext};
use crate::commands::agent_profile::{
    apply_agent_profile_with_pool, AgentProfileAnswerInput, AgentProfilePayload,
};
use crate::commands::employee_agents::{
    list_agent_employees_with_pool, upsert_agent_employee_with_pool, AgentEmployee,
    UpsertAgentEmployeeInput,
};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

const DEFAULT_PRIMARY_SKILL_ID: &str = "builtin-general";

pub struct EmployeeManageTool {
    pool: SqlitePool,
}

impl EmployeeManageTool {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_string_array(input: &Value, field: &str) -> Vec<String> {
        input[field]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn parse_profile_answers(input: &Value) -> Vec<AgentProfileAnswerInput> {
        input["profile_answers"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let key = item["key"].as_str().map(str::trim).unwrap_or("");
                        if key.is_empty() {
                            return None;
                        }
                        let question = item["question"]
                            .as_str()
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
                            .unwrap_or(key)
                            .to_string();
                        let answer = item["answer"]
                            .as_str()
                            .map(str::trim)
                            .unwrap_or("")
                            .to_string();
                        Some(AgentProfileAnswerInput {
                            key: key.to_string(),
                            question,
                            answer,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn parse_optional_string(input: &Value, field: &str) -> Option<String> {
        input
            .as_object()
            .and_then(|obj| obj.get(field))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .map(ToString::to_string)
    }

    fn parse_optional_bool(input: &Value, field: &str) -> Option<bool> {
        input
            .as_object()
            .and_then(|obj| obj.get(field))
            .and_then(|v| v.as_bool())
    }

    fn dedupe_skill_ids(skill_ids: Vec<String>) -> Vec<String> {
        let mut seen_skill_ids = HashSet::new();
        skill_ids
            .into_iter()
            .filter(|id| seen_skill_ids.insert(id.to_lowercase()))
            .collect::<Vec<_>>()
    }

    async fn resolve_employee(
        pool: &SqlitePool,
        input: &Value,
        action: &str,
    ) -> std::result::Result<AgentEmployee, String> {
        let employee_db_id = input["employee_db_id"]
            .as_str()
            .map(str::trim)
            .unwrap_or("");
        let employee_id = input["employee_id"].as_str().map(str::trim).unwrap_or("");
        let employees = list_agent_employees_with_pool(pool).await?;

        if !employee_db_id.is_empty() {
            return employees
                .into_iter()
                .find(|item| item.id.eq_ignore_ascii_case(employee_db_id))
                .ok_or_else(|| format!("{action} 未找到对应员工"));
        }

        if !employee_id.is_empty() {
            return employees
                .into_iter()
                .find(|item| {
                    item.id.eq_ignore_ascii_case(employee_id)
                        || item.employee_id.eq_ignore_ascii_case(employee_id)
                        || item.role_id.eq_ignore_ascii_case(employee_id)
                })
                .ok_or_else(|| format!("{action} 未找到对应员工"));
        }

        Err(format!("{action} 缺少 employee_db_id 或 employee_id 参数"))
    }

    fn normalize_employee_id(raw: &str) -> String {
        let mut out = String::new();
        let mut last_sep = false;
        for ch in raw.trim().chars() {
            if ch.is_ascii_alphanumeric() {
                out.push(ch.to_ascii_lowercase());
                last_sep = false;
            } else if (ch == '-' || ch == '_' || ch == ' ') && !last_sep {
                out.push('_');
                last_sep = true;
            }
        }
        let normalized = out.trim_matches('_').to_string();
        if normalized.is_empty() {
            let id = Uuid::new_v4().to_string();
            format!("employee_{}", &id[..8])
        } else {
            normalized
        }
    }

    fn default_employee_work_dir(employee_id: &str) -> String {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join("WorkClaw")
            .join("workspace")
            .join("employees")
            .join(employee_id)
            .to_string_lossy()
            .to_string()
    }

    fn block_on<T, F>(&self, fut: F) -> Result<T>
    where
        F: std::future::Future<Output = std::result::Result<T, String>>,
    {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| anyhow!("构建运行时失败: {}", e))?;
        rt.block_on(fut).map_err(|e| anyhow!(e))
    }

    async fn list_skills(pool: SqlitePool) -> std::result::Result<Value, String> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, manifest, COALESCE(source_type, 'encrypted') FROM installed_skills ORDER BY installed_at DESC",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for (id, manifest_json, source_type) in rows {
            let Ok(manifest) = serde_json::from_str::<skillpack_rs::SkillManifest>(&manifest_json)
            else {
                continue;
            };
            items.push(json!({
                "id": id,
                "name": manifest.name,
                "description": manifest.description,
                "source_type": source_type,
                "tags": manifest.tags,
            }));
        }
        Ok(json!({
            "action": "list_skills",
            "items": items,
        }))
    }

    async fn list_employees(pool: SqlitePool) -> std::result::Result<Value, String> {
        let employees = list_agent_employees_with_pool(&pool).await?;
        Ok(json!({
            "action": "list_employees",
            "items": employees,
        }))
    }

    async fn create_employee(pool: SqlitePool, input: Value) -> std::result::Result<Value, String> {
        let name = input["name"].as_str().unwrap_or("").trim();
        if name.is_empty() {
            return Err("create_employee 缺少 name 参数".to_string());
        }

        let requested_employee_id = input["employee_id"].as_str().unwrap_or("").trim();
        let generated = Self::normalize_employee_id(name);
        let employee_id = if requested_employee_id.is_empty() {
            generated
        } else {
            Self::normalize_employee_id(requested_employee_id)
        };

        let mut skill_ids = Self::parse_string_array(&input, "skill_ids");
        let requested_primary_skill_id = input["primary_skill_id"]
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string);
        let primary_skill_id = requested_primary_skill_id
            .or_else(|| skill_ids.first().cloned())
            .unwrap_or_else(|| DEFAULT_PRIMARY_SKILL_ID.to_string());
        if !skill_ids
            .iter()
            .any(|id| id.eq_ignore_ascii_case(primary_skill_id.as_str()))
        {
            skill_ids.insert(0, primary_skill_id.clone());
        }
        let skill_ids = Self::dedupe_skill_ids(skill_ids);

        let default_work_dir = input["default_work_dir"]
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| Self::default_employee_work_dir(&employee_id));

        let upsert_input = UpsertAgentEmployeeInput {
            id: input["id"]
                .as_str()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToString::to_string),
            employee_id: employee_id.clone(),
            name: name.to_string(),
            role_id: employee_id.clone(),
            persona: input["persona"].as_str().unwrap_or("").trim().to_string(),
            feishu_open_id: input["feishu_open_id"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string(),
            feishu_app_id: input["feishu_app_id"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string(),
            feishu_app_secret: input["feishu_app_secret"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string(),
            primary_skill_id,
            default_work_dir,
            openclaw_agent_id: employee_id,
            routing_priority: 100,
            enabled_scopes: {
                let scopes = Self::parse_string_array(&input, "enabled_scopes");
                if scopes.is_empty() {
                    vec!["app".to_string()]
                } else {
                    scopes
                }
            },
            enabled: input["enabled"].as_bool().unwrap_or(true),
            is_default: input["is_default"].as_bool().unwrap_or(false),
            skill_ids,
        };

        let created_id = upsert_agent_employee_with_pool(&pool, upsert_input).await?;
        let employees = list_agent_employees_with_pool(&pool).await?;
        let created = employees
            .into_iter()
            .find(|item| item.id == created_id)
            .ok_or_else(|| "创建成功但未找到员工记录".to_string())?;

        let auto_apply_profile = input["auto_apply_profile"].as_bool().unwrap_or(true);
        let profile = if auto_apply_profile {
            let payload = AgentProfilePayload {
                employee_db_id: created_id.clone(),
                answers: Self::parse_profile_answers(&input),
            };
            match apply_agent_profile_with_pool(&pool, payload).await {
                Ok(result) => json!({
                    "applied": true,
                    "files": result.files,
                }),
                Err(error) => json!({
                    "applied": false,
                    "error": error,
                }),
            }
        } else {
            json!({
                "applied": false,
                "skipped": true,
            })
        };

        Ok(json!({
            "action": "create_employee",
            "ok": true,
            "employee": created,
            "profile": profile,
        }))
    }

    async fn update_employee(pool: SqlitePool, input: Value) -> std::result::Result<Value, String> {
        let existing = Self::resolve_employee(&pool, &input, "update_employee").await?;

        let name = Self::parse_optional_string(&input, "name").unwrap_or(existing.name.clone());
        if name.trim().is_empty() {
            return Err("update_employee name 不能为空".to_string());
        }
        let persona =
            Self::parse_optional_string(&input, "persona").unwrap_or(existing.persona.clone());
        let feishu_open_id = Self::parse_optional_string(&input, "feishu_open_id")
            .unwrap_or(existing.feishu_open_id.clone());
        let feishu_app_id =
            Self::parse_optional_string(&input, "feishu_app_id").unwrap_or(existing.feishu_app_id);
        let feishu_app_secret = Self::parse_optional_string(&input, "feishu_app_secret")
            .unwrap_or(existing.feishu_app_secret);
        let default_work_dir = Self::parse_optional_string(&input, "default_work_dir")
            .unwrap_or(existing.default_work_dir.clone());
        let enabled = Self::parse_optional_bool(&input, "enabled").unwrap_or(existing.enabled);
        let is_default =
            Self::parse_optional_bool(&input, "is_default").unwrap_or(existing.is_default);
        let enabled_scopes = if input
            .as_object()
            .is_some_and(|obj| obj.contains_key("enabled_scopes"))
        {
            Self::parse_string_array(&input, "enabled_scopes")
        } else {
            existing.enabled_scopes.clone()
        };

        let mut skill_ids = if input
            .as_object()
            .is_some_and(|obj| obj.contains_key("skill_ids"))
        {
            Self::parse_string_array(&input, "skill_ids")
        } else {
            existing.skill_ids.clone()
        };
        if input
            .as_object()
            .is_some_and(|obj| obj.contains_key("add_skill_ids"))
        {
            skill_ids.extend(Self::parse_string_array(&input, "add_skill_ids"));
        }
        if input
            .as_object()
            .is_some_and(|obj| obj.contains_key("remove_skill_ids"))
        {
            let remove_set = Self::parse_string_array(&input, "remove_skill_ids")
                .into_iter()
                .map(|id| id.to_lowercase())
                .collect::<HashSet<_>>();
            skill_ids.retain(|id| !remove_set.contains(&id.to_lowercase()));
        }
        let mut skill_ids = Self::dedupe_skill_ids(skill_ids);

        let requested_primary_skill_id = input["primary_skill_id"]
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string);
        let mut primary_skill_id = requested_primary_skill_id
            .clone()
            .or_else(|| {
                let current = existing.primary_skill_id.trim();
                if current.is_empty() {
                    None
                } else {
                    Some(current.to_string())
                }
            })
            .or_else(|| skill_ids.first().cloned())
            .unwrap_or_else(|| DEFAULT_PRIMARY_SKILL_ID.to_string());

        if skill_ids.is_empty() {
            skill_ids.push(primary_skill_id.clone());
        }
        if !skill_ids
            .iter()
            .any(|id| id.eq_ignore_ascii_case(primary_skill_id.as_str()))
        {
            if requested_primary_skill_id.is_some() {
                skill_ids.insert(0, primary_skill_id.clone());
            } else {
                primary_skill_id = skill_ids
                    .first()
                    .cloned()
                    .unwrap_or_else(|| DEFAULT_PRIMARY_SKILL_ID.to_string());
            }
        }
        if primary_skill_id.trim().is_empty() {
            primary_skill_id = skill_ids
                .first()
                .cloned()
                .unwrap_or_else(|| DEFAULT_PRIMARY_SKILL_ID.to_string());
        }
        if skill_ids.is_empty() {
            skill_ids.push(primary_skill_id.clone());
        }

        let upsert_input = UpsertAgentEmployeeInput {
            id: Some(existing.id.clone()),
            employee_id: existing.employee_id.clone(),
            name,
            role_id: existing.role_id.clone(),
            persona,
            feishu_open_id,
            feishu_app_id,
            feishu_app_secret,
            primary_skill_id,
            default_work_dir,
            openclaw_agent_id: existing.openclaw_agent_id.clone(),
            routing_priority: 100,
            enabled_scopes,
            enabled,
            is_default,
            skill_ids,
        };

        let updated_id = upsert_agent_employee_with_pool(&pool, upsert_input).await?;
        let employees = list_agent_employees_with_pool(&pool).await?;
        let updated = employees
            .into_iter()
            .find(|item| item.id == updated_id)
            .ok_or_else(|| "更新成功但未找到员工记录".to_string())?;

        let profile_answers = Self::parse_profile_answers(&input);
        let should_apply_profile = input["auto_apply_profile"]
            .as_bool()
            .unwrap_or(!profile_answers.is_empty());
        let profile = if should_apply_profile {
            let payload = AgentProfilePayload {
                employee_db_id: updated_id,
                answers: profile_answers,
            };
            match apply_agent_profile_with_pool(&pool, payload).await {
                Ok(result) => json!({
                    "applied": true,
                    "files": result.files,
                }),
                Err(error) => json!({
                    "applied": false,
                    "error": error,
                }),
            }
        } else {
            json!({
                "applied": false,
                "skipped": true,
            })
        };

        Ok(json!({
            "action": "update_employee",
            "ok": true,
            "employee": updated,
            "profile": profile,
        }))
    }

    async fn apply_profile(pool: SqlitePool, input: Value) -> std::result::Result<Value, String> {
        let employee_db_id = input["employee_db_id"]
            .as_str()
            .map(str::trim)
            .unwrap_or("");
        let employee_id = input["employee_id"].as_str().map(str::trim).unwrap_or("");

        let resolved_db_id = if !employee_db_id.is_empty() {
            employee_db_id.to_string()
        } else if !employee_id.is_empty() {
            let employees = list_agent_employees_with_pool(&pool).await?;
            let matched = employees
                .into_iter()
                .find(|item| {
                    item.id.eq_ignore_ascii_case(employee_id)
                        || item.employee_id.eq_ignore_ascii_case(employee_id)
                        || item.role_id.eq_ignore_ascii_case(employee_id)
                })
                .ok_or_else(|| "apply_profile 未找到对应员工".to_string())?;
            matched.id
        } else {
            return Err("apply_profile 缺少 employee_db_id 或 employee_id 参数".to_string());
        };

        let payload = AgentProfilePayload {
            employee_db_id: resolved_db_id.clone(),
            answers: Self::parse_profile_answers(&input),
        };
        let result = apply_agent_profile_with_pool(&pool, payload).await?;
        Ok(json!({
            "action": "apply_profile",
            "ok": true,
            "employee_db_id": resolved_db_id,
            "files": result.files,
        }))
    }
}

impl Tool for EmployeeManageTool {
    fn name(&self) -> &str {
        "employee_manage"
    }

    fn description(&self) -> &str {
        "员工配置管理工具。支持 list_skills、list_employees、create_employee、update_employee、apply_profile 五种操作。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list_skills", "list_employees", "create_employee", "update_employee", "apply_profile"],
                    "description": "执行动作"
                },
                "id": { "type": "string" },
                "employee_db_id": { "type": "string" },
                "employee_id": { "type": "string" },
                "name": { "type": "string" },
                "persona": { "type": "string" },
                "primary_skill_id": { "type": "string" },
                "skill_ids": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "add_skill_ids": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "remove_skill_ids": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "enabled_scopes": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "enabled": { "type": "boolean" },
                "is_default": { "type": "boolean" },
                "auto_apply_profile": { "type": "boolean" },
                "default_work_dir": { "type": "string" },
                "feishu_open_id": { "type": "string" },
                "feishu_app_id": { "type": "string" },
                "feishu_app_secret": { "type": "string" },
                "profile_answers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "key": { "type": "string" },
                            "question": { "type": "string" },
                            "answer": { "type": "string" }
                        },
                        "required": ["key", "answer"]
                    }
                }
            },
            "required": ["action"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let action = input["action"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 action 参数"))?;
        let payload = match action {
            "list_skills" => self.block_on(Self::list_skills(self.pool.clone()))?,
            "list_employees" => self.block_on(Self::list_employees(self.pool.clone()))?,
            "create_employee" => {
                self.block_on(Self::create_employee(self.pool.clone(), input.clone()))?
            }
            "update_employee" => {
                self.block_on(Self::update_employee(self.pool.clone(), input.clone()))?
            }
            "apply_profile" => {
                self.block_on(Self::apply_profile(self.pool.clone(), input.clone()))?
            }
            _ => return Err(anyhow!("未知 action: {}", action)),
        };
        serde_json::to_string_pretty(&payload).map_err(|e| anyhow!("序列化结果失败: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::path::Path;

    fn setup_pool() -> SqlitePool {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("create runtime");

        rt.block_on(async {
            let db_path = std::env::temp_dir().join(format!(
                "employee-manage-tool-test-{}.db",
                Uuid::new_v4()
            ));
            let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect(&db_url)
                .await
                .expect("create sqlite memory pool");

            sqlx::query(
                "CREATE TABLE installed_skills (
                    id TEXT PRIMARY KEY,
                    manifest TEXT NOT NULL,
                    installed_at TEXT NOT NULL,
                    last_used_at TEXT,
                    username TEXT NOT NULL,
                    pack_path TEXT NOT NULL DEFAULT '',
                    source_type TEXT NOT NULL DEFAULT 'encrypted'
                )",
            )
            .execute(&pool)
            .await
            .expect("create installed_skills");

            sqlx::query(
                "CREATE TABLE agent_employees (
                    id TEXT PRIMARY KEY,
                    employee_id TEXT NOT NULL DEFAULT '',
                    name TEXT NOT NULL,
                    role_id TEXT NOT NULL,
                    persona TEXT NOT NULL DEFAULT '',
                    feishu_open_id TEXT NOT NULL DEFAULT '',
                    feishu_app_id TEXT NOT NULL DEFAULT '',
                    feishu_app_secret TEXT NOT NULL DEFAULT '',
                    primary_skill_id TEXT NOT NULL DEFAULT '',
                    default_work_dir TEXT NOT NULL DEFAULT '',
                    openclaw_agent_id TEXT NOT NULL DEFAULT '',
                    routing_priority INTEGER NOT NULL DEFAULT 100,
                    enabled_scopes_json TEXT NOT NULL DEFAULT '[]',
                    enabled INTEGER NOT NULL DEFAULT 1,
                    is_default INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
            )
            .execute(&pool)
            .await
            .expect("create agent_employees");

            sqlx::query(
                "CREATE TABLE agent_employee_skills (
                    employee_id TEXT NOT NULL,
                    skill_id TEXT NOT NULL,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (employee_id, skill_id)
                )",
            )
            .execute(&pool)
            .await
            .expect("create agent_employee_skills");

            let manifest = json!({
                "id": "builtin-general",
                "name": "通用助手",
                "description": "通用处理能力",
                "version": "1.0.0",
                "author": "WorkClaw",
                "recommended_model": "",
                "tags": [],
                "created_at": "2026-01-01T00:00:00Z",
                "username_hint": null,
                "encrypted_verify": ""
            })
            .to_string();

            sqlx::query(
                "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
                 VALUES ('builtin-general', ?, '2026-01-01T00:00:00Z', '', '', 'builtin')",
            )
            .bind(manifest)
            .execute(&pool)
            .await
            .expect("seed builtin-general");

            pool
        })
    }

    #[test]
    fn employee_manage_lists_skills() {
        let pool = setup_pool();
        let tool = EmployeeManageTool::new(pool);
        let output = tool
            .execute(json!({ "action": "list_skills" }), &ToolContext::default())
            .expect("list skills");
        let payload: Value = serde_json::from_str(&output).expect("parse json");
        assert_eq!(payload["action"], "list_skills");
        assert_eq!(payload["items"][0]["id"], "builtin-general");
    }

    #[test]
    fn employee_manage_creates_employee_and_can_list() {
        let pool = setup_pool();
        let tool = EmployeeManageTool::new(pool);
        let profile_root =
            std::env::temp_dir().join(format!("employee-manage-profile-{}", Uuid::new_v4()));
        let profile_root_text = profile_root.to_string_lossy().to_string();
        let create_output = tool
            .execute(
                json!({
                    "action": "create_employee",
                    "name": "项目经理",
                    "persona": "推进需求交付并协调多技能执行",
                    "primary_skill_id": "builtin-general",
                    "skill_ids": ["builtin-general"],
                    "enabled_scopes": ["app"],
                    "default_work_dir": profile_root_text,
                    "profile_answers": [
                        { "key": "mission", "question": "核心使命", "answer": "推进需求上线交付" },
                        { "key": "tone", "question": "沟通风格", "answer": "结论先行、简洁明确" }
                    ]
                }),
                &ToolContext::default(),
            )
            .expect("create employee");
        let created: Value = serde_json::from_str(&create_output).expect("parse create output");
        assert_eq!(created["action"], "create_employee");
        assert_eq!(created["ok"], true);
        assert_eq!(created["employee"]["name"], "项目经理");
        assert!(created["employee"]["employee_id"]
            .as_str()
            .is_some_and(|v| !v.is_empty()));
        assert_eq!(created["profile"]["applied"], true);
        assert_eq!(
            created["profile"]["files"]
                .as_array()
                .map(|items| items.len())
                .unwrap_or_default(),
            3
        );
        let has_agents = created["profile"]["files"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("AGENTS.md") && Path::new(path).exists())
            })
        });
        assert!(has_agents);

        let list_output = tool
            .execute(
                json!({ "action": "list_employees" }),
                &ToolContext::default(),
            )
            .expect("list employees");
        let listed: Value = serde_json::from_str(&list_output).expect("parse list output");
        assert_eq!(listed["action"], "list_employees");
        assert_eq!(listed["items"][0]["name"], "项目经理");
        let _ = std::fs::remove_dir_all(&profile_root);
    }

    #[test]
    fn employee_manage_can_apply_profile_for_existing_employee() {
        let pool = setup_pool();
        let tool = EmployeeManageTool::new(pool);
        let profile_root =
            std::env::temp_dir().join(format!("employee-manage-apply-profile-{}", Uuid::new_v4()));
        let profile_root_text = profile_root.to_string_lossy().to_string();

        let create_output = tool
            .execute(
                json!({
                    "action": "create_employee",
                    "name": "客服专员",
                    "employee_id": "service_agent",
                    "primary_skill_id": "builtin-general",
                    "default_work_dir": profile_root_text,
                    "auto_apply_profile": false
                }),
                &ToolContext::default(),
            )
            .expect("create employee");
        let created: Value = serde_json::from_str(&create_output).expect("parse create output");
        assert_eq!(created["profile"]["applied"], false);

        let apply_output = tool
            .execute(
                json!({
                    "action": "apply_profile",
                    "employee_id": "service_agent",
                    "profile_answers": [
                        { "key": "mission", "question": "核心使命", "answer": "保障客户问题闭环" },
                        { "key": "boundaries", "question": "边界规则", "answer": "高风险操作需二次确认" }
                    ]
                }),
                &ToolContext::default(),
            )
            .expect("apply profile");
        let applied: Value = serde_json::from_str(&apply_output).expect("parse apply output");
        assert_eq!(applied["action"], "apply_profile");
        assert_eq!(applied["ok"], true);
        assert_eq!(
            applied["files"]
                .as_array()
                .map(|items| items.len())
                .unwrap_or_default(),
            3
        );
        let has_user = applied["files"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("USER.md") && Path::new(path).exists())
            })
        });
        assert!(has_user);
        let _ = std::fs::remove_dir_all(&profile_root);
    }

    #[test]
    fn employee_manage_auto_derives_primary_skill_when_missing() {
        let pool = setup_pool();
        let tool = EmployeeManageTool::new(pool);
        let profile_root =
            std::env::temp_dir().join(format!("employee-manage-auto-primary-{}", Uuid::new_v4()));
        let profile_root_text = profile_root.to_string_lossy().to_string();

        let create_output = tool
            .execute(
                json!({
                    "action": "create_employee",
                    "name": "自动主技能员工",
                    "default_work_dir": profile_root_text,
                    "auto_apply_profile": false
                }),
                &ToolContext::default(),
            )
            .expect("create employee");
        let created: Value = serde_json::from_str(&create_output).expect("parse create output");
        assert_eq!(created["action"], "create_employee");
        assert_eq!(created["employee"]["primary_skill_id"], "builtin-general");
        assert_eq!(
            created["employee"]["skill_ids"]
                .as_array()
                .map(|items| items.len()),
            Some(1)
        );
        assert_eq!(created["employee"]["skill_ids"][0], "builtin-general");
        let _ = std::fs::remove_dir_all(&profile_root);
    }

    #[test]
    fn employee_manage_defaults_enabled_scopes_to_app() {
        let pool = setup_pool();
        let tool = EmployeeManageTool::new(pool);
        let profile_root =
            std::env::temp_dir().join(format!("employee-manage-default-scope-{}", Uuid::new_v4()));
        let profile_root_text = profile_root.to_string_lossy().to_string();

        let create_output = tool
            .execute(
                json!({
                    "action": "create_employee",
                    "name": "默认范围员工",
                    "employee_id": "default_scope_employee",
                    "default_work_dir": profile_root_text,
                    "auto_apply_profile": false
                }),
                &ToolContext::default(),
            )
            .expect("create employee");
        let created: Value = serde_json::from_str(&create_output).expect("parse create output");
        assert_eq!(created["employee"]["enabled_scopes"], json!(["app"]));
        let _ = std::fs::remove_dir_all(&profile_root);
    }

    #[test]
    fn employee_manage_updates_employee_with_skill_deltas() {
        let pool = setup_pool();
        let tool = EmployeeManageTool::new(pool);
        let profile_root =
            std::env::temp_dir().join(format!("employee-manage-update-{}", Uuid::new_v4()));
        let profile_root_text = profile_root.to_string_lossy().to_string();

        let create_output = tool
            .execute(
                json!({
                    "action": "create_employee",
                    "name": "内容运营",
                    "employee_id": "content_creator",
                    "persona": "负责内容生产与发布",
                    "skill_ids": ["builtin-general", "docx-helper"],
                    "default_work_dir": profile_root_text,
                    "auto_apply_profile": false
                }),
                &ToolContext::default(),
            )
            .expect("create employee");
        let created: Value = serde_json::from_str(&create_output).expect("parse create output");
        assert_eq!(created["employee"]["employee_id"], "content_creator");

        let update_output = tool
            .execute(
                json!({
                    "action": "update_employee",
                    "employee_id": "content_creator",
                    "name": "内容专家",
                    "persona": "负责内容策略、素材管理与产出审核",
                    "primary_skill_id": "docx-helper",
                    "add_skill_ids": ["find-skills"],
                    "remove_skill_ids": ["builtin-general"],
                    "enabled": false
                }),
                &ToolContext::default(),
            )
            .expect("update employee");
        let updated: Value = serde_json::from_str(&update_output).expect("parse update output");
        assert_eq!(updated["action"], "update_employee");
        assert_eq!(updated["ok"], true);
        assert_eq!(updated["employee"]["name"], "内容专家");
        assert_eq!(
            updated["employee"]["persona"],
            "负责内容策略、素材管理与产出审核"
        );
        assert_eq!(updated["employee"]["primary_skill_id"], "docx-helper");
        assert_eq!(updated["employee"]["enabled"], false);
        assert_eq!(
            updated["employee"]["skill_ids"],
            json!(["docx-helper", "find-skills"])
        );

        let _ = std::fs::remove_dir_all(&profile_root);
    }

    #[test]
    fn employee_manage_schema_exposes_update_employee_action() {
        let pool = setup_pool();
        let tool = EmployeeManageTool::new(pool);
        let schema = tool.input_schema();
        let actions = schema["properties"]["action"]["enum"]
            .as_array()
            .expect("action enum should be array");
        assert!(
            actions
                .iter()
                .any(|item| item.as_str().is_some_and(|v| v == "update_employee")),
            "update_employee should be exposed in employee_manage schema"
        );
    }

    #[test]
    fn employee_manage_schema_hides_routing_priority() {
        let pool = setup_pool();
        let tool = EmployeeManageTool::new(pool);
        let schema = tool.input_schema();
        assert!(
            schema["properties"].get("routing_priority").is_none(),
            "routing priority should not be exposed in employee_manage schema"
        );
    }
}
