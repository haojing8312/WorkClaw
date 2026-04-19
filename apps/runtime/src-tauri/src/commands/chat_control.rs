use super::approvals::resolve_approval;
use super::chat::{
    ApprovalManagerState, AskUserPendingSessionState, AskUserState, CancelFlagState,
    PendingApprovalBridgeState, ToolConfirmState,
};
use super::im_host::maybe_emit_registered_host_lifecycle_phase_for_session_with_pool;
use super::skills::DbState;
use crate::approval_bus::ApprovalDecision;
use crate::commands::openclaw_plugins::im_host_contract::ImReplyLifecyclePhase;
use tauri::{Manager, State};

/// 用户回答 AskUser 工具的问题
#[tauri::command]
pub async fn answer_user_question(
    answer: String,
    app: tauri::AppHandle,
    ask_user_state: State<'_, AskUserState>,
    ask_user_pending_session: State<'_, AskUserPendingSessionState>,
) -> Result<(), String> {
    let pending_session_id = ask_user_pending_session
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?
        .clone();
    let sender = ask_user_state
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?
        .as_ref()
        .cloned();

    if let Some(sender) = sender {
        sender
            .send(answer)
            .map_err(|e| format!("发送响应失败: {}", e))?;
        if let Some(session_id) = pending_session_id {
            if let Some(db_state) = app.try_state::<DbState>() {
                let _ = maybe_emit_registered_host_lifecycle_phase_for_session_with_pool(
                    &db_state.0,
                    &session_id,
                    None,
                    ImReplyLifecyclePhase::AskUserAnswered,
                    None,
                )
                .await;
            }
        }
        Ok(())
    } else {
        Err("没有等待中的用户问题".to_string())
    }
}

/// 用户确认或拒绝工具执行
#[tauri::command]
pub async fn confirm_tool_execution(
    confirmed: bool,
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    approvals: State<'_, ApprovalManagerState>,
    pending_approval_bridge: State<'_, PendingApprovalBridgeState>,
    tool_confirm_state: State<'_, ToolConfirmState>,
) -> Result<(), String> {
    let pending_approval_id = pending_approval_bridge
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?
        .clone();

    if let Some(approval_id) = pending_approval_id {
        resolve_approval(
            app,
            approval_id,
            if confirmed {
                ApprovalDecision::AllowOnce
            } else {
                ApprovalDecision::Deny
            },
            "desktop_legacy".to_string(),
            Some("desktop_legacy".to_string()),
            db,
            approvals,
            pending_approval_bridge,
        )
        .await?;
        return Ok(());
    }

    let sender = tool_confirm_state
        .0
        .lock()
        .map_err(|e| format!("锁获取失败: {}", e))?
        .as_ref()
        .cloned();
    if let Some(sender) = sender {
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
pub async fn cancel_agent(
    session_id: Option<String>,
    app: tauri::AppHandle,
    cancel_flag: State<'_, CancelFlagState>,
) -> Result<(), String> {
    if let Some(session_id) = session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(db_state) = app.try_state::<DbState>() {
            let _ = maybe_emit_registered_host_lifecycle_phase_for_session_with_pool(
                &db_state.0,
                session_id,
                None,
                ImReplyLifecyclePhase::InterruptRequested,
                None,
            )
            .await;
        }
    }
    cancel_flag
        .0
        .store(true, std::sync::atomic::Ordering::SeqCst);
    eprintln!("[agent] 收到取消信号");
    Ok(())
}
