use runtime_lib::agent::tools::process_manager::ProcessManager;
use runtime_lib::agent::{
    BashKillTool, BashOutputTool, BashTool, ExecKillTool, ExecOutputTool, ExecTool, Tool,
    ToolContext,
};
use serde_json::json;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn parse_bash_result(result: &str) -> serde_json::Value {
    serde_json::from_str(result).expect("valid bash result json")
}

fn parse_tool_result(result: &str) -> serde_json::Value {
    serde_json::from_str(result).expect("valid tool result json")
}

#[test]
fn test_bash_background_returns_process_id() {
    let pm = Arc::new(ProcessManager::new());
    let tool = BashTool::with_process_manager(Arc::clone(&pm));
    let ctx = ToolContext::default();

    let command = if cfg!(target_os = "windows") {
        "echo bg_test"
    } else {
        "echo bg_test"
    };

    let input = json!({"command": command, "background": true});
    let result = tool.execute(input, &ctx).unwrap();

    let parsed = parse_bash_result(&result);
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["tool"], "bash");
    assert_eq!(parsed["details"]["background"], true);
    assert!(parsed["details"]["process_id"].as_str().is_some());
    let output_file_path = parsed["details"]["output_file_path"]
        .as_str()
        .expect("background start should return output file path");
    assert!(std::path::Path::new(output_file_path).exists());
}

#[test]
fn test_bash_background_false_runs_sync() {
    let pm = Arc::new(ProcessManager::new());
    let tool = BashTool::with_process_manager(Arc::clone(&pm));
    let ctx = ToolContext::default();

    let input = json!({"command": "echo sync_test", "background": false});
    let result = tool.execute(input, &ctx).unwrap();

    let parsed = parse_bash_result(&result);
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["details"]["background"], false);
    assert!(
        parsed["details"]["stdout"]
            .as_str()
            .unwrap_or_default()
            .contains("sync_test")
    );
    assert!(parsed["details"].get("process_id").is_none());
}

#[test]
fn test_bash_background_without_pm_errors() {
    // 不配置 ProcessManager 的 BashTool
    let tool = BashTool::new();
    let ctx = ToolContext::default();

    let input = json!({"command": "echo test", "background": true});
    let result = tool.execute(input, &ctx);

    // 应该返回错误
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("ProcessManager"));
}

#[test]
fn test_bash_output_gets_finished_process() {
    let pm = Arc::new(ProcessManager::new());
    let bash = BashTool::with_process_manager(Arc::clone(&pm));
    let output_tool = BashOutputTool::new(Arc::clone(&pm));
    let ctx = ToolContext::default();

    // 启动后台命令
    let command = if cfg!(target_os = "windows") {
        "echo output_test"
    } else {
        "echo output_test"
    };
    let input = json!({"command": command, "background": true});
    let result = bash.execute(input, &ctx).unwrap();

    // 提取 process_id
    let process_id = parse_bash_result(&result)["details"]["process_id"]
        .as_str()
        .expect("process_id")
        .to_string();

    // 等待完成后获取输出
    thread::sleep(Duration::from_millis(1000));

    let output_input = json!({"process_id": process_id, "block": false});
    let output_result = output_tool.execute(output_input, &ctx).unwrap();

    let parsed = parse_tool_result(&output_result);
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["tool"], "bash_output");
    assert_eq!(parsed["details"]["process_id"], process_id);
    assert_eq!(parsed["details"]["exited"], true);
    assert_eq!(parsed["details"]["exit_code"], 0);
    assert!(
        parsed["details"]["stdout"]
            .as_str()
            .unwrap_or_default()
            .contains("output_test")
    );
}

#[test]
fn test_exec_background_output_uses_exec_named_tool_and_persisted_output_file() {
    let pm = Arc::new(ProcessManager::new());
    let exec = ExecTool::with_process_manager(Arc::clone(&pm));
    let output_tool = ExecOutputTool::new(Arc::clone(&pm));
    let ctx = ToolContext::default();

    let input = json!({"command": "echo exec_output_test", "background": true});
    let result = exec.execute(input, &ctx).unwrap();
    let start = parse_bash_result(&result);
    let start_output_file_path = start["details"]["output_file_path"]
        .as_str()
        .expect("exec background start should return output file path")
        .to_string();
    assert!(std::path::Path::new(&start_output_file_path).exists());
    let process_id = parse_bash_result(&result)["details"]["process_id"]
        .as_str()
        .expect("process_id")
        .to_string();

    let output_result = output_tool
        .execute(json!({"process_id": process_id, "block": true}), &ctx)
        .unwrap();
    let parsed = parse_tool_result(&output_result);

    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["tool"], "exec_output");
    assert_eq!(parsed["details"]["process_id"], process_id);
    assert_eq!(parsed["details"]["exited"], true);
    assert!(
        parsed["details"]["stdout"]
            .as_str()
            .unwrap_or_default()
            .contains("exec_output_test")
    );

    let output_file_path = parsed["details"]["output_file_path"]
        .as_str()
        .expect("output file path");
    assert_eq!(output_file_path, start_output_file_path);
    assert!(std::path::Path::new(output_file_path).exists());
    assert!(
        parsed["details"]["output_file_size"]
            .as_u64()
            .unwrap_or_default()
            > 0
    );
}

