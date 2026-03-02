//! 全量集成测试：验证所有已注册工具的完整性
//!
//! 确保 ToolRegistry::with_standard_tools() 静态注册的 15 个工具全部可用，
//! 以及 L3/L4 动态注册工具可独立创建并注册。

use runtime_lib::agent::ToolRegistry;
use runtime_lib::agent::tools::{
    BashTool, BashOutputTool, BashKillTool, ProcessManager,
    browser_tools::register_browser_tools,
};
use std::sync::Arc;

/// 验证 with_standard_tools() 静态注册的 15 个工具全部存在
#[test]
fn test_all_standard_tools_registered() {
    let registry = ToolRegistry::with_standard_tools();

    let expected_tools = [
        // L1: 原有基础工具
        "read_file", "write_file", "glob", "grep", "edit",
        "todo_write", "web_fetch", "bash",
        // L2: 文件扩展工具
        "list_dir", "file_stat", "file_delete", "file_move", "file_copy",
        // L5: 系统工具
        "screenshot", "open_in_folder",
    ];

    for name in &expected_tools {
        assert!(
            registry.get(name).is_some(),
            "标准工具 '{}' 未注册",
            name
        );
    }

    let defs = registry.get_tool_definitions();
    assert_eq!(defs.len(), 15, "标准工具总数应为 15");
}

/// 验证向后兼容：with_file_tools() 别名与 with_standard_tools() 行为一致
#[test]
fn test_with_file_tools_backward_compat() {
    let old = ToolRegistry::with_file_tools();
    let new = ToolRegistry::with_standard_tools();

    assert_eq!(
        old.get_tool_definitions().len(),
        new.get_tool_definitions().len(),
        "with_file_tools() 和 with_standard_tools() 应注册相同数量的工具"
    );
}

/// 验证 L3 工具（进程管理）可独立创建并注册
#[test]
fn test_l3_process_management_tools() {
    let registry = ToolRegistry::with_standard_tools();
    let pm = Arc::new(ProcessManager::new());

    // 注册 L3 工具
    registry.register(Arc::new(BashOutputTool::new(Arc::clone(&pm))));
    registry.register(Arc::new(BashKillTool::new(Arc::clone(&pm))));

    // 替换默认 bash 为支持后台模式的版本
    registry.unregister("bash");
    registry.register(Arc::new(BashTool::with_process_manager(Arc::clone(&pm))));

    assert!(registry.get("bash_output").is_some(), "bash_output 应已注册");
    assert!(registry.get("bash_kill").is_some(), "bash_kill 应已注册");
    assert!(registry.get("bash").is_some(), "替换后的 bash 应仍然存在");

    // 15 标准 + 2 新增 (bash_output, bash_kill) = 17
    assert_eq!(registry.get_tool_definitions().len(), 17);
}

/// 验证 L4 浏览器工具动态注册
#[test]
fn test_l4_browser_tools_registration() {
    let registry = ToolRegistry::with_standard_tools();

    // 注册 17 个浏览器工具
    register_browser_tools(&registry, "http://localhost:8765");

    let browser_tools = registry.tools_with_prefix("browser_");
    assert_eq!(browser_tools.len(), 17, "应注册 17 个浏览器工具");

    // 15 标准 + 17 浏览器 = 32
    assert_eq!(registry.get_tool_definitions().len(), 32);
}

/// 验证全量注册：L1-L5 静态 + L3 动态 + L4 动态 = 34 个工具
#[test]
fn test_full_tool_registration() {
    let registry = ToolRegistry::with_standard_tools();

    // L3: 进程管理
    let pm = Arc::new(ProcessManager::new());
    registry.register(Arc::new(BashOutputTool::new(Arc::clone(&pm))));
    registry.register(Arc::new(BashKillTool::new(Arc::clone(&pm))));
    registry.unregister("bash");
    registry.register(Arc::new(BashTool::with_process_manager(pm)));

    // L4: 浏览器工具
    register_browser_tools(&registry, "http://localhost:8765");

    // 总数：15 标准 + 2 进程管理 + 17 浏览器 = 34
    let total = registry.get_tool_definitions().len();
    assert_eq!(total, 34, "全量注册后应有 34 个工具，实际 {}", total);
}

/// 验证每个工具都有名称、描述和 input_schema
#[test]
fn test_all_tools_have_metadata() {
    let registry = ToolRegistry::with_standard_tools();
    let pm = Arc::new(ProcessManager::new());
    registry.register(Arc::new(BashOutputTool::new(Arc::clone(&pm))));
    registry.register(Arc::new(BashKillTool::new(pm)));
    register_browser_tools(&registry, "http://localhost:8765");

    let defs = registry.get_tool_definitions();
    for def in &defs {
        let name = def["name"].as_str().unwrap_or("");
        assert!(!name.is_empty(), "工具名称不应为空");
        assert!(
            def["description"].as_str().map_or(false, |d| !d.is_empty()),
            "工具 '{}' 缺少描述",
            name
        );
        assert!(
            def["input_schema"].is_object(),
            "工具 '{}' 的 input_schema 不是对象",
            name
        );
    }
}

/// 验证 get_filtered_tool_definitions 白名单功能
#[test]
fn test_filtered_tool_definitions() {
    let registry = ToolRegistry::with_standard_tools();

    let whitelist: Vec<String> = vec![
        "read_file".to_string(),
        "write_file".to_string(),
        "bash".to_string(),
    ];

    let filtered = registry.get_filtered_tool_definitions(&whitelist);
    assert_eq!(filtered.len(), 3, "过滤后应有 3 个工具");

    let names: Vec<&str> = filtered.iter()
        .filter_map(|d| d["name"].as_str())
        .collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"write_file"));
    assert!(names.contains(&"bash"));
}
