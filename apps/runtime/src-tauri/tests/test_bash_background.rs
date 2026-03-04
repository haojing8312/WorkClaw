use runtime_lib::agent::tools::process_manager::ProcessManager;
use runtime_lib::agent::{BashKillTool, BashOutputTool, BashTool, Tool, ToolContext};
use serde_json::json;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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

    // 应返回包含 process_id 的消息
    assert!(result.contains("后台进程已启动"));
    assert!(result.contains("process_id:"));
}

#[test]
fn test_bash_background_false_runs_sync() {
    let pm = Arc::new(ProcessManager::new());
    let tool = BashTool::with_process_manager(Arc::clone(&pm));
    let ctx = ToolContext::default();

    let input = json!({"command": "echo sync_test", "background": false});
    let result = tool.execute(input, &ctx).unwrap();

    // 同步模式应直接返回输出
    assert!(result.contains("sync_test"));
    assert!(!result.contains("process_id"));
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
    let process_id = result.split("process_id: ").nth(1).unwrap().trim();

    // 等待完成后获取输出
    thread::sleep(Duration::from_millis(1000));

    let output_input = json!({"process_id": process_id, "block": false});
    let output_result = output_tool.execute(output_input, &ctx).unwrap();

    assert!(output_result.contains("output_test"));
    assert!(output_result.contains("已退出"));
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
    let process_id = result.split("process_id: ").nth(1).unwrap().trim();

    // 使用 block=true，会等待进程退出
    let output_input = json!({"process_id": process_id, "block": true});
    let output_result = output_tool.execute(output_input, &ctx).unwrap();

    assert!(output_result.contains("block_test"));
    assert!(output_result.contains("已退出"));
    assert!(output_result.contains("退出码: 0"));
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
    let process_id = result.split("process_id: ").nth(1).unwrap().trim();

    // 等待进程启动
    thread::sleep(Duration::from_millis(500));

    // 确认进程在运行
    let output_input = json!({"process_id": process_id});
    let output_result = output_tool.execute(output_input, &ctx).unwrap();
    assert!(output_result.contains("运行中"));

    // 终止进程
    let kill_input = json!({"process_id": process_id});
    let kill_result = kill_tool.execute(kill_input, &ctx).unwrap();
    assert!(kill_result.contains("已终止进程"));

    // 等待退出
    thread::sleep(Duration::from_millis(500));

    // 确认已退出
    let output_input2 = json!({"process_id": process_id});
    let output_result2 = output_tool.execute(output_input2, &ctx).unwrap();
    assert!(output_result2.contains("已退出"));
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
