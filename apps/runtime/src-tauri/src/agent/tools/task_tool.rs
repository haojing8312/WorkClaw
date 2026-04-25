use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{parse_run_stop_reason, RunStopReasonKind};
use crate::agent::runtime::child_session_runtime::{
    run_hidden_child_session, ChildSessionRunRequest,
};
use crate::agent::types::{StreamDelta, Tool, ToolContext};
use crate::agent::{AgentExecutor, ToolRegistry};
use crate::session_journal::SessionJournalStore;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::Emitter;

/// 子 Agent 分发工具
///
/// 允许主 Agent 将独立子任务分发给子 Agent 执行，子 Agent 拥有独立上下文。
/// 支持三种类型：
/// - `explore`：只读操作（read_file、glob、grep）
/// - `plan`：只读 + bash
/// - `general-purpose`：全部工具
pub struct TaskTool {
    registry: Arc<ToolRegistry>,
    api_format: String,
    base_url: String,
    api_key: String,
    model: String,
    app_handle: Option<tauri::AppHandle>,
    session_id: Option<String>,
    db: Option<sqlx::SqlitePool>,
    journal: Option<Arc<SessionJournalStore>>,
}

impl TaskTool {
    pub fn new(
        registry: Arc<ToolRegistry>,
        api_format: String,
        base_url: String,
        api_key: String,
        model: String,
    ) -> Self {
        Self {
            registry,
            api_format,
            base_url,
            api_key,
            model,
            app_handle: None,
            session_id: None,
            db: None,
            journal: None,
        }
    }

    /// 设置 AppHandle 和 session_id，启用子 Agent 流式输出转发
    pub fn with_app_handle(mut self, app: tauri::AppHandle, session_id: String) -> Self {
        self.app_handle = Some(app);
        self.session_id = Some(session_id);
        self
    }

    pub fn with_runtime_state(
        mut self,
        db: sqlx::SqlitePool,
        journal: Arc<SessionJournalStore>,
    ) -> Self {
        self.db = Some(db);
        self.journal = Some(journal);
        self
    }

    /// explore 类型：只读工具列表
    pub fn get_explore_tools() -> Vec<String> {
        vec![
            "read_file".to_string(),
            "glob".to_string(),
            "grep".to_string(),
        ]
    }

    /// plan 类型：只读 + bash 工具列表
    pub fn get_plan_tools() -> Vec<String> {
        vec![
            "read_file".to_string(),
            "glob".to_string(),
            "grep".to_string(),
            "bash".to_string(),
        ]
    }

    fn budget_for_agent_type(agent_type: &str) -> (Option<Vec<String>>, usize) {
        match agent_type {
            "explore" => (Some(Self::get_explore_tools()), 100),
            "plan" => (Some(Self::get_plan_tools()), 100),
            _ => (None, 100), // general-purpose: 全部工具
        }
    }

    async fn execute_direct_subagent(
        registry: Arc<ToolRegistry>,
        api_format: String,
        base_url: String,
        api_key: String,
        model: String,
        prompt: String,
        agent_type: String,
        delegate_display_name: String,
        delegate_role_id: String,
        delegate_role_name: String,
        app_handle: Option<tauri::AppHandle>,
        session_id: Option<String>,
        allowed_tools: Option<Vec<String>>,
        max_iter: usize,
        work_dir: Option<String>,
    ) -> Result<Vec<Value>> {
        let sub_executor = AgentExecutor::with_max_iterations(registry, max_iter);
        let system_prompt = format!(
            "你是一个专注的子 Agent (类型: {})，当前承接角色: {}。完成以下任务后返回结果。简洁地报告你的发现。",
            agent_type, delegate_display_name
        );
        let messages = vec![json!({"role": "user", "content": prompt})];

        let on_token_arc: Arc<dyn Fn(String) + Send + Sync> = match (&app_handle, &session_id) {
            (Some(app), Some(parent_session_id)) => {
                let app = app.clone();
                let parent_session_id = parent_session_id.clone();
                let role_id = delegate_role_id.clone();
                let role_name = delegate_role_name.clone();
                Arc::new(move |token: String| {
                    let _ = app.emit(
                        "stream-token",
                        json!({
                            "session_id": parent_session_id,
                            "token": token,
                            "done": false,
                            "sub_agent": true,
                            "role_id": role_id,
                            "role_name": role_name,
                        }),
                    );
                })
            }
            _ => Arc::new(|_| {}),
        };
        let on_token = move |delta: StreamDelta| {
            if let StreamDelta::Text(token) = delta {
                on_token_arc(token);
            }
        };

        sub_executor
            .execute_turn(
                &api_format,
                &base_url,
                &api_key,
                &model,
                &system_prompt,
                messages,
                on_token,
                app_handle.as_ref(),
                session_id.as_deref(),
                allowed_tools.as_deref(),
                PermissionMode::Unrestricted,
                None,
                work_dir,
                Some(max_iter),
                None,
                None,
                None,
            )
            .await
    }
}

