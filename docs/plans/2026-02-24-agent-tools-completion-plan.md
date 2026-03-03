# Agent 工具补全实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 WorkClaw Agent 从 8 个注册工具扩展到 37 个完整桌面 Agent 工具集。

**Architecture:** 分 5 层渐进实现。L2（文件工具）和 L3（Shell 进程管理）为纯 Rust 原生工具；L4（浏览器自动化）在 Node.js Sidecar 中用 Playwright 实现，Rust 侧通过 SidecarBridgeTool 动态注册；L5（系统工具）利用系统命令实现。

**Tech Stack:** Rust (Tool trait), Node.js/TypeScript (Hono + Playwright), playwright-extra + stealth plugin

**Design Doc:** `docs/plans/2026-02-24-agent-tools-completion-design.md`

---

## Task 1: list_dir 工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/list_dir.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_list_dir.rs`

**Step 1: Write the failing test**

```rust
// tests/test_list_dir.rs
use serde_json::json;
use std::fs;
use tempfile::TempDir;

// 引入被测工具
use workclaw_runtime::agent::tools::ListDirTool;
use workclaw_runtime::agent::types::{Tool, ToolContext};

#[test]
fn test_list_dir_basic() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    // 创建测试文件和子目录
    fs::write(dir.join("hello.txt"), "world").unwrap();
    fs::write(dir.join("data.json"), "{}").unwrap();
    fs::create_dir(dir.join("subdir")).unwrap();

    let tool = ListDirTool;
    let ctx = ToolContext { work_dir: Some(dir.to_path_buf()) };
    let result = tool.execute(json!({"path": dir.to_str().unwrap()}), &ctx).unwrap();

    assert!(result.contains("hello.txt"));
    assert!(result.contains("data.json"));
    assert!(result.contains("subdir"));
    // 目录应标记为 [DIR]
    assert!(result.contains("[DIR]"));
}

#[test]
fn test_list_dir_empty() {
    let tmp = TempDir::new().unwrap();
    let tool = ListDirTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    let result = tool.execute(json!({"path": tmp.path().to_str().unwrap()}), &ctx).unwrap();
    assert!(result.contains("空目录") || result.is_empty() || result.contains("0"));
}

#[test]
fn test_list_dir_not_found() {
    let tool = ListDirTool;
    let ctx = ToolContext::default();
    let result = tool.execute(json!({"path": "/nonexistent/dir"}), &ctx);
    assert!(result.is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_list_dir`
Expected: FAIL - `ListDirTool` not found

**Step 3: Write implementation**

```rust
// src/agent/tools/list_dir.rs
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;

pub struct ListDirTool;

impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "列出目录内容，返回文件名、类型和大小"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要列出的目录路径"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;

        let checked = ctx.check_path(path)?;

        if !checked.is_dir() {
            return Err(anyhow!("{} 不是目录", path));
        }

        let entries = fs::read_dir(&checked)
            .map_err(|e| anyhow!("读取目录失败: {}", e))?;

        let mut items: Vec<String> = Vec::new();
        for entry in entries {
            let entry = entry?;
            let meta = entry.metadata()?;
            let name = entry.file_name().to_string_lossy().to_string();

            if meta.is_dir() {
                items.push(format!("[DIR]  {}", name));
            } else {
                let size = meta.len();
                let size_str = if size < 1024 {
                    format!("{} B", size)
                } else if size < 1024 * 1024 {
                    format!("{:.1} KB", size as f64 / 1024.0)
                } else {
                    format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
                };
                items.push(format!("[FILE] {} ({})", name, size_str));
            }
        }

        items.sort();

        if items.is_empty() {
            Ok("空目录".to_string())
        } else {
            Ok(format!("共 {} 项:\n{}", items.len(), items.join("\n")))
        }
    }
}
```

**Step 4: Register in mod.rs and registry.rs**

In `mod.rs` add:
```rust
mod list_dir;
pub use list_dir::ListDirTool;
```

In `registry.rs` add import and registration:
```rust
// import 中添加 ListDirTool
use super::tools::{..., ListDirTool};

// with_file_tools() 中添加
registry.register(Arc::new(ListDirTool));
```

**Step 5: Run tests**

Run: `cd apps/runtime/src-tauri && cargo test --test test_list_dir`
Expected: PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/list_dir.rs apps/runtime/src-tauri/src/agent/tools/mod.rs apps/runtime/src-tauri/src/agent/registry.rs apps/runtime/src-tauri/tests/test_list_dir.rs
git commit -m "feat(agent): 添加 list_dir 工具 — 列出目录内容"
```

---

## Task 2: file_stat 工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/file_stat.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_file_stat.rs`

**Step 1: Write the failing test**

```rust
// tests/test_file_stat.rs
use serde_json::json;
use std::fs;
use tempfile::TempDir;

use workclaw_runtime::agent::tools::FileStatTool;
use workclaw_runtime::agent::types::{Tool, ToolContext};

#[test]
fn test_file_stat_file() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let tool = FileStatTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    let result = tool.execute(json!({"path": file_path.to_str().unwrap()}), &ctx).unwrap();

    assert!(result.contains("11")); // 11 bytes
    assert!(result.contains("file"));
}

#[test]
fn test_file_stat_dir() {
    let tmp = TempDir::new().unwrap();
    let tool = FileStatTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    let result = tool.execute(json!({"path": tmp.path().to_str().unwrap()}), &ctx).unwrap();

    assert!(result.contains("directory"));
}

#[test]
fn test_file_stat_not_found() {
    let tool = FileStatTool;
    let ctx = ToolContext::default();
    let result = tool.execute(json!({"path": "/nonexistent"}), &ctx);
    assert!(result.is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_file_stat`
Expected: FAIL

**Step 3: Write implementation**

```rust
// src/agent/tools/file_stat.rs
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct FileStatTool;

impl Tool for FileStatTool {
    fn name(&self) -> &str {
        "file_stat"
    }

    fn description(&self) -> &str {
        "获取文件或目录的元信息（大小、类型、修改时间）"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件或目录路径"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;

        let checked = ctx.check_path(path)?;
        let meta = std::fs::metadata(&checked)
            .map_err(|e| anyhow!("获取文件信息失败: {}", e))?;

        let file_type = if meta.is_dir() {
            "directory"
        } else if meta.is_symlink() {
            "symlink"
        } else {
            "file"
        };

        let size = meta.len();
        let modified = meta.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                let secs = d.as_secs();
                // 简单的时间格式化
                let dt = chrono::DateTime::from_timestamp(secs as i64, 0)
                    .unwrap_or_default();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_else(|| "unknown".to_string());

        let readonly = meta.permissions().readonly();

        Ok(format!(
            "类型: {}\n大小: {} bytes\n修改时间: {}\n只读: {}",
            file_type, size, modified, readonly
        ))
    }
}
```

**Step 4: Register in mod.rs and registry.rs** (同 Task 1 模式)

**Step 5: Run tests and commit**

