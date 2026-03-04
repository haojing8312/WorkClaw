use runtime_lib::agent::{ListDirTool, Tool, ToolContext};
use serde_json::json;
use std::fs;

#[test]
fn test_list_dir_basic() {
    // 创建临时目录结构
    let dir = "test_list_dir_basic_tmp";
    fs::create_dir_all(format!("{}/subdir", dir)).unwrap();
    fs::write(format!("{}/hello.txt", dir), "Hello!").unwrap();
    fs::write(format!("{}/data.bin", dir), vec![0u8; 2048]).unwrap();

    let tool = ListDirTool;
    let ctx = ToolContext::default();
    let input = json!({"path": dir});
    let result = tool.execute(input, &ctx).unwrap();

    // 应包含文件和子目录标记
    assert!(result.contains("[FILE]"), "应包含 [FILE] 标记");
    assert!(result.contains("[DIR]"), "应包含 [DIR] 标记");
    assert!(result.contains("hello.txt"), "应包含 hello.txt");
    assert!(result.contains("data.bin"), "应包含 data.bin");
    assert!(result.contains("subdir"), "应包含 subdir");

    // 文件大小应以人类可读格式显示
    assert!(
        result.contains("KB") || result.contains("B"),
        "应显示文件大小"
    );

    // 清理
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn test_list_dir_empty() {
    // 创建空目录
    let dir = "test_list_dir_empty_tmp";
    fs::create_dir_all(dir).unwrap();

    let tool = ListDirTool;
    let ctx = ToolContext::default();
    let input = json!({"path": dir});
    let result = tool.execute(input, &ctx).unwrap();

    assert!(result.contains("空目录"), "空目录应返回 '空目录'");

    // 清理
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn test_list_dir_nonexistent() {
    let tool = ListDirTool;
    let ctx = ToolContext::default();
    let input = json!({"path": "nonexistent_dir_xyz_12345"});
    let result = tool.execute(input, &ctx);

    assert!(result.is_err(), "不存在的目录应返回错误");
}

#[test]
fn test_list_dir_sorted() {
    // 创建包含多个文件的目录，验证排序
    let dir = "test_list_dir_sorted_tmp";
    fs::create_dir_all(dir).unwrap();
    fs::write(format!("{}/charlie.txt", dir), "c").unwrap();
    fs::write(format!("{}/alpha.txt", dir), "a").unwrap();
    fs::write(format!("{}/bravo.txt", dir), "b").unwrap();

    let tool = ListDirTool;
    let ctx = ToolContext::default();
    let input = json!({"path": dir});
    let result = tool.execute(input, &ctx).unwrap();

    // 验证字母序排列：alpha < bravo < charlie
    let alpha_pos = result.find("alpha.txt").unwrap();
    let bravo_pos = result.find("bravo.txt").unwrap();
    let charlie_pos = result.find("charlie.txt").unwrap();
    assert!(alpha_pos < bravo_pos, "alpha 应排在 bravo 前面");
    assert!(bravo_pos < charlie_pos, "bravo 应排在 charlie 前面");

    // 清理
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn test_list_dir_missing_path_param() {
    let tool = ListDirTool;
    let ctx = ToolContext::default();
    let input = json!({});
    let result = tool.execute(input, &ctx);
    assert!(result.is_err(), "缺少 path 参数应返回错误");
    assert!(result.unwrap_err().to_string().contains("缺少 path 参数"));
}
