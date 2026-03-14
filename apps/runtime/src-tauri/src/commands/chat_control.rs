use super::chat::{AskUserState, CancelFlagState, ToolConfirmState};
use tauri::State;

/// 用户回答 AskUser 工具的问题
#[tauri::command]
pub async fn answer_user_question(
    answer: String,
    ask_user_state: State<'_, AskUserState>,
) -> Result<(), String> {
    let guard = ask_user_state
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?;

    if let Some(sender) = guard.as_ref() {
        sender
            .send(answer)
            .map_err(|e| format!("发送响应失败: {}", e))?;
        Ok(())
    } else {
        Err("没有等待中的用户问题".to_string())
    }
}

/// 用户确认或拒绝工具执行
#[tauri::command]
pub async fn confirm_tool_execution(
    confirmed: bool,
    tool_confirm_state: State<'_, ToolConfirmState>,
) -> Result<(), String> {
    let guard = tool_confirm_state
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?;
    if let Some(sender) = guard.as_ref() {
        sender
            .send(confirmed)
            .map_err(|e| format!("发送确认失败: {}", e))?;
        Ok(())
    } else {
        Err("没有等待中的工具确认请求".to_string())
    }
}

/// 取消正在执行的 Agent
#[tauri::command]
pub async fn cancel_agent(cancel_flag: State<'_, CancelFlagState>) -> Result<(), String> {
    cancel_flag
        .0
        .store(true, std::sync::atomic::Ordering::SeqCst);
    eprintln!("[agent] 收到取消信号");
    Ok(())
}