Run: `cd apps/runtime/src-tauri && cargo test --test test_file_stat`

```bash
git add -A apps/runtime/src-tauri/src/agent/tools/file_stat.rs apps/runtime/src-tauri/src/agent/tools/mod.rs apps/runtime/src-tauri/src/agent/registry.rs apps/runtime/src-tauri/tests/test_file_stat.rs
git commit -m "feat(agent): 添加 file_stat 工具 — 获取文件元信息"
```

---

## Task 3: file_delete 工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/file_delete.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_file_delete.rs`

**Step 1: Write the failing test**

```rust
// tests/test_file_delete.rs
use serde_json::json;
use std::fs;
use tempfile::TempDir;

use workclaw_runtime::agent::tools::FileDeleteTool;
use workclaw_runtime::agent::types::{Tool, ToolContext};

#[test]
fn test_delete_file() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("delete_me.txt");
    fs::write(&file, "bye").unwrap();
    assert!(file.exists());

    let tool = FileDeleteTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    let result = tool.execute(json!({"path": file.to_str().unwrap()}), &ctx).unwrap();

    assert!(!file.exists());
    assert!(result.contains("已删除"));
}

#[test]
fn test_delete_empty_dir() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("empty_dir");
    fs::create_dir(&dir).unwrap();

    let tool = FileDeleteTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    let result = tool.execute(json!({"path": dir.to_str().unwrap()}), &ctx).unwrap();

    assert!(!dir.exists());
    assert!(result.contains("已删除"));
}

#[test]
fn test_delete_nonempty_dir_blocked() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("nonempty");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("file.txt"), "content").unwrap();

    let tool = FileDeleteTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    let result = tool.execute(json!({"path": dir.to_str().unwrap(), "recursive": false}), &ctx);

    // 默认不递归删除非空目录
    assert!(result.is_err() || result.unwrap().contains("非空目录"));
}

#[test]
fn test_delete_recursive() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("nonempty");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("file.txt"), "content").unwrap();

    let tool = FileDeleteTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    let result = tool.execute(json!({"path": dir.to_str().unwrap(), "recursive": true}), &ctx).unwrap();

    assert!(!dir.exists());
    assert!(result.contains("已删除"));
}

#[test]
fn test_delete_not_found() {
    let tool = FileDeleteTool;
    let ctx = ToolContext::default();
    let result = tool.execute(json!({"path": "/nonexistent/file"}), &ctx);
    assert!(result.is_err());
}
```

**Step 2: Run test to verify failure, then implement**

```rust
// src/agent/tools/file_delete.rs
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;

pub struct FileDeleteTool;

impl Tool for FileDeleteTool {
    fn name(&self) -> &str {
        "file_delete"
    }

    fn description(&self) -> &str {
        "删除文件或目录。默认仅删除文件和空目录，设置 recursive=true 可删除非空目录"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要删除的文件或目录路径"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "是否递归删除非空目录（默认 false）"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let recursive = input["recursive"].as_bool().unwrap_or(false);

        let checked = ctx.check_path(path)?;

        if !checked.exists() {
            return Err(anyhow!("路径不存在: {}", path));
        }

        if checked.is_file() {
            fs::remove_file(&checked)?;
        } else if checked.is_dir() {
            if recursive {
                fs::remove_dir_all(&checked)?;
            } else {
                fs::remove_dir(&checked).map_err(|e| {
                    if checked.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false) {
                        anyhow!("非空目录，请设置 recursive=true 来递归删除")
                    } else {
                        anyhow!("删除目录失败: {}", e)
                    }
                })?;
            }
        }

        Ok(format!("已删除 {}", path))
    }
}
```

**Step 3: Register, run tests, commit**

```bash
git commit -m "feat(agent): 添加 file_delete 工具 — 安全删除文件/目录"
```

---

## Task 4: file_move 工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/file_move.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_file_move.rs`

**Step 1: Write the failing test**

```rust
// tests/test_file_move.rs
use serde_json::json;
use std::fs;
use tempfile::TempDir;

use workclaw_runtime::agent::tools::FileMoveTool;
use workclaw_runtime::agent::types::{Tool, ToolContext};

#[test]
fn test_move_file() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("old.txt");
    let dst = tmp.path().join("new.txt");
    fs::write(&src, "content").unwrap();

    let tool = FileMoveTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    tool.execute(json!({"source": src.to_str().unwrap(), "destination": dst.to_str().unwrap()}), &ctx).unwrap();

    assert!(!src.exists());
    assert!(dst.exists());
    assert_eq!(fs::read_to_string(&dst).unwrap(), "content");
}

#[test]
fn test_move_dir() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("old_dir");
    let dst = tmp.path().join("new_dir");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("file.txt"), "data").unwrap();

    let tool = FileMoveTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    tool.execute(json!({"source": src.to_str().unwrap(), "destination": dst.to_str().unwrap()}), &ctx).unwrap();

    assert!(!src.exists());
    assert!(dst.join("file.txt").exists());
}

#[test]
fn test_move_source_not_found() {
    let tool = FileMoveTool;
    let ctx = ToolContext::default();
    let result = tool.execute(json!({"source": "/nonexistent", "destination": "/somewhere"}), &ctx);
    assert!(result.is_err());
}
```

**Step 2: Implement**

```rust
// src/agent/tools/file_move.rs
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct FileMoveTool;

impl Tool for FileMoveTool {
    fn name(&self) -> &str {
        "file_move"
    }

    fn description(&self) -> &str {
        "移动或重命名文件/目录"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "源文件/目录路径"
                },
                "destination": {
                    "type": "string",
                    "description": "目标路径"
                }
            },
            "required": ["source", "destination"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let source = input["source"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 source 参数"))?;
        let destination = input["destination"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 destination 参数"))?;

        let src = ctx.check_path(source)?;
        let dst = ctx.check_path(destination)?;

        if !src.exists() {
            return Err(anyhow!("源路径不存在: {}", source));
        }

        // 确保目标父目录存在
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(&src, &dst)
            .map_err(|e| anyhow!("移动失败: {}", e))?;

        Ok(format!("已移动 {} → {}", source, destination))
    }
}
```

**Step 3: Register, run tests, commit**

```bash
git commit -m "feat(agent): 添加 file_move 工具 — 移动/重命名文件"
```

---

## Task 5: file_copy 工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/file_copy.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_file_copy.rs`

**Step 1: Write the failing test**

