use crate::agent::permissions::PermissionMode;
use crate::agent::run_guard::{parse_run_stop_reason, RunStopReasonKind};
use crate::agent::types::{StreamDelta, Tool, ToolContext};
use crate::agent::{AgentExecutor, ToolRegistry};
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
        }
    }

    /// 设置 AppHandle 和 session_id，启用子 Agent 流式输出转发
    pub fn with_app_handle(mut self, app: tauri::AppHandle, session_id: String) -> Self {
        self.app_handle = Some(app);
        self.session_id = Some(session_id);
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
        let (allowed_tools, max_iter): (Option<Vec<String>>, usize) = match agent_type.as_str() {
            "explore" => (Some(Self::get_explore_tools()), 15),
            "plan" => (Some(Self::get_plan_tools()), 20),
            _ => (None, 30), // general-purpose: 全部工具
        };

        // 在闭包外保留副本，用于之后的格式化输出
        let agent_type_display = agent_type.clone();

        let registry = Arc::clone(&self.registry);
        let api_format = self.api_format.clone();
        let base_url = self.base_url.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let delegated_role_id = delegate_role_id.clone();
        let delegated_role_name = delegate_role_name.clone();
        let delegated_display = delegate_display_name.clone();

        // 在线程 spawn 前克隆，避免所有权冲突
        let sub_app_handle = self.app_handle.clone();
        let sub_session_id = self.session_id.clone();

        // 必须在新线程中创建新的 tokio runtime，否则会死锁
        // （Tool::execute 是同步的，但被 async 上下文调用，不能用 block_on）
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| anyhow!("创建运行时失败: {}", e))?;

            rt.block_on(async {
                let sub_executor = AgentExecutor::with_max_iterations(registry, max_iter);

                let system_prompt = format!(
                    "你是一个专注的子 Agent (类型: {})，当前承接角色: {}。完成以下任务后返回结果。简洁地报告你的发现。",
                    agent_type, delegated_display
                );

                let messages = vec![json!({"role": "user", "content": prompt})];

                // 根据是否配置了 AppHandle 决定是否将 token 转发到前端。
                // execute_turn 要求 impl Fn(String) + Send + Clone。
                // 用 Arc<dyn Fn + Send + Sync> 包装回调，再用外层闭包按值捕获 Arc（Arc: Clone）
                // 从而满足 Clone 约束。
                let on_token_arc: Arc<dyn Fn(String) + Send + Sync> =
                    match (&sub_app_handle, &sub_session_id) {
                        (Some(app), Some(sid)) => {
                            let app = app.clone();
                            let sid = sid.clone();
                            let role_id = delegated_role_id.clone();
                            let role_name = delegated_role_name.clone();
                            Arc::new(move |token: String| {
                                let _ = app.emit(
                                    "stream-token",
                                    json!({
                                        "session_id": sid,
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
                // 将 Arc 包装成满足 Clone 的普通闭包
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
                        sub_app_handle.as_ref(),
                        sub_session_id.as_deref(),
                        allowed_tools.as_deref(),
                        PermissionMode::Unrestricted, // 子 Agent 不需要权限确认
                        None,                         // 无确认通道
                        None,                         // work_dir: 子 Agent 继承主 Agent 设置
                        None,                         // 使用 sub_executor 默认迭代限制
                        None,                         // 子 Agent 无取消标志
                        None,                         // 子 Agent 使用默认节点超时
                        None,                         // 子 Agent 默认不重试
                    )
                    .await
            })
        })
        .join()
        .map_err(|_| anyhow!("子 Agent 线程异常"))?;

        match handle {
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