impl Tool for TaskTool {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "分发子 Agent 执行独立任务。子 Agent 拥有独立上下文，完成后返回结果。支持 explore（只读）、plan（只读+bash）、general-purpose（全部工具）三种类型。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "子 Agent 的任务描述"
                },
                "agent_type": {
                    "type": "string",
                    "enum": ["general-purpose", "explore", "plan"],
                    "description": "子 Agent 类型（默认 general-purpose）"
                },
                "delegate_role_id": {
                    "type": "string",
                    "description": "可选：委托的目标员工/角色ID（用于多员工协作标注）"
                },
                "delegate_role_name": {
                    "type": "string",
                    "description": "可选：委托的目标员工显示名（用于桌面端与飞书同步区分）"
                }
            },
            "required": ["prompt"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let prompt = input["prompt"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 prompt 参数"))?
            .to_string();
        let agent_type = input["agent_type"]
            .as_str()
            .unwrap_or("general-purpose")
            .to_string();
        let delegate_role_id = input["delegate_role_id"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();
        let delegate_role_name = input["delegate_role_name"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();
        let delegate_display_name = if !delegate_role_name.is_empty() {
            delegate_role_name.clone()
        } else if !delegate_role_id.is_empty() {
            delegate_role_id.clone()
        } else {
            "子智能体".to_string()
        };

        // 根据类型确定工具白名单和迭代限制
        let (allowed_tools, max_iter): (Option<Vec<String>>, usize) =
            Self::budget_for_agent_type(agent_type.as_str());

        // 在闭包外保留副本，用于之后的格式化输出
        let agent_type_display = agent_type.clone();

        let runtime =
            tokio::runtime::Runtime::new().map_err(|e| anyhow!("创建运行时失败: {}", e))?;
        let task_result = if let (Some(db), Some(journal), Some(parent_session_id)) = (
            self.db.as_ref(),
            self.journal.as_ref(),
            self.session_id.as_deref(),
        ) {
            runtime
                .block_on(run_hidden_child_session(ChildSessionRunRequest {
                    parent_session_id,
                    prompt: &prompt,
                    agent_type: &agent_type,
                    delegate_display_name: &delegate_display_name,
                    registry: Arc::clone(&self.registry),
                    db,
                    journal,
                    api_format: &self.api_format,
                    base_url: &self.base_url,
                    api_key: &self.api_key,
                    model: &self.model,
                    allowed_tools: allowed_tools.clone(),
                    max_iterations: max_iter,
                    app_handle: self.app_handle.as_ref(),
                    parent_stream_session_id: self.session_id.as_deref(),
                    delegate_role_id: Some(delegate_role_id.as_str()),
                    delegate_role_name: Some(delegate_role_name.as_str()),
                    work_dir: _ctx
                        .work_dir
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string()),
                }))
                .map(|outcome| {
                    vec![json!({
                        "role": "assistant",
                        "content": outcome.final_text,
                    })]
                })
        } else {
            runtime.block_on(Self::execute_direct_subagent(
                Arc::clone(&self.registry),
                self.api_format.clone(),
                self.base_url.clone(),
                self.api_key.clone(),
                self.model.clone(),
                prompt.clone(),
                agent_type.clone(),
                delegate_display_name.clone(),
                delegate_role_id.clone(),
                delegate_role_name.clone(),
                self.app_handle.clone(),
                self.session_id.clone(),
                allowed_tools.clone(),
                max_iter,
                _ctx.work_dir
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
            ))
        };

        match task_result {
            Ok(final_messages) => {
                // 提取最后一条 assistant 消息
                let last_text = final_messages
                    .iter()
                    .rev()
                    .find_map(|m| {
                        if m["role"].as_str() == Some("assistant") {
                            m["content"].as_str().map(String::from)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "子 Agent 未返回文本结果".to_string());

                Ok(format!(
                    "子 Agent ({}, 角色: {}) 执行完成:\n\n{}",
                    agent_type_display, delegate_display_name, last_text
                ))
            }
            Err(e) => {
                let err_str = e.to_string();
                let stop_reason = parse_run_stop_reason(&err_str);
                if stop_reason
                    .as_ref()
                    .map(|reason| reason.kind == RunStopReasonKind::MaxTurns)
                    .unwrap_or_else(|| err_str.contains("最大迭代次数"))
                {
                    Ok(format!(
                        "子 Agent ({}, 角色: {}) 达到最大迭代次数 ({}):\n\n最后状态: 未完成",
                        agent_type_display, delegate_display_name, max_iter
                    ))
                } else {
                    Err(anyhow!("子 Agent 执行失败: {}", err_str))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TaskTool;

    #[test]
    fn task_tool_uses_100_turn_budget_for_explore_agents() {
        let (allowed_tools, max_iter) = TaskTool::budget_for_agent_type("explore");
        assert_eq!(max_iter, 100);
        assert_eq!(
            allowed_tools,
            Some(vec![
                "read_file".to_string(),
                "glob".to_string(),
                "grep".to_string()
            ])
        );
    }

    #[test]
    fn task_tool_uses_100_turn_budget_for_plan_agents() {
        let (allowed_tools, max_iter) = TaskTool::budget_for_agent_type("plan");
        assert_eq!(max_iter, 100);
        assert_eq!(
            allowed_tools,
            Some(vec![
                "read_file".to_string(),
                "glob".to_string(),
                "grep".to_string(),
                "bash".to_string()
            ])
        );
    }

    #[test]
    fn task_tool_uses_100_turn_budget_for_general_purpose_agents() {
        let (allowed_tools, max_iter) = TaskTool::budget_for_agent_type("general-purpose");
        assert_eq!(max_iter, 100);
        assert_eq!(allowed_tools, None);
    }
}