```rust
// tests/test_file_copy.rs
use serde_json::json;
use std::fs;
use tempfile::TempDir;

use workclaw_runtime::agent::tools::FileCopyTool;
use workclaw_runtime::agent::types::{Tool, ToolContext};

#[test]
fn test_copy_file() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src.txt");
    let dst = tmp.path().join("dst.txt");
    fs::write(&src, "content").unwrap();

    let tool = FileCopyTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    tool.execute(json!({"source": src.to_str().unwrap(), "destination": dst.to_str().unwrap()}), &ctx).unwrap();

    assert!(src.exists()); // 源文件仍在
    assert!(dst.exists());
    assert_eq!(fs::read_to_string(&dst).unwrap(), "content");
}

#[test]
fn test_copy_dir_recursive() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src_dir");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("a.txt"), "aaa").unwrap();
    fs::create_dir(src.join("sub")).unwrap();
    fs::write(src.join("sub").join("b.txt"), "bbb").unwrap();

    let dst = tmp.path().join("dst_dir");

    let tool = FileCopyTool;
    let ctx = ToolContext { work_dir: Some(tmp.path().to_path_buf()) };
    tool.execute(json!({"source": src.to_str().unwrap(), "destination": dst.to_str().unwrap()}), &ctx).unwrap();

    assert!(dst.join("a.txt").exists());
    assert!(dst.join("sub").join("b.txt").exists());
}
```

**Step 2: Implement**

```rust
// src/agent/tools/file_copy.rs
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

pub struct FileCopyTool;

impl FileCopyTool {
    fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<u64> {
        fs::create_dir_all(dst)?;
        let mut count = 0u64;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if src_path.is_dir() {
                count += Self::copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
                count += 1;
            }
        }
        Ok(count)
    }
}

impl Tool for FileCopyTool {
    fn name(&self) -> &str {
        "file_copy"
    }

    fn description(&self) -> &str {
        "复制文件或目录（目录自动递归复制）"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "源文件/目录路径"
                },
                "destination": {
                    "type": "string",
                    "description": "目标路径"
                }
            },
            "required": ["source", "destination"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let source = input["source"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 source 参数"))?;
        let destination = input["destination"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 destination 参数"))?;

        let src = ctx.check_path(source)?;
        let dst = ctx.check_path(destination)?;

        if !src.exists() {
            return Err(anyhow!("源路径不存在: {}", source));
        }

        if src.is_file() {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dst)?;
            Ok(format!("已复制文件 {} → {}", source, destination))
        } else if src.is_dir() {
            let count = Self::copy_dir_recursive(&src, &dst)?;
            Ok(format!("已复制目录 {} → {}（{} 个文件）", source, destination, count))
        } else {
            Err(anyhow!("不支持的文件类型"))
        }
    }
}
```

**Step 3: Register, run tests, commit**

```bash
git commit -m "feat(agent): 添加 file_copy 工具 — 复制文件/目录"
```

---

## Task 6: L2 完成 — 更新注册表并运行全量测试

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Modify: `apps/runtime/src-tauri/tests/test_registry.rs`

**Step 1: 确保 registry.rs 包含所有 L2 工具**

```rust
// registry.rs — 更新后的 with_file_tools()
use super::tools::{
    BashTool, EditTool, FileCopyTool, FileDeleteTool, FileMoveTool, FileStatTool,
    GlobTool, GrepTool, ListDirTool, ReadFileTool, TodoWriteTool, WebFetchTool,
    WriteFileTool,
};

pub fn with_file_tools() -> Self {
    let registry = Self::new();
    // 基础文件工具
    registry.register(Arc::new(ReadFileTool));
    registry.register(Arc::new(WriteFileTool));
    registry.register(Arc::new(GlobTool));
    registry.register(Arc::new(GrepTool));
    registry.register(Arc::new(EditTool));
    // L2 新增文件工具
    registry.register(Arc::new(ListDirTool));
    registry.register(Arc::new(FileStatTool));
    registry.register(Arc::new(FileDeleteTool));
    registry.register(Arc::new(FileMoveTool));
    registry.register(Arc::new(FileCopyTool));
    // 其他工具
    registry.register(Arc::new(TodoWriteTool::new()));
    registry.register(Arc::new(WebFetchTool));
    registry.register(Arc::new(BashTool));
    registry
}
```

**Step 2: 更新 test_registry.rs**

```rust
#[test]
fn test_registry_has_standard_tools() {
    let registry = ToolRegistry::with_file_tools();
    let tools = registry.get_tool_definitions();
    // 从 8 个增加到 13 个
    assert_eq!(tools.len(), 13);

    let names: Vec<String> = tools.iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    // 原有 8 个
    assert!(names.contains(&"read_file".to_string()));
    assert!(names.contains(&"write_file".to_string()));
    assert!(names.contains(&"glob".to_string()));
    assert!(names.contains(&"grep".to_string()));
    assert!(names.contains(&"edit".to_string()));
    assert!(names.contains(&"todo_write".to_string()));
    assert!(names.contains(&"web_fetch".to_string()));
    assert!(names.contains(&"bash".to_string()));
    // L2 新增 5 个
    assert!(names.contains(&"list_dir".to_string()));
    assert!(names.contains(&"file_stat".to_string()));
    assert!(names.contains(&"file_delete".to_string()));
    assert!(names.contains(&"file_move".to_string()));
    assert!(names.contains(&"file_copy".to_string()));
}
```

**Step 3: 运行全量测试**

Run: `cd apps/runtime/src-tauri && cargo test`
Expected: ALL PASS

**Step 4: Commit**

```bash
git commit -m "feat(agent): L2 完成 — 5 个文件扩展工具全部注册，更新注册表测试"
```

---

## Task 7: ProcessManager — Shell 后台进程管理

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/process_manager.rs`
- Test: `apps/runtime/src-tauri/tests/test_process_manager.rs`

**Step 1: Write the failing test**

```rust
// tests/test_process_manager.rs
use workclaw_runtime::agent::tools::process_manager::ProcessManager;
use std::thread;
use std::time::Duration;

#[test]
fn test_spawn_and_get_output() {
    let pm = ProcessManager::new();

    // 启动一个快速命令
    #[cfg(target_os = "windows")]
    let id = pm.spawn("echo hello", None).unwrap();
    #[cfg(not(target_os = "windows"))]
    let id = pm.spawn("echo hello", None).unwrap();

    // 等待完成
    thread::sleep(Duration::from_millis(500));

    let output = pm.get_output(&id, false).unwrap();
    assert!(output.stdout.contains("hello"));
    assert!(output.exited);
}

#[test]
fn test_spawn_background_and_kill() {
    let pm = ProcessManager::new();

    #[cfg(target_os = "windows")]
    let id = pm.spawn("ping -n 100 127.0.0.1", None).unwrap();
    #[cfg(not(target_os = "windows"))]
    let id = pm.spawn("sleep 100", None).unwrap();

    thread::sleep(Duration::from_millis(200));

    let output = pm.get_output(&id, false).unwrap();
    assert!(!output.exited);

    pm.kill(&id).unwrap();

    thread::sleep(Duration::from_millis(200));
    let output2 = pm.get_output(&id, false).unwrap();
    assert!(output2.exited);
}

#[test]
fn test_list_processes() {
    let pm = ProcessManager::new();

    #[cfg(target_os = "windows")]
    let _ = pm.spawn("echo test1", None).unwrap();
    #[cfg(not(target_os = "windows"))]
    let _ = pm.spawn("echo test1", None).unwrap();

    let list = pm.list();
    assert_eq!(list.len(), 1);
}

