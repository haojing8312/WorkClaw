use runtime_lib::agent::{FileCopyTool, Tool, ToolContext};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// 复制单个文件，验证源文件仍然存在、目标文件内容一致
#[test]
fn test_copy_file() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("source.txt");
    let dst = tmp.path().join("dest.txt");

    fs::write(&src, "hello copy").unwrap();

    let tool = FileCopyTool;
    let ctx = ToolContext::default();

    let input = json!({
        "source": src.to_str().unwrap(),
        "destination": dst.to_str().unwrap(),
    });

    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("已复制文件"));

    // 源文件仍然存在
    assert!(src.exists(), "源文件应仍然存在");

    // 目标文件内容一致
    let content = fs::read_to_string(&dst).unwrap();
    assert_eq!(content, "hello copy");
}

/// 递归复制目录，验证嵌套文件和子目录都被保留
#[test]
fn test_copy_directory_recursive() {
    let tmp = TempDir::new().unwrap();
    let src_dir = tmp.path().join("src_dir");
    let sub_dir = src_dir.join("sub");
    fs::create_dir_all(&sub_dir).unwrap();
    fs::write(src_dir.join("a.txt"), "file a").unwrap();
    fs::write(sub_dir.join("b.txt"), "file b").unwrap();

    let dst_dir = tmp.path().join("dst_dir");

    let tool = FileCopyTool;
    let ctx = ToolContext::default();

    let input = json!({
        "source": src_dir.to_str().unwrap(),
        "destination": dst_dir.to_str().unwrap(),
    });

    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("已复制目录"));
    assert!(result.contains("2 个文件"));

    // 源目录仍然存在
    assert!(src_dir.exists(), "源目录应仍然存在");

    // 目标目录结构完整
    assert_eq!(fs::read_to_string(dst_dir.join("a.txt")).unwrap(), "file a");
    assert_eq!(
        fs::read_to_string(dst_dir.join("sub").join("b.txt")).unwrap(),
        "file b"
    );
}

/// 源路径不存在时应返回错误
#[test]
fn test_copy_source_not_found() {
    let tmp = TempDir::new().unwrap();
    let tool = FileCopyTool;
    let ctx = ToolContext::default();

    let input = json!({
        "source": tmp.path().join("nonexistent.txt").to_str().unwrap(),
        "destination": tmp.path().join("dest.txt").to_str().unwrap(),
    });

    let result = tool.execute(input, &ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("源路径不存在"));
}

/// 复制文件时自动创建目标父目录
#[test]
fn test_copy_creates_parent_dirs() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("source.txt");
    fs::write(&src, "nested copy").unwrap();

    let dst = tmp.path().join("deep").join("nested").join("dest.txt");

    let tool = FileCopyTool;
    let ctx = ToolContext::default();

    let input = json!({
        "source": src.to_str().unwrap(),
        "destination": dst.to_str().unwrap(),
    });

    let result = tool.execute(input, &ctx).unwrap();
    assert!(result.contains("已复制文件"));
    assert_eq!(fs::read_to_string(&dst).unwrap(), "nested copy");
}

/// 缺少必需参数时应返回错误
#[test]
fn test_copy_missing_params() {
    let tool = FileCopyTool;
    let ctx = ToolContext::default();

    // 缺少 destination
    let input = json!({"source": "some_path"});
    let result = tool.execute(input, &ctx);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("缺少 destination 参数"));

    // 缺少 source
    let input = json!({"destination": "some_path"});
    let result = tool.execute(input, &ctx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("缺少 source 参数"));
}
