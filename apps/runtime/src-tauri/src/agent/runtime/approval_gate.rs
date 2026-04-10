use crate::agent::approval_flow::{
    request_tool_approval_and_wait, resolve_approval_wait_runtime, wait_for_tool_confirmation,
    ToolConfirmationDecision, TOOL_CONFIRM_TIMEOUT_SECS,
};
use crate::agent::types::ToolCall;
use crate::approval_bus::{approval_bus_rollout_enabled_with_pool, ApprovalDecision};
use crate::approval_rules::find_matching_approval_rule_with_pool;
use crate::session_journal::{SessionRunTaskContinuationSnapshot, SessionRunTaskIdentitySnapshot};
use anyhow::Result;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

pub(crate) async fn gate_tool_approval(
    app_handle: Option<&AppHandle>,
    session_id: Option<&str>,
    persisted_run_id: Option<&str>,
    task_identity: Option<SessionRunTaskIdentitySnapshot>,
    task_continuation: Option<SessionRunTaskContinuationSnapshot>,
    call: &ToolCall,
    work_dir: Option<&Path>,
    tool_confirm_tx: Option<&Arc<Mutex<Option<Sender<bool>>>>>,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> Result<Option<ApprovalDecision>> {
    if let (Some(app), Some(sid)) = (app_handle, session_id) {
        let runtime = resolve_approval_wait_runtime(app)?;
        let approval_bus_enabled = approval_bus_rollout_enabled_with_pool(&runtime.pool)
            .await
            .unwrap_or(true);

        if approval_bus_enabled {
            match find_matching_approval_rule_with_pool(&runtime.pool, &call.name, &call.input)
                .await
            {
                Ok(Some(_)) => Ok(Some(ApprovalDecision::AllowAlways)),
                Ok(None) | Err(_) => request_tool_approval_and_wait(
                    &runtime,
                    Some(app),
                    sid,
                    persisted_run_id,
                    task_identity,
                    task_continuation,
                    &call.name,
                    &call.id,
                    &call.input,
                    work_dir,
                    cancel_flag,
                )
                .await
                .map(Some),
            }
        } else {
            Ok(resolve_manual_confirmation(tool_confirm_tx)?)
        }
    } else {
        Ok(resolve_manual_confirmation(tool_confirm_tx)?)
    }
}

fn resolve_manual_confirmation(
    tool_confirm_tx: Option<&Arc<Mutex<Option<Sender<bool>>>>>,
) -> Result<Option<ApprovalDecision>> {
    let Some(confirm_state) = tool_confirm_tx else {
        return Ok(Some(ApprovalDecision::AllowOnce));
    };

    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    if let Ok(mut guard) = confirm_state.lock() {
        *guard = Some(tx);
    }

    let confirmation = wait_for_tool_confirmation(
        &rx,
        std::time::Duration::from_secs(TOOL_CONFIRM_TIMEOUT_SECS),
    );

    if let Ok(mut guard) = confirm_state.lock() {
        *guard = None;
    }

    let decision = match confirmation {
        ToolConfirmationDecision::Confirmed => Some(ApprovalDecision::AllowOnce),
        ToolConfirmationDecision::Rejected => Some(ApprovalDecision::Deny),
        ToolConfirmationDecision::TimedOut => None,
    };
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::resolve_manual_confirmation;
    use crate::approval_bus::ApprovalDecision;
    use std::sync::mpsc::Sender;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    fn drive_manual_confirmation(
        confirm_state: Arc<Mutex<Option<Sender<bool>>>>,
        decision: bool,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            for _ in 0..50 {
                let sender = {
                    let guard = confirm_state.lock().expect("lock confirm state");
                    guard.as_ref().cloned()
                };
                if let Some(sender) = sender {
                    sender.send(decision).expect("send decision");
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            panic!("manual confirmation sender was not installed");
        })
    }

    #[test]
    fn manual_confirmation_true_is_allowed_once() {
        let confirm_state = Arc::new(Mutex::new(None));
        let driver = drive_manual_confirmation(Arc::clone(&confirm_state), true);

        let decision = resolve_manual_confirmation(Some(&confirm_state))
            .expect("manual confirmation")
            .expect("decision");

        driver.join().expect("driver thread");
        assert_eq!(decision, ApprovalDecision::AllowOnce);
    }

    #[test]
    fn manual_confirmation_false_is_rejected() {
        let confirm_state = Arc::new(Mutex::new(None));
        let driver = drive_manual_confirmation(Arc::clone(&confirm_state), false);

        let decision = resolve_manual_confirmation(Some(&confirm_state))
            .expect("manual confirmation")
            .expect("decision");

        driver.join().expect("driver thread");
        assert_eq!(decision, ApprovalDecision::Deny);
    }
}