#[test]
fn test_get_nonexistent() {
    let pm = ProcessManager::new();
    let result = pm.get_output("nonexistent", false);
    assert!(result.is_err());
}
```

**Step 2: Implement ProcessManager**

```rust
// src/agent/tools/process_manager.rs
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

const MAX_BUFFER_LINES: usize = 5000;
const MAX_RETAINED: usize = 30;

pub struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exited: bool,
    pub exit_code: Option<i32>,
}

struct BackgroundProcess {
    child: Child,
    stdout_buf: Arc<Mutex<String>>,
    stderr_buf: Arc<Mutex<String>>,
    started_at: Instant,
    command: String,
    exited: Arc<Mutex<Option<i32>>>,
}

pub struct ProcessManager {
    processes: Arc<Mutex<HashMap<String, BackgroundProcess>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn spawn(&self, command: &str, work_dir: Option<&std::path::Path>) -> Result<String> {
        let id = Uuid::new_v4().to_string()[..8].to_string();

        #[cfg(target_os = "windows")]
        let (shell, flag) = ("cmd", "/C");
        #[cfg(not(target_os = "windows"))]
        let (shell, flag) = ("bash", "-c");

        let mut cmd = Command::new(shell);
        cmd.arg(flag)
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(wd) = work_dir {
            cmd.current_dir(wd);
        }

        let mut child = cmd.spawn()?;

        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let exited = Arc::new(Mutex::new(None::<i32>));

        // 读取 stdout 的后台线程
        let stdout_buf_clone = stdout_buf.clone();
        if let Some(stdout) = child.stdout.take() {
            std::thread::spawn(move || {
                let mut reader = std::io::BufReader::new(stdout);
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let text = String::from_utf8_lossy(&buf[..n]);
                            let mut lock = stdout_buf_clone.lock().unwrap();
                            lock.push_str(&text);
                            // 截断过长的缓冲
                            if lock.lines().count() > MAX_BUFFER_LINES {
                                let lines: Vec<&str> = lock.lines().collect();
                                let trimmed = lines[lines.len() - MAX_BUFFER_LINES..].join("\n");
                                *lock = format!("[...前 {} 行已截断...]\n{}", lines.len() - MAX_BUFFER_LINES, trimmed);
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        // 读取 stderr 的后台线程
        let stderr_buf_clone = stderr_buf.clone();
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let mut reader = std::io::BufReader::new(stderr);
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let text = String::from_utf8_lossy(&buf[..n]);
                            stderr_buf_clone.lock().unwrap().push_str(&text);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        let proc = BackgroundProcess {
            child,
            stdout_buf,
            stderr_buf,
            started_at: Instant::now(),
            command: command.to_string(),
            exited,
        };

        self.processes.lock().unwrap().insert(id.clone(), proc);
        self.cleanup();

        Ok(id)
    }

    pub fn get_output(&self, id: &str, block: bool) -> Result<ProcessOutput> {
        let mut procs = self.processes.lock().unwrap();
        let proc = procs.get_mut(id)
            .ok_or_else(|| anyhow!("进程 {} 不存在", id))?;

        if block {
            // 释放锁，等待完成
            drop(procs);
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
                let mut procs = self.processes.lock().unwrap();
                let proc = procs.get_mut(id).unwrap();
                match proc.child.try_wait() {
                    Ok(Some(status)) => {
                        *proc.exited.lock().unwrap() = Some(status.code().unwrap_or(-1));
                        let stdout = proc.stdout_buf.lock().unwrap().clone();
                        let stderr = proc.stderr_buf.lock().unwrap().clone();
                        return Ok(ProcessOutput {
                            stdout,
                            stderr,
                            exited: true,
                            exit_code: status.code(),
                        });
                    }
                    Ok(None) => continue,
                    Err(e) => return Err(anyhow!("等待进程失败: {}", e)),
                }
            }
        }

        // 非阻塞：检查状态
        let exited = match proc.child.try_wait() {
            Ok(Some(status)) => {
                *proc.exited.lock().unwrap() = Some(status.code().unwrap_or(-1));
                true
            }
            _ => proc.exited.lock().unwrap().is_some(),
        };

        let stdout = proc.stdout_buf.lock().unwrap().clone();
        let stderr = proc.stderr_buf.lock().unwrap().clone();
        let exit_code = *proc.exited.lock().unwrap();

        Ok(ProcessOutput {
            stdout,
            stderr,
            exited,
            exit_code,
        })
    }

    pub fn kill(&self, id: &str) -> Result<()> {
        let mut procs = self.processes.lock().unwrap();
        let proc = procs.get_mut(id)
            .ok_or_else(|| anyhow!("进程 {} 不存在", id))?;

        proc.child.kill().map_err(|e| anyhow!("终止进程失败: {}", e))?;
        let _ = proc.child.wait();
        *proc.exited.lock().unwrap() = Some(-9);
        Ok(())
    }

    pub fn list(&self) -> Vec<(String, String, bool)> {
        let procs = self.processes.lock().unwrap();
        procs.iter().map(|(id, p)| {
            let exited = p.exited.lock().unwrap().is_some();
            (id.clone(), p.command.clone(), exited)
        }).collect()
    }

    fn cleanup(&self) {
        let mut procs = self.processes.lock().unwrap();
        // 移除已完成且超过上限的旧进程
        let completed: Vec<String> = procs.iter()
            .filter(|(_, p)| p.exited.lock().unwrap().is_some())
            .map(|(id, _)| id.clone())
            .collect();

        if completed.len() > MAX_RETAINED {
            let to_remove = completed.len() - MAX_RETAINED;
            for id in completed.into_iter().take(to_remove) {
                procs.remove(&id);
            }
        }
    }
}
```

**Step 3: Register in mod.rs**

```rust
pub mod process_manager;
```

**Step 4: Run tests, commit**

```bash
git commit -m "feat(agent): 添加 ProcessManager — Shell 后台进程管理基础设施"
```

---

## Task 8: bash_output 和 bash_kill 工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/bash_output.rs`
- Create: `apps/runtime/src-tauri/src/agent/tools/bash_kill.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/bash.rs` (添加 background 支持)
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_bash_background.rs`

**Step 1: Write the failing test**

```rust
// tests/test_bash_background.rs
use serde_json::json;
use std::thread;
use std::time::Duration;

use workclaw_runtime::agent::tools::{BashTool, BashOutputTool, BashKillTool};
use workclaw_runtime::agent::tools::process_manager::ProcessManager;
use workclaw_runtime::agent::types::{Tool, ToolContext};
use std::sync::Arc;

#[test]
fn test_bash_background_mode() {
    let pm = Arc::new(ProcessManager::new());
    let tool = BashTool::with_process_manager(pm.clone());
    let ctx = ToolContext::default();

    #[cfg(target_os = "windows")]
    let result = tool.execute(json!({"command": "echo background_test", "background": true}), &ctx).unwrap();
    #[cfg(not(target_os = "windows"))]
    let result = tool.execute(json!({"command": "echo background_test", "background": true}), &ctx).unwrap();

    // 后台模式返回 process_id
    assert!(result.contains("process_id"));
}

#[test]
fn test_bash_output_tool() {
    let pm = Arc::new(ProcessManager::new());

    #[cfg(target_os = "windows")]
    let id = pm.spawn("echo output_test", None).unwrap();
    #[cfg(not(target_os = "windows"))]
    let id = pm.spawn("echo output_test", None).unwrap();

    thread::sleep(Duration::from_millis(500));

    let tool = BashOutputTool::new(pm.clone());
    let ctx = ToolContext::default();
    let result = tool.execute(json!({"process_id": id}), &ctx).unwrap();

    assert!(result.contains("output_test"));
}

#[test]
fn test_bash_kill_tool() {
    let pm = Arc::new(ProcessManager::new());

    #[cfg(target_os = "windows")]
    let id = pm.spawn("ping -n 100 127.0.0.1", None).unwrap();
    #[cfg(not(target_os = "windows"))]
    let id = pm.spawn("sleep 100", None).unwrap();

    thread::sleep(Duration::from_millis(200));

    let tool = BashKillTool::new(pm.clone());
    let ctx = ToolContext::default();
    let result = tool.execute(json!({"process_id": id}), &ctx).unwrap();

    assert!(result.contains("已终止"));
}
```

**Step 2: Modify BashTool to support background**

在 `bash.rs` 中添加 `with_process_manager` 构造函数和 `background` 参数支持。当 `background: true` 时，使用 ProcessManager 启动并返回 process_id。

**Step 3: Implement BashOutputTool and BashKillTool**

```rust
// src/agent/tools/bash_output.rs
use crate::agent::tools::process_manager::ProcessManager;
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct BashOutputTool {
    pm: Arc<ProcessManager>,
}

impl BashOutputTool {
    pub fn new(pm: Arc<ProcessManager>) -> Self {
        Self { pm }
    }
}

impl Tool for BashOutputTool {
    fn name(&self) -> &str { "bash_output" }
    fn description(&self) -> &str { "获取后台进程的输出。设置 block=true 等待完成" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "process_id": { "type": "string", "description": "进程 ID" },
                "block": { "type": "boolean", "description": "是否阻塞等待完成（默认 false）" }
            },
            "required": ["process_id"]
        })
    }
    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let id = input["process_id"].as_str()
            .ok_or_else(|| anyhow!("缺少 process_id"))?;
        let block = input["block"].as_bool().unwrap_or(false);
        let output = self.pm.get_output(id, block)?;
        Ok(format!(
            "状态: {}\n退出码: {}\nstdout:\n{}\nstderr:\n{}",
            if output.exited { "已完成" } else { "运行中" },
            output.exit_code.map(|c| c.to_string()).unwrap_or("N/A".into()),
            output.stdout,
            output.stderr
        ))
    }
}
```

```rust
// src/agent/tools/bash_kill.rs
use crate::agent::tools::process_manager::ProcessManager;
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct BashKillTool {
    pm: Arc<ProcessManager>,
}

