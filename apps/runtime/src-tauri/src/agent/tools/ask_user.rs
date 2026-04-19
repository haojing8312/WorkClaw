use crate::agent::types::{Tool, ToolContext};
use crate::commands::im_host::{
    maybe_emit_registered_host_lifecycle_phase_for_session_with_pool,
    maybe_notify_registered_ask_user_requested_with_pool,
};
use crate::commands::openclaw_plugins::im_host_contract::ImReplyLifecyclePhase;
use crate::commands::skills::DbState;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::{mpsc, Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

/// AskUser 响应通道 - 前端通过 Tauri command 发送用户响应
pub type AskUserResponder = Arc<Mutex<Option<mpsc::Sender<String>>>>;

/// 创建新的 AskUser 响应通道
pub fn new_responder() -> AskUserResponder {
    Arc::new(Mutex::new(None))
}

/// 交互式用户问答工具
///
/// 当 Agent 需要用户输入时调用此工具。
/// 执行时发送事件到前端，阻塞等待用户响应。
pub struct AskUserTool {
    app_handle: AppHandle,
    session_id: String,
    responder: AskUserResponder,
    pending_session: Arc<Mutex<Option<String>>>,
}

#[derive(serde::Serialize, Clone, Debug)]
struct AskUserEvent {
    session_id: String,
    question: String,
    options: Vec<String>,
}

impl AskUserTool {
    pub fn new(
        app_handle: AppHandle,
        session_id: String,
        responder: AskUserResponder,
        pending_session: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            app_handle,
            session_id,
            responder,
            pending_session,
        }
    }
}

impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn description(&self) -> &str {
        "向用户提问并等待回答。当需要用户确认、选择或提供信息时使用。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "要问用户的问题"
                },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "可选的预设选项列表"
                }
            },
            "required": ["question"]
        })
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let question = input["question"]
            .as_str()
            .ok_or(anyhow!("缺少 question 参数"))?
            .to_string();

        let options: Vec<String> = input["options"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // 创建响应通道
        let (tx, rx) = mpsc::channel();

        // 设置 responder 以便前端 command 可以发送响应
        {
            let mut guard = self
                .responder
                .lock()
                .map_err(|e| anyhow!("锁获取失败: {}", e))?;
            *guard = Some(tx);
        }
        {
            let mut guard = self
                .pending_session
                .lock()
                .map_err(|e| anyhow!("锁获取失败: {}", e))?;
            *guard = Some(self.session_id.clone());
        }

        // 发送事件到前端
        self.app_handle
            .emit(
                "ask-user-event",
                AskUserEvent {
                    session_id: self.session_id.clone(),
                    question: question.clone(),
                    options: options.clone(),
                },
            )
            .map_err(|e| anyhow!("事件发送失败: {}", e))?;

        if let Some(db_state) = self.app_handle.try_state::<DbState>() {
            let result = tauri::async_runtime::block_on(
                maybe_notify_registered_ask_user_requested_with_pool(
                    &db_state.0,
                    &self.session_id,
                    &question,
                    &options,
                    None,
                ),
            );
            if let Err(error) = result {
                eprintln!("[agent] AskUser: IM 宿主发问失败: {}", error);
            }
        }

        eprintln!("[agent] AskUser: 等待用户回答 \"{}\"", question);

        // 阻塞等待用户响应（最多 5 分钟）
        let response = rx
            .recv_timeout(std::time::Duration::from_secs(300))
            .map_err(|_| anyhow!("等待用户响应超时（5 分钟）"))?;

        // 清理 responder
        {
            let mut guard = self
                .responder
                .lock()
                .map_err(|e| anyhow!("锁获取失败: {}", e))?;
            *guard = None;
        }
        {
            let mut guard = self
                .pending_session
                .lock()
                .map_err(|e| anyhow!("锁获取失败: {}", e))?;
            *guard = None;
        }
        if let Some(db_state) = self.app_handle.try_state::<DbState>() {
            let _ = tauri::async_runtime::block_on(
                maybe_emit_registered_host_lifecycle_phase_for_session_with_pool(
                    &db_state.0,
                    &self.session_id,
                    None,
                    ImReplyLifecyclePhase::Resumed,
                    None,
                ),
            );
        }

        eprintln!("[agent] AskUser: 收到用户回答: {}", response);
        Ok(response)
    }
}
