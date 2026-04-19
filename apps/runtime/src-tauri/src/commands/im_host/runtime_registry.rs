use std::collections::HashMap;
use std::io::Write;
use std::process::ChildStdin;
use std::sync::{Arc, Mutex};

pub(crate) type PendingRuntimeRequestMap<T> =
    HashMap<String, std::sync::mpsc::SyncSender<Result<T, String>>>;

pub(crate) fn write_runtime_command_json(
    stdin: &Arc<Mutex<ChildStdin>>,
    payload_json: &str,
) -> Result<(), String> {
    let mut stdin_guard = stdin
        .lock()
        .map_err(|_| "failed to lock runtime stdin".to_string())?;
    stdin_guard
        .write_all(payload_json.as_bytes())
        .and_then(|_| stdin_guard.write_all(b"\n"))
        .and_then(|_| stdin_guard.flush())
        .map_err(|error| format!("failed to write runtime command: {error}"))
}

pub(crate) fn ensure_runtime_stdin_for_commands(
    running: bool,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    runtime_name: &str,
) -> Result<Arc<Mutex<ChildStdin>>, String> {
    if !running {
        return Err(format!("{runtime_name} is not running"));
    }

    stdin.ok_or_else(|| format!("{runtime_name} is not accepting outbound commands"))
}

pub(crate) fn register_pending_runtime_request<T>(
    pending_requests: &mut PendingRuntimeRequestMap<T>,
    request_id: &str,
) -> Result<std::sync::mpsc::Receiver<Result<T, String>>, String> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    if pending_requests.contains_key(request_id) {
        return Err(format!("duplicate outbound requestId: {request_id}"));
    }
    pending_requests.insert(request_id.to_string(), sender);
    Ok(receiver)
}

pub(crate) fn resolve_pending_runtime_request<T>(
    pending_requests: &mut PendingRuntimeRequestMap<T>,
    request_id: &str,
    result: Result<T, String>,
) -> bool {
    let Some(sender) = pending_requests.remove(request_id) else {
        return false;
    };
    sender.send(result).is_ok()
}

pub(crate) fn fail_pending_runtime_requests<T>(
    pending_requests: &mut PendingRuntimeRequestMap<T>,
    error: String,
) -> usize {
    let senders = std::mem::take(pending_requests);
    let mut count = 0;
    for (_request_id, sender) in senders {
        let _ = sender.send(Err(error.clone()));
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_runtime_stdin_for_commands, fail_pending_runtime_requests,
        register_pending_runtime_request, resolve_pending_runtime_request,
        PendingRuntimeRequestMap,
    };
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};

    #[test]
    fn pending_runtime_request_round_trip_succeeds() {
        let mut pending = PendingRuntimeRequestMap::<String>::new();
        let receiver =
            register_pending_runtime_request(&mut pending, "request-1").expect("register");
        let delivered =
            resolve_pending_runtime_request(&mut pending, "request-1", Ok("done".to_string()));

        assert!(delivered);
        assert!(pending.is_empty());
        assert_eq!(
            receiver.recv().expect("receive result").expect("success"),
            "done"
        );
    }

    #[test]
    fn failing_pending_runtime_requests_drains_registry() {
        let mut pending = PendingRuntimeRequestMap::<String>::new();
        let receiver_a =
            register_pending_runtime_request(&mut pending, "request-a").expect("register a");
        let receiver_b =
            register_pending_runtime_request(&mut pending, "request-b").expect("register b");

        let failed =
            fail_pending_runtime_requests(&mut pending, "runtime disconnected".to_string());

        assert_eq!(failed, 2);
        assert!(pending.is_empty());
        assert!(receiver_a
            .recv()
            .expect("receive a")
            .expect_err("a should fail")
            .contains("runtime disconnected"));
        assert!(receiver_b
            .recv()
            .expect("receive b")
            .expect_err("b should fail")
            .contains("runtime disconnected"));
    }

    #[test]
    fn ensure_runtime_stdin_rejects_stopped_runtime() {
        let error = ensure_runtime_stdin_for_commands(true, None, "test runtime")
            .expect_err("missing stdin should fail");
        assert!(error.contains("not accepting outbound commands"));
        let error = ensure_runtime_stdin_for_commands(false, None, "test runtime")
            .expect_err("stopped runtime should fail");
        assert!(error.contains("is not running"));
    }

    #[test]
    fn ensure_runtime_stdin_returns_stdin_when_available() {
        #[cfg(target_os = "windows")]
        let mut child = Command::new("cmd")
            .args(["/C", "more"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn more");
        #[cfg(not(target_os = "windows"))]
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn cat");

        let stdin = Arc::new(Mutex::new(child.stdin.take().expect("stdin")));
        let resolved = ensure_runtime_stdin_for_commands(true, Some(stdin), "test runtime")
            .expect("resolve stdin");
        drop(resolved);
        let _ = child.kill();
        let _ = child.wait();
    }
}