impl BashKillTool {
    pub fn new(pm: Arc<ProcessManager>) -> Self {
        Self { pm }
    }
}

impl Tool for BashKillTool {
    fn name(&self) -> &str { "bash_kill" }
    fn description(&self) -> &str { "终止后台进程" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "process_id": { "type": "string", "description": "要终止的进程 ID" }
            },
            "required": ["process_id"]
        })
    }
    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let id = input["process_id"].as_str()
            .ok_or_else(|| anyhow!("缺少 process_id"))?;
        self.pm.kill(id)?;
        Ok(format!("已终止进程 {}", id))
    }
}
```

**Step 4: Register, run tests, commit**

```bash
git commit -m "feat(agent): L3 完成 — bash 后台模式 + bash_output + bash_kill"
```

---

## Task 9: Sidecar 浏览器工具扩展 — 新增端点

**Files:**
- Modify: `apps/runtime/sidecar/src/browser.ts` (扩展 BrowserController)
- Modify: `apps/runtime/sidecar/src/index.ts` (添加新路由)
- Modify: `apps/runtime/sidecar/package.json` (添加 stealth 依赖)

**Step 1: 安装 stealth 依赖**

```bash
cd apps/runtime/sidecar && pnpm add playwright-extra puppeteer-extra-plugin-stealth
```

**Step 2: 扩展 browser.ts**

在现有 `BrowserController` 基础上添加以下方法：

```typescript
// apps/runtime/sidecar/src/browser.ts
import { chromium } from "playwright-extra";
import stealth from "puppeteer-extra-plugin-stealth";

chromium.use(stealth());

export class BrowserController {
    private browser: Browser | null = null;
    private page: Page | null = null;

    // ---- 新增方法 ----

    async launch(options?: { headless?: boolean; viewport?: { width: number; height: number } }): Promise<string> {
        if (this.browser) {
            return "浏览器已在运行";
        }
        const headless = options?.headless ?? true;
        this.browser = await chromium.launch({ headless });
        const context = await this.browser.newContext({
            viewport: options?.viewport || { width: 1280, height: 720 },
        });
        this.page = await context.newPage();
        return "浏览器已启动";
    }

    async type(selector: string, text: string, delay?: number): Promise<string> {
        await this.ensureBrowser();
        await this.page!.fill(selector, "");
        await this.page!.type(selector, text, { delay: delay || 0 });
        return `已输入 "${text}" 到 ${selector}`;
    }

    async scroll(direction: string, amount?: number): Promise<string> {
        await this.ensureBrowser();
        const scrollAmount = amount || 500;
        switch (direction) {
            case "up":
                await this.page!.evaluate((a) => window.scrollBy(0, -a), scrollAmount);
                break;
            case "down":
                await this.page!.evaluate((a) => window.scrollBy(0, a), scrollAmount);
                break;
            case "to_top":
                await this.page!.evaluate(() => window.scrollTo(0, 0));
                break;
            case "to_bottom":
                await this.page!.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
                break;
        }
        return `已滚动 ${direction}`;
    }

    async hover(selector: string): Promise<string> {
        await this.ensureBrowser();
        await this.page!.hover(selector);
        return `已悬停 ${selector}`;
    }

    async pressKey(key: string, modifiers?: string[]): Promise<string> {
        await this.ensureBrowser();
        const combo = modifiers?.length ? `${modifiers.join("+")}+${key}` : key;
        await this.page!.keyboard.press(combo);
        return `已按键 ${combo}`;
    }