#[test]
fn test_exec_kill_terminates_exec_background_process() {
    let pm = Arc::new(ProcessManager::new());
    let exec = ExecTool::with_process_manager(Arc::clone(&pm));
    let output_tool = ExecOutputTool::new(Arc::clone(&pm));
    let kill_tool = ExecKillTool::new(Arc::clone(&pm));
    let ctx = ToolContext::default();

    let command = if cfg!(target_os = "windows") {
        "ping -n 100 127.0.0.1"
    } else {
        "sleep 100"
    };
    let result = exec
        .execute(json!({"command": command, "background": true}), &ctx)
        .unwrap();
    let process_id = parse_bash_result(&result)["details"]["process_id"]
        .as_str()
        .expect("process_id")
        .to_string();

    thread::sleep(Duration::from_millis(500));
    let running = parse_tool_result(
        &output_tool
            .execute(json!({"process_id": process_id}), &ctx)
            .unwrap(),
    );
    assert_eq!(running["details"]["exited"], false);

    let killed = parse_tool_result(
        &kill_tool
            .execute(json!({"process_id": process_id}), &ctx)
            .unwrap(),
    );
    assert_eq!(killed["ok"], true);
    assert_eq!(killed["tool"], "exec_kill");

    thread::sleep(Duration::from_millis(500));
    let exited = parse_tool_result(
        &output_tool
            .execute(json!({"process_id": process_id}), &ctx)
            .unwrap(),
    );
    assert_eq!(exited["details"]["exited"], true);
}

#[test]
fn test_process_manager_notifies_when_background_process_exits() {
    let (tx, rx) = mpsc::channel();
    let pm = Arc::new(ProcessManager::with_completion_notifier(Arc::new(
        move |completion| {
            tx.send(completion).expect("send completion");
        },
    )));
    let exec = ExecTool::with_process_manager(Arc::clone(&pm));
    let ctx = ToolContext::default();

    let result = exec
        .execute(
            json!({"command": "echo background_completion_test", "background": true}),
            &ctx,
        )
        .unwrap();
    let process_id = parse_bash_result(&result)["details"]["process_id"]
        .as_str()
        .expect("process id")
        .to_string();

    let completion = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("background completion notification");

    assert_eq!(completion.process_id, process_id);
    assert_eq!(completion.command, "echo background_completion_test");
    assert_eq!(completion.exit_code, Some(0));
    assert!(completion.output_file_path.exists());
}

#[test]
fn test_bash_output_block_mode() {
    let pm = Arc::new(ProcessManager::new());
    let bash = BashTool::with_process_manager(Arc::clone(&pm));
    let output_tool = BashOutputTool::new(Arc::clone(&pm));
    let ctx = ToolContext::default();

    let command = if cfg!(target_os = "windows") {
        "echo block_test"
    } else {
        "echo block_test"
    };
    let input = json!({"command": command, "background": true});
    let result = bash.execute(input, &ctx).unwrap();
    let process_id = parse_bash_result(&result)["details"]["process_id"]
        .as_str()
        .expect("process_id")
        .to_string();

    // 使用 block=true，会等待进程退出
    let output_input = json!({"process_id": process_id, "block": true});
    let output_result = output_tool.execute(output_input, &ctx).unwrap();

    let parsed = parse_tool_result(&output_result);
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["details"]["process_id"], process_id);
    assert_eq!(parsed["details"]["block"], true);
    assert_eq!(parsed["details"]["exited"], true);
    assert_eq!(parsed["details"]["exit_code"], 0);
    assert!(
        parsed["details"]["stdout"]
            .as_str()
            .unwrap_or_default()
            .contains("block_test")
    );
}

#[test]
fn test_bash_kill_terminates_running_process() {
    let pm = Arc::new(ProcessManager::new());
    let bash = BashTool::with_process_manager(Arc::clone(&pm));
    let kill_tool = BashKillTool::new(Arc::clone(&pm));
    let output_tool = BashOutputTool::new(Arc::clone(&pm));
    let ctx = ToolContext::default();

    // 启动长时间运行的后台命令
    let command = if cfg!(target_os = "windows") {
        "ping -n 100 127.0.0.1"
    } else {
        "sleep 100"
    };
    let input = json!({"command": command, "background": true});
    let result = bash.execute(input, &ctx).unwrap();
    let process_id = parse_bash_result(&result)["details"]["process_id"]
        .as_str()
        .expect("process_id")
        .to_string();

    // 等待进程启动
    thread::sleep(Duration::from_millis(500));

    // 确认进程在运行
    let output_input = json!({"process_id": process_id});
    let output_result = output_tool.execute(output_input, &ctx).unwrap();
    let running = parse_tool_result(&output_result);
    assert_eq!(running["ok"], true);
    assert_eq!(running["details"]["exited"], false);

    // 终止进程
    let kill_input = json!({"process_id": process_id});
    let kill_result = kill_tool.execute(kill_input, &ctx).unwrap();
    let killed = parse_tool_result(&kill_result);
    assert_eq!(killed["ok"], true);
    assert_eq!(killed["tool"], "bash_kill");
    assert_eq!(killed["details"]["process_id"], process_id);

    // 等待退出
    thread::sleep(Duration::from_millis(500));

    // 确认已退出
    let output_input2 = json!({"process_id": process_id});
    let output_result2 = output_tool.execute(output_input2, &ctx).unwrap();
    let exited = parse_tool_result(&output_result2);
    assert_eq!(exited["ok"], true);
    assert_eq!(exited["details"]["exited"], true);
}

#[test]
fn test_bash_output_nonexistent_process() {
    let pm = Arc::new(ProcessManager::new());
    let output_tool = BashOutputTool::new(Arc::clone(&pm));
    let ctx = ToolContext::default();

    let input = json!({"process_id": "no_exist"});
    let result = output_tool.execute(input, &ctx);
    assert!(result.is_err());
}

#[test]
fn test_bash_kill_nonexistent_process() {
    let pm = Arc::new(ProcessManager::new());
    let kill_tool = BashKillTool::new(Arc::clone(&pm));
    let ctx = ToolContext::default();

    let input = json!({"process_id": "no_exist"});
    let result = kill_tool.execute(input, &ctx);
    assert!(result.is_err());
}
