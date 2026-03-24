use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::Value;
use sqlx::SqlitePool;

#[path = "employee_manage/support.rs"]
mod support;

#[path = "employee_manage/actions.rs"]
mod actions;

#[path = "employee_manage/schema.rs"]
mod schema;

pub struct EmployeeManageTool {
    pool: SqlitePool,
}

impl EmployeeManageTool {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
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
        actions::list_skills(pool).await
    }

    async fn list_employees(pool: SqlitePool) -> std::result::Result<Value, String> {
        actions::list_employees(pool).await
    }

    async fn create_employee(pool: SqlitePool, input: Value) -> std::result::Result<Value, String> {
        actions::create_employee(pool, input).await
    }

    async fn update_employee(pool: SqlitePool, input: Value) -> std::result::Result<Value, String> {
        actions::update_employee(pool, input).await
    }

    async fn apply_profile(pool: SqlitePool, input: Value) -> std::result::Result<Value, String> {
        actions::apply_profile(pool, input).await
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
        schema::input_schema()
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
#[path = "employee_manage/tests.rs"]
mod tests;