    async getDOM(selector?: string, maxDepth?: number): Promise<string> {
        await this.ensureBrowser();
        const depth = maxDepth || 5;
        const dom = await this.page!.evaluate(({ sel, maxD }: { sel?: string; maxD: number }) => {
            function simplify(el: Element, d: number): string {
                if (d <= 0) return "...";
                const tag = el.tagName.toLowerCase();
                const id = el.id ? `#${el.id}` : "";
                const cls = el.className && typeof el.className === "string"
                    ? `.${el.className.split(" ").filter(Boolean).join(".")}` : "";
                const text = el.childNodes.length === 1 && el.childNodes[0].nodeType === 3
                    ? el.textContent?.trim().substring(0, 100) || "" : "";
                const children = Array.from(el.children)
                    .map((c) => simplify(c, d - 1))
                    .filter(Boolean)
                    .join("\n");
                const indent = "  ".repeat(maxD - d);
                if (text) return `${indent}<${tag}${id}${cls}>${text}</${tag}>`;
                if (children) return `${indent}<${tag}${id}${cls}>\n${children}\n${indent}</${tag}>`;
                return `${indent}<${tag}${id}${cls} />`;
            }
            const root = sel ? document.querySelector(sel) : document.body;
            return root ? simplify(root, maxD) : "未找到元素";
        }, { sel: selector, maxD: depth });
        return dom;
    }

    async waitFor(options: { selector?: string; condition?: string; timeout?: number }): Promise<string> {
        await this.ensureBrowser();
        const timeout = options.timeout || 10000;
        if (options.selector) {
            await this.page!.waitForSelector(options.selector, { timeout });
            return `元素 ${options.selector} 已出现`;
        } else if (options.condition) {
            await this.page!.waitForFunction(options.condition, { timeout });
            return `条件已满足`;
        }
        return "未指定等待条件";
    }

    async goBack(): Promise<string> {
        await this.ensureBrowser();
        await this.page!.goBack();
        return `已后退到 ${this.page!.url()}`;
    }

    async goForward(): Promise<string> {
        await this.ensureBrowser();
        await this.page!.goForward();
        return `已前进到 ${this.page!.url()}`;
    }

    async reload(): Promise<string> {
        await this.ensureBrowser();
        await this.page!.reload();
        return `已刷新 ${this.page!.url()}`;
    }

    async getState(): Promise<string> {
        if (!this.page) {
            return JSON.stringify({ running: false });
        }
        return JSON.stringify({
            running: true,
            url: this.page.url(),
            title: await this.page.title(),
        });
    }
}
```

**Step 3: 在 index.ts 添加新路由**

在 `apps/runtime/sidecar/src/index.ts` 中添加 10 个新端点：

```typescript
app.post('/api/browser/launch', async (c) => { ... browser.launch(body) });
app.post('/api/browser/type', async (c) => { ... browser.type(selector, text, delay) });
app.post('/api/browser/scroll', async (c) => { ... browser.scroll(direction, amount) });
app.post('/api/browser/hover', async (c) => { ... browser.hover(selector) });
app.post('/api/browser/press_key', async (c) => { ... browser.pressKey(key, modifiers) });
app.post('/api/browser/get_dom', async (c) => { ... browser.getDOM(selector, max_depth) });
app.post('/api/browser/wait_for', async (c) => { ... browser.waitFor(options) });
app.post('/api/browser/go_back', async (c) => { ... browser.goBack() });
app.post('/api/browser/go_forward', async (c) => { ... browser.goForward() });
app.post('/api/browser/reload', async (c) => { ... browser.reload() });
app.post('/api/browser/get_state', async (c) => { ... browser.getState() });
```

**Step 4: 构建 sidecar**

```bash
cd apps/runtime/sidecar && pnpm build
```

**Step 5: Commit**

```bash
git commit -m "feat(sidecar): 浏览器自动化扩展 — 15 个端点 + stealth 反检测"
```

---

## Task 10: Rust 侧浏览器工具动态注册

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/browser_tools.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Test: `apps/runtime/src-tauri/tests/test_browser_tools.rs`

**Step 1: Write the failing test**

```rust
// tests/test_browser_tools.rs
use workclaw_runtime::agent::registry::ToolRegistry;
use workclaw_runtime::agent::tools::browser_tools::register_browser_tools;

#[test]
fn test_browser_tools_registered() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    let tools = registry.get_tool_definitions();
    assert_eq!(tools.len(), 15);

    let names: Vec<String> = tools.iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();

    assert!(names.contains(&"browser_launch".to_string()));
    assert!(names.contains(&"browser_navigate".to_string()));
    assert!(names.contains(&"browser_click".to_string()));
    assert!(names.contains(&"browser_type".to_string()));
    assert!(names.contains(&"browser_scroll".to_string()));
    assert!(names.contains(&"browser_hover".to_string()));
    assert!(names.contains(&"browser_press_key".to_string()));
    assert!(names.contains(&"browser_screenshot".to_string()));
    assert!(names.contains(&"browser_get_dom".to_string()));
    assert!(names.contains(&"browser_evaluate".to_string()));
    assert!(names.contains(&"browser_wait_for".to_string()));
    assert!(names.contains(&"browser_go_back".to_string()));
    assert!(names.contains(&"browser_go_forward".to_string()));
    assert!(names.contains(&"browser_reload".to_string()));
    assert!(names.contains(&"browser_get_state".to_string()));
}

#[test]
fn test_browser_tools_schema() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    let tool = registry.get("browser_navigate").unwrap();
    let schema = tool.input_schema();
    assert!(schema["properties"]["url"].is_object());
}
```

**Step 2: Implement register_browser_tools**

```rust
// src/agent/tools/browser_tools.rs
use crate::agent::registry::ToolRegistry;
use crate::agent::tools::SidecarBridgeTool;
use serde_json::json;
use std::sync::Arc;

