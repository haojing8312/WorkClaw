use runtime_lib::agent::{FileMoveTool, Tool, ToolContext};
use serde_json::json;
use std::fs;

#[test]
fn test_move_file() {
    let tool = FileMoveTool;
    let ctx = ToolContext::default();

    // 准备：创建源文件
    let src = "test_move_src.txt";
    let dst = "test_move_dst.txt";
    fs::write(src, "move me").unwrap();

    let input = json!({
        "source": src,
        "destination": dst
    });

    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("已移动"));

    // 验证源文件已不存在
    assert!(!std::path::Path::new(src).exists());
    // 验证目标文件内容正确
    let content = fs::read_to_string(dst).unwrap();
    assert_eq!(content, "move me");

    // 清理
    fs::remove_file(dst).unwrap();
}

#[test]
fn test_move_directory_with_contents() {
    let tool = FileMoveTool;
    let ctx = ToolContext::default();

    // 准备：创建带内容的目录
    let src_dir = "test_move_dir_src";
    let dst_dir = "test_move_dir_dst";
    fs::create_dir_all(format!("{}/sub", src_dir)).unwrap();
    fs::write(format!("{}/a.txt", src_dir), "file a").unwrap();
    fs::write(format!("{}/sub/b.txt", src_dir), "file b").unwrap();

    let input = json!({
        "source": src_dir,
        "destination": dst_dir
    });

    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("已移动"));

    // 验证源目录已不存在
    assert!(!std::path::Path::new(src_dir).exists());
    // 验证目标目录及子文件内容正确
    assert_eq!(
        fs::read_to_string(format!("{}/a.txt", dst_dir)).unwrap(),
        "file a"
    );
    assert_eq!(
        fs::read_to_string(format!("{}/sub/b.txt", dst_dir)).unwrap(),
        "file b"
    );

    // 清理
    fs::remove_dir_all(dst_dir).unwrap();
}

#[test]
fn test_move_source_not_found() {
    let tool = FileMoveTool;
    let ctx = ToolContext::default();

    let input = json!({
        "source": "nonexistent_file_for_move.txt",
        "destination": "whatever.txt"
    });

    let result = tool.execute(input, &ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("源路径不存在"));
}

#[test]
fn test_move_creates_parent_dirs() {
    let tool = FileMoveTool;
    let ctx = ToolContext::default();

    // 准备：创建源文件
    let src = "test_move_parent_src.txt";
    let dst = "test_move_nested/deep/moved.txt";
    fs::write(src, "nested move").unwrap();

    let input = json!({
        "source": src,
        "destination": dst
    });

    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("已移动"));

    // 验证目标文件内容
    let content = fs::read_to_string(dst).unwrap();
    assert_eq!(content, "nested move");

    // 清理
    fs::remove_dir_all("test_move_nested").unwrap();
}
