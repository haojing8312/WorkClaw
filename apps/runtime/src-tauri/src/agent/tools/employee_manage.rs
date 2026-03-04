use crate::agent::types::{Tool, ToolContext};
use crate::commands::employee_agents::{
    list_agent_employees_with_pool, upsert_agent_employee_with_pool, UpsertAgentEmployeeInput,
};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use sqlx::SqlitePool;
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

        let primary_skill_id = input["primary_skill_id"]
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(DEFAULT_PRIMARY_SKILL_ID)
            .to_string();

        let default_work_dir = input["default_work_dir"]
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| Self::default_employee_work_dir(&employee_id));

        let input = UpsertAgentEmployeeInput {
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
            routing_priority: input["routing_priority"].as_i64().unwrap_or(100),
            enabled_scopes: {
                let scopes = Self::parse_string_array(&input, "enabled_scopes");
                if scopes.is_empty() {
                    vec!["feishu".to_string()]
                } else {
                    scopes
                }
            },
            enabled: input["enabled"].as_bool().unwrap_or(true),
            is_default: input["is_default"].as_bool().unwrap_or(false),
            skill_ids: Self::parse_string_array(&input, "skill_ids"),
        };

        let created_id = upsert_agent_employee_with_pool(&pool, input).await?;
        let employees = list_agent_employees_with_pool(&pool).await?;
        let created = employees
            .into_iter()
            .find(|item| item.id == created_id)
            .ok_or_else(|| "创建成功但未找到员工记录".to_string())?;

        Ok(json!({
            "action": "create_employee",
            "ok": true,
            "employee": created,
        }))
    }
}

impl Tool for EmployeeManageTool {
    fn name(&self) -> &str {
        "employee_manage"
    }

    fn description(&self) -> &str {
        "员工配置管理工具。支持 list_skills、list_employees、create_employee 三种操作。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list_skills", "list_employees", "create_employee"],
                    "description": "执行动作"
                },
                "id": { "type": "string" },
                "employee_id": { "type": "string" },
                "name": { "type": "string" },
                "persona": { "type": "string" },
                "primary_skill_id": { "type": "string" },
                "skill_ids": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "enabled_scopes": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "routing_priority": { "type": "integer" },
                "enabled": { "type": "boolean" },
                "is_default": { "type": "boolean" },
                "default_work_dir": { "type": "string" },
                "feishu_open_id": { "type": "string" },
                "feishu_app_id": { "type": "string" },
                "feishu_app_secret": { "type": "string" }
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
            _ => return Err(anyhow!("未知 action: {}", action)),
        };
        serde_json::to_string_pretty(&payload).map_err(|e| anyhow!("序列化结果失败: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

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
        let create_output = tool
            .execute(
                json!({
                    "action": "create_employee",
                    "name": "项目经理",
                    "persona": "推进需求交付并协调多技能执行",
                    "primary_skill_id": "builtin-general",
                    "skill_ids": ["builtin-general"],
                    "enabled_scopes": ["feishu"]
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

        let list_output = tool
            .execute(
                json!({ "action": "list_employees" }),
                &ToolContext::default(),
            )
            .expect("list employees");
        let listed: Value = serde_json::from_str(&list_output).expect("parse list output");
        assert_eq!(listed["action"], "list_employees");
        assert_eq!(listed["items"][0]["name"], "项目经理");
    }
}