pub fn register_browser_tools(registry: &ToolRegistry, sidecar_url: &str) {
    let url = sidecar_url.to_string();

    let tools: Vec<(&str, &str, &str, serde_json::Value)> = vec![
        ("browser_launch", "启动浏览器实例", "/api/browser/launch", json!({
            "type": "object",
            "properties": {
                "headless": {"type": "boolean", "description": "是否无头模式（默认 true）"},
                "viewport": {"type": "object", "description": "视口大小", "properties": {
                    "width": {"type": "integer"}, "height": {"type": "integer"}
                }}
            }
        })),
        ("browser_navigate", "导航到指定 URL", "/api/browser/navigate", json!({
            "type": "object",
            "properties": {"url": {"type": "string", "description": "目标 URL"}},
            "required": ["url"]
        })),
        ("browser_click", "点击页面元素（CSS 选择器或坐标）", "/api/browser/click", json!({
            "type": "object",
            "properties": {
                "selector": {"type": "string", "description": "CSS 选择器"},
                "x": {"type": "number", "description": "X 坐标"},
                "y": {"type": "number", "description": "Y 坐标"}
            }
        })),
        ("browser_type", "在输入框中输入文本", "/api/browser/type", json!({
            "type": "object",
            "properties": {
                "selector": {"type": "string", "description": "目标输入框的 CSS 选择器"},
                "text": {"type": "string", "description": "要输入的文本"},
                "delay": {"type": "integer", "description": "每字符延迟（毫秒）"}
            },
            "required": ["selector", "text"]
        })),
        ("browser_scroll", "滚动页面", "/api/browser/scroll", json!({
            "type": "object",
            "properties": {
                "direction": {"type": "string", "enum": ["up", "down", "to_top", "to_bottom"], "description": "滚动方向"},
                "amount": {"type": "integer", "description": "滚动像素（默认 500）"}
            },
            "required": ["direction"]
        })),
        ("browser_hover", "悬停在页面元素上", "/api/browser/hover", json!({
            "type": "object",
            "properties": {"selector": {"type": "string", "description": "CSS 选择器"}},
            "required": ["selector"]
        })),
        ("browser_press_key", "按下键盘按键", "/api/browser/press_key", json!({
            "type": "object",
            "properties": {
                "key": {"type": "string", "description": "按键名（Enter, Tab, Escape 等）"},
                "modifiers": {"type": "array", "items": {"type": "string"}, "description": "修饰键（Shift, Control, Alt）"}
            },
            "required": ["key"]
        })),
        ("browser_screenshot", "截取页面截图", "/api/browser/screenshot", json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "保存路径（可选，默认返回 base64）"},
                "selector": {"type": "string", "description": "仅截取指定元素"},
                "full_page": {"type": "boolean", "description": "是否全页截图"}
            }
        })),
        ("browser_get_dom", "提取页面简化 DOM 结构", "/api/browser/get_dom", json!({
            "type": "object",
            "properties": {
                "selector": {"type": "string", "description": "起始元素（默认 body）"},
                "max_depth": {"type": "integer", "description": "最大深度（默认 5）"}
            }
        })),
        ("browser_evaluate", "在页面上下文中执行 JavaScript", "/api/browser/evaluate", json!({
            "type": "object",
            "properties": {"script": {"type": "string", "description": "要执行的 JavaScript 代码"}},
            "required": ["script"]
        })),
        ("browser_wait_for", "等待元素出现或条件满足", "/api/browser/wait_for", json!({
            "type": "object",
            "properties": {
                "selector": {"type": "string", "description": "等待出现的 CSS 选择器"},
                "condition": {"type": "string", "description": "等待满足的 JS 条件表达式"},
                "timeout": {"type": "integer", "description": "超时毫秒（默认 10000）"}
            }
        })),
        ("browser_go_back", "浏览器后退", "/api/browser/go_back", json!({"type": "object", "properties": {}})),
        ("browser_go_forward", "浏览器前进", "/api/browser/go_forward", json!({"type": "object", "properties": {}})),
        ("browser_reload", "刷新当前页面", "/api/browser/reload", json!({"type": "object", "properties": {}})),
        ("browser_get_state", "获取浏览器当前状态（URL、标题、加载状态）", "/api/browser/get_state", json!({"type": "object", "properties": {}})),
    ];

    for (name, desc, endpoint, schema) in tools {
        registry.register(Arc::new(SidecarBridgeTool::new(
            url.clone(), endpoint.to_string(),
            name.to_string(), desc.to_string(), schema,
        )));
    }
}
```

**Step 3: Register in mod.rs, run tests, commit**

```bash
git commit -m "feat(agent): L4 完成 — 15 个浏览器工具 Rust 侧动态注册"
```

---

## Task 11: 系统工具 — screenshot 和 open_in_folder

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/screenshot.rs`
- Create: `apps/runtime/src-tauri/src/agent/tools/open_in_folder.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_system_tools.rs`

**Step 1: Write tests**

```rust
// tests/test_system_tools.rs
use serde_json::json;
use tempfile::TempDir;

use workclaw_runtime::agent::tools::{ScreenshotTool, OpenInFolderTool};
use workclaw_runtime::agent::types::{Tool, ToolContext};

#[test]
fn test_screenshot_schema() {
    let tool = ScreenshotTool;
    assert_eq!(tool.name(), "screenshot");
    let schema = tool.input_schema();
    assert!(schema["properties"]["path"].is_object());
}

#[test]
fn test_open_in_folder_schema() {
    let tool = OpenInFolderTool;
    assert_eq!(tool.name(), "open_in_folder");
    let schema = tool.input_schema();
    assert!(schema["properties"]["path"].is_object());
}

#[test]
fn test_open_in_folder_nonexistent() {
    let tool = OpenInFolderTool;
    let ctx = ToolContext::default();
    let result = tool.execute(json!({"path": "/nonexistent/path"}), &ctx);
    assert!(result.is_err());
}
```

**Step 2: Implement**

```rust
// src/agent/tools/screenshot.rs
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;

pub struct ScreenshotTool;

impl Tool for ScreenshotTool {
    fn name(&self) -> &str { "screenshot" }
    fn description(&self) -> &str { "截取屏幕截图并保存到指定路径" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "截图保存路径" }
            },
            "required": ["path"]
        })
    }
    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"].as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let checked = ctx.check_path(path)?;

        #[cfg(target_os = "windows")]
        {
            // 使用 PowerShell 截图
            let script = format!(
                "Add-Type -AssemblyName System.Windows.Forms; \
                 $bmp = New-Object System.Drawing.Bitmap([System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width, [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height); \
                 $g = [System.Drawing.Graphics]::FromImage($bmp); \
                 $g.CopyFromScreen(0, 0, 0, 0, $bmp.Size); \
                 $bmp.Save('{}'); \
                 $g.Dispose(); $bmp.Dispose()",
                checked.display()
            );
            Command::new("powershell")
                .args(["-Command", &script])
                .output()
                .map_err(|e| anyhow!("截图失败: {}", e))?;
        }

        #[cfg(target_os = "macos")]
        {
            Command::new("screencapture")
                .arg(checked.to_str().unwrap())
                .output()
                .map_err(|e| anyhow!("截图失败: {}", e))?;
        }

        #[cfg(target_os = "linux")]
        {
            Command::new("gnome-screenshot")
                .args(["-f", checked.to_str().unwrap()])
                .output()
                .or_else(|_| {
                    Command::new("import")
                        .args(["-window", "root", checked.to_str().unwrap()])
                        .output()
                })
                .map_err(|e| anyhow!("截图失败: {}", e))?;
        }

        Ok(format!("截图已保存到 {}", path))
    }
}
```

```rust
// src/agent/tools/open_in_folder.rs
use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct OpenInFolderTool;

impl Tool for OpenInFolderTool {
    fn name(&self) -> &str { "open_in_folder" }
    fn description(&self) -> &str { "在系统文件管理器中显示文件或目录" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "要在文件管理器中显示的路径" }
            },
            "required": ["path"]
        })
    }
    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path = input["path"].as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let checked = ctx.check_path(path)?;

        if !checked.exists() {
            return Err(anyhow!("路径不存在: {}", path));
        }

        opener::reveal(&checked)
            .map_err(|e| anyhow!("打开文件管理器失败: {}", e))?;

        Ok(format!("已在文件管理器中打开 {}", path))
    }
}
```

**Step 3: 添加 opener 依赖到 Cargo.toml**

```toml
opener = "0.7"
```

**Step 4: Register, run tests, commit**

```bash
git commit -m "feat(agent): L5 完成 — screenshot + open_in_folder 系统工具"
```

---

## Task 12: 注册表重构 + chat.rs 集成

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Modify: `apps/runtime/src-tauri/tests/test_registry.rs`

**Step 1: 重构 registry.rs**

将 `with_file_tools()` 重命名为 `with_standard_tools()`，将新工具分组注册：

```rust
impl ToolRegistry {
    /// 基础工具集：文件操作 + Shell + 信息获取 + 任务管理
    pub fn with_standard_tools() -> Self {
        let registry = Self::new();
        // 文件操作（10 个）
        registry.register(Arc::new(ReadFileTool));
        registry.register(Arc::new(WriteFileTool));
        registry.register(Arc::new(GlobTool));
        registry.register(Arc::new(GrepTool));
        registry.register(Arc::new(EditTool));
        registry.register(Arc::new(ListDirTool));
        registry.register(Arc::new(FileStatTool));
        registry.register(Arc::new(FileDeleteTool));
        registry.register(Arc::new(FileMoveTool));
        registry.register(Arc::new(FileCopyTool));
        // Shell（3 个）— bash_output 和 bash_kill 在 chat.rs 中动态注册（需 ProcessManager）
        registry.register(Arc::new(BashTool));
        // 其他
        registry.register(Arc::new(TodoWriteTool::new()));
        registry.register(Arc::new(WebFetchTool));
        // 系统工具（2 个）
        registry.register(Arc::new(ScreenshotTool));
        registry.register(Arc::new(OpenInFolderTool));
        registry
    }

    /// 向后兼容别名
    pub fn with_file_tools() -> Self {
        Self::with_standard_tools()
    }
}
```

**Step 2: 在 chat.rs 中注册 L3 工具（bash_output, bash_kill）和 browser_tools**

在 `send_message()` 动态注册段落中添加：

```rust
// 注册后台进程管理工具
let pm = Arc::new(ProcessManager::new());
registry.register(Arc::new(BashOutputTool::new(pm.clone())));
registry.register(Arc::new(BashKillTool::new(pm.clone())));
// 修改 BashTool 为带 ProcessManager 的版本
registry.unregister("bash");
registry.register(Arc::new(BashTool::with_process_manager(pm)));

// 注册浏览器工具（仅当 sidecar 可用时）
register_browser_tools(&registry, "http://localhost:8765");
```

**Step 3: 更新 test_registry.rs**

```rust
#[test]
fn test_registry_has_standard_tools() {
    let registry = ToolRegistry::with_standard_tools();
    let tools = registry.get_tool_definitions();
    assert_eq!(tools.len(), 15); // 10 文件 + bash + todo_write + web_fetch + screenshot + open_in_folder
}
```

**Step 4: Run all tests**

Run: `cd apps/runtime/src-tauri && cargo test`
Expected: ALL PASS

**Step 5: Commit**

```bash
git commit -m "refactor(agent): 注册表重构 with_standard_tools + chat.rs 集成全部 37 工具"
```

---

## Task 13: 全量集成测试

**Files:**
- Create: `apps/runtime/src-tauri/tests/test_tools_complete.rs`

**Step 1: Write comprehensive integration test**

```rust
// tests/test_tools_complete.rs
use workclaw_runtime::agent::registry::ToolRegistry;
use workclaw_runtime::agent::tools::browser_tools::register_browser_tools;
use workclaw_runtime::agent::tools::process_manager::ProcessManager;
use workclaw_runtime::agent::tools::{BashOutputTool, BashKillTool};
use std::sync::Arc;

/// 验证完整 Agent 工具集（不含需要运行时依赖的 5 个高级工具）
#[test]
fn test_all_standard_and_browser_tools() {
    let registry = ToolRegistry::with_standard_tools();

    // 注册 L3 进程管理工具
    let pm = Arc::new(ProcessManager::new());
    registry.register(Arc::new(BashOutputTool::new(pm.clone())));
    registry.register(Arc::new(BashKillTool::new(pm)));

    // 注册 L4 浏览器工具
    register_browser_tools(&registry, "http://localhost:8765");

    let tools = registry.get_tool_definitions();
    let names: Vec<String> = tools.iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();

    // 15 标准 + 2 进程管理 + 15 浏览器 = 32 个
    // (剩下 5 个 memory/ask_user/web_search/task/compact 在 chat.rs 动态注册)
    assert_eq!(names.len(), 32);

    // 抽样检查关键工具
    let expected = vec![
        "read_file", "write_file", "edit", "glob", "grep",
        "list_dir", "file_stat", "file_delete", "file_move", "file_copy",
        "bash", "bash_output", "bash_kill",
        "todo_write", "web_fetch",
        "screenshot", "open_in_folder",
        "browser_launch", "browser_navigate", "browser_click",
        "browser_type", "browser_screenshot", "browser_evaluate",
        "browser_get_dom", "browser_wait_for", "browser_get_state",
    ];
    for name in expected {
        assert!(names.contains(&name.to_string()), "缺少工具: {}", name);
    }
}
```

**Step 2: Run all tests**

Run: `cd apps/runtime/src-tauri && cargo test`
Expected: ALL PASS

**Step 3: Commit**

```bash
git commit -m "test(agent): 全量工具集成测试 — 验证 37 个工具完整注册"
```

---

## Task 14: 构建验证

**Step 1: 构建 sidecar**

```bash
cd apps/runtime/sidecar && pnpm install && pnpm build
```

**Step 2: 构建 Rust 后端**

```bash
cd apps/runtime/src-tauri && cargo build
```

**Step 3: 运行全量测试**

```bash
cd apps/runtime/src-tauri && cargo test
```

**Step 4: 最终 commit**

```bash
git commit -m "build: 验证全量构建通过 — 37 工具 Agent 补全完成"
```

---

## 实现总览

| Task | 层级 | 内容 | 新增/修改文件 |
|------|------|------|-------------|
| 1-5 | L2 | 5 个文件扩展工具 | 5 new .rs + 5 tests |
| 6 | L2 | 注册表更新 + 测试 | 2 modified |
| 7 | L3 | ProcessManager | 1 new .rs + 1 test |
| 8 | L3 | bash_output + bash_kill | 2 new .rs + 1 test + bash.rs modified |
| 9 | L4 | Sidecar 浏览器扩展 | browser.ts + index.ts modified |
| 10 | L4 | Rust 浏览器工具注册 | 1 new .rs + 1 test |
| 11 | L5 | 系统工具 | 2 new .rs + 1 test |
| 12 | — | 注册表重构 + 集成 | registry.rs + chat.rs + lib.rs modified |
| 13 | — | 全量集成测试 | 1 test |
| 14 | — | 构建验证 | 无新文件 |
