# Skill Runtime Enhancement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 WorkClaw Runtime 从基础 ReAct 引擎提升到接近 Claude Code 体验，分 4 个 Phase、17 个 Task 逐步实现。

**Architecture:** 自底向上：Phase 1 补齐工具层 → Phase 2 Skill 元数据加载 → Phase 3 多 Agent 协调 → Phase 4 高级特性（权限/Web/Memory/AskUser）。

**Tech Stack:** Rust (Tauri backend), TypeScript (React frontend), Node.js (Sidecar), SQLite, serde_yaml

**Design doc:** `docs/plans/2026-02-22-skill-runtime-enhancement-design.md`

---

## Phase 1: 工具补齐

### Task 1: Edit 工具（精确文本替换）

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/edit_tool.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_edit_tool.rs`

**Step 1: Write the failing test**

Create `apps/runtime/src-tauri/tests/test_edit_tool.rs`:

```rust
use runtime_lib::agent::{Tool, ToolRegistry};
use serde_json::json;
use std::fs;
use std::sync::Arc;

#[test]
fn test_edit_replace_single() {
    let tool = runtime_lib::agent::tools::EditTool;

    let path = "test_edit_single.txt";
    fs::write(path, "Hello, World!\nGoodbye, World!").unwrap();

    let input = json!({
        "path": path,
        "old_string": "Hello",
        "new_string": "Hi"
    });
    let result = tool.execute(input).unwrap();
    assert!(result.contains("成功替换"));

    let content = fs::read_to_string(path).unwrap();
    assert_eq!(content, "Hi, World!\nGoodbye, World!");

    fs::remove_file(path).unwrap();
}

#[test]
fn test_edit_not_found() {
    let tool = runtime_lib::agent::tools::EditTool;

    let path = "test_edit_notfound.txt";
    fs::write(path, "Hello, World!").unwrap();

    let input = json!({
        "path": path,
        "old_string": "NONEXISTENT",
        "new_string": "replacement"
    });
    let result = tool.execute(input);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("未找到"));

    fs::remove_file(path).unwrap();
}

#[test]
fn test_edit_not_unique() {
    let tool = runtime_lib::agent::tools::EditTool;

    let path = "test_edit_notunique.txt";
    fs::write(path, "aaa bbb aaa").unwrap();

    let input = json!({
        "path": path,
        "old_string": "aaa",
        "new_string": "ccc"
    });
    let result = tool.execute(input);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("不唯一"));

    fs::remove_file(path).unwrap();
}

#[test]
fn test_edit_replace_all() {
    let tool = runtime_lib::agent::tools::EditTool;

    let path = "test_edit_replaceall.txt";
    fs::write(path, "aaa bbb aaa").unwrap();

    let input = json!({
        "path": path,
        "old_string": "aaa",
        "new_string": "ccc",
        "replace_all": true
    });
    let result = tool.execute(input).unwrap();
    assert!(result.contains("2"));

    let content = fs::read_to_string(path).unwrap();
    assert_eq!(content, "ccc bbb ccc");

    fs::remove_file(path).unwrap();
}

#[test]
fn test_edit_missing_params() {
    let tool = runtime_lib::agent::tools::EditTool;
    let result = tool.execute(json!({}));
    assert!(result.is_err());
}

#[test]
fn test_edit_registered() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(runtime_lib::agent::tools::EditTool));
    assert!(registry.get("edit").is_some());
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_edit_tool 2>&1`
Expected: FAIL with compilation error (EditTool not defined)

**Step 3: Write implementation**

Create `apps/runtime/src-tauri/src/agent/tools/edit_tool.rs`:

```rust
use crate::agent::types::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;

pub struct EditTool;

impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "在文件中精确替换文本。查找 old_string 并替换为 new_string。默认要求 old_string 在文件中唯一。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径"
                },
                "old_string": {
                    "type": "string",
                    "description": "要替换的原始文本"
                },
                "new_string": {
                    "type": "string",
                    "description": "替换后的文本"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "是否替换所有匹配（默认 false，要求唯一匹配）"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let old_string = input["old_string"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 old_string 参数"))?;
        let new_string = input["new_string"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 new_string 参数"))?;
        let replace_all = input["replace_all"].as_bool().unwrap_or(false);

        let content = fs::read_to_string(path)
            .map_err(|e| anyhow!("读取文件失败: {}", e))?;

        let count = content.matches(old_string).count();

        if count == 0 {
            return Err(anyhow!("未找到匹配文本: \"{}\"", old_string));
        }

        if !replace_all && count > 1 {
            return Err(anyhow!(
                "匹配不唯一（找到 {} 处），请提供更多上下文或使用 replace_all",
                count
            ));
        }

        let new_content = content.replace(old_string, new_string);
        fs::write(path, &new_content)
            .map_err(|e| anyhow!("写入文件失败: {}", e))?;

        Ok(format!("成功替换 {} 处，文件: {}", count, path))
    }
}
```

Modify `apps/runtime/src-tauri/src/agent/tools/mod.rs` — add:
```rust
mod edit_tool;
pub use edit_tool::EditTool;
```

Modify `apps/runtime/src-tauri/src/agent/registry.rs` — in `with_file_tools()` add:
```rust
use super::tools::EditTool;
// ...
registry.register(Arc::new(EditTool));
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_edit_tool -v 2>&1`
Expected: All 6 tests PASS

**Step 5: Run all existing tests**

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All tests PASS (including existing ones)

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/edit_tool.rs \
       apps/runtime/src-tauri/src/agent/tools/mod.rs \
       apps/runtime/src-tauri/src/agent/registry.rs \
       apps/runtime/src-tauri/tests/test_edit_tool.rs
git commit -m "feat(agent): 添加 Edit 工具（精确文本替换）"
```

---

### Task 2: 工具输出截断

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs:119-141`
- Test: `apps/runtime/src-tauri/tests/test_output_truncation.rs`

**Step 1: Write the failing test**

Create `apps/runtime/src-tauri/tests/test_output_truncation.rs`:

```rust
#[test]
fn test_truncate_output() {
    // 测试截断逻辑的辅助函数
    let long_output = "x".repeat(40_000);
    let truncated = runtime_lib::agent::executor::truncate_tool_output(&long_output, 30_000);
    assert!(truncated.len() < 31_000);
    assert!(truncated.contains("[输出已截断"));
    assert!(truncated.contains("40000"));

    // 短输出不截断
    let short_output = "hello world";
    let not_truncated = runtime_lib::agent::executor::truncate_tool_output(short_output, 30_000);
    assert_eq!(not_truncated, "hello world");
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_output_truncation 2>&1`
Expected: FAIL (function not found)

**Step 3: Write implementation**

In `apps/runtime/src-tauri/src/agent/executor.rs`, add public function:

```rust
const MAX_TOOL_OUTPUT_CHARS: usize = 30_000;

/// 截断过长的工具输出
pub fn truncate_tool_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        return output.to_string();
    }
    let truncated: String = output.chars().take(max_chars).collect();
    format!(
        "{}\n\n[输出已截断，共 {} 字符，已显示前 {} 字符]",
        truncated,
        output.len(),
        max_chars
    )
}
```

In `execute_turn` method, after tool execution result (around line 120-121), wrap the result:

```rust
// 在 let result = match self.registry.get(...) 之后
let result = truncate_tool_output(&result, MAX_TOOL_OUTPUT_CHARS);
```

**Step 4: Run tests**

Run: `cd apps/runtime/src-tauri && cargo test --test test_output_truncation -v 2>&1`
Expected: PASS

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/executor.rs \
       apps/runtime/src-tauri/tests/test_output_truncation.rs
git commit -m "feat(agent): 添加工具输出截断（超过 30000 字符自动截断）"
```

---

### Task 3: 上下文裁剪

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Test: `apps/runtime/src-tauri/tests/test_context_trimming.rs`

**Step 1: Write the failing test**

Create `apps/runtime/src-tauri/tests/test_context_trimming.rs`:

```rust
use serde_json::json;

#[test]
fn test_trim_messages_under_budget() {
    let messages = vec![
        json!({"role": "user", "content": "hello"}),
        json!({"role": "assistant", "content": "hi"}),
    ];
    let trimmed = runtime_lib::agent::executor::trim_messages(&messages, 10_000);
    assert_eq!(trimmed.len(), 2);
}

#[test]
fn test_trim_messages_over_budget() {
    let long_text = "x".repeat(5000);
    let messages = vec![
        json!({"role": "user", "content": long_text}),
        json!({"role": "assistant", "content": long_text}),
        json!({"role": "user", "content": long_text}),
        json!({"role": "assistant", "content": long_text}),
        json!({"role": "user", "content": "latest question"}),
    ];
    // 预算 3000 tokens ≈ 12000 字符，5 条消息总约 20000+ 字符
    let trimmed = runtime_lib::agent::executor::trim_messages(&messages, 3_000);
    // 应裁剪中间消息，保留第一条和最后几条
    assert!(trimmed.len() < 5);
    // 最后一条消息必须保留
    let last = trimmed.last().unwrap();
    assert_eq!(last["content"].as_str().unwrap(), "latest question");
    // 被裁剪的消息应有提示
    let has_trimmed_marker = trimmed.iter().any(|m| {
        m["content"].as_str().map_or(false, |c| c.contains("已省略"))
    });
    assert!(has_trimmed_marker);
}

#[test]
fn test_trim_preserves_first_and_last() {
    let text = "x".repeat(5000);
    let messages = vec![
        json!({"role": "user", "content": text}),
        json!({"role": "assistant", "content": text}),
        json!({"role": "user", "content": text}),
        json!({"role": "assistant", "content": text}),
        json!({"role": "user", "content": "final"}),
    ];
    let trimmed = runtime_lib::agent::executor::trim_messages(&messages, 2_000);
    // 第一条必须保留
    assert_eq!(trimmed.first().unwrap()["content"].as_str().unwrap(), &text);
    // 最后一条必须保留
    assert_eq!(trimmed.last().unwrap()["content"].as_str().unwrap(), "final");
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_context_trimming 2>&1`
Expected: FAIL (function not found)

**Step 3: Write implementation**

In `apps/runtime/src-tauri/src/agent/executor.rs`, add:

```rust
const CHARS_PER_TOKEN: usize = 4;
const DEFAULT_TOKEN_BUDGET: usize = 100_000; // ~400k 字符

/// 估算消息列表的 token 数
fn estimate_tokens(messages: &[Value]) -> usize {
    let total_chars: usize = messages.iter()
        .map(|m| {
            m["content"].as_str().map_or(0, |s| s.len())
                + m["content"].as_array().map_or(0, |arr| {
                    arr.iter().map(|v| serde_json::to_string(v).map_or(0, |s| s.len())).sum()
                })
        })
        .sum();
    total_chars / CHARS_PER_TOKEN
}

/// 裁剪消息列表到 token 预算内
/// 保留第一条消息和最后的消息，从第二条开始裁剪中间的
pub fn trim_messages(messages: &[Value], token_budget: usize) -> Vec<Value> {
    if messages.len() <= 2 || estimate_tokens(messages) <= token_budget {
        return messages.to_vec();
    }

    // 保留第一条和最后一条
    let first = &messages[0];
    let last = &messages[messages.len() - 1];

    // 从后往前累加，直到达到预算的 70%
    let budget_chars = token_budget * CHARS_PER_TOKEN * 70 / 100;
    let mut keep_from_end: Vec<&Value> = Vec::new();
    let mut char_count = first["content"].as_str().map_or(0, |s| s.len())
        + last["content"].as_str().map_or(0, |s| s.len());

    for msg in messages[1..messages.len()-1].iter().rev() {
        let msg_chars = msg["content"].as_str().map_or(0, |s| s.len())
            + msg["content"].as_array().map_or(0, |arr| {
                arr.iter().map(|v| serde_json::to_string(v).map_or(0, |s| s.len())).sum()
            });
        if char_count + msg_chars > budget_chars {
            break;
        }
        char_count += msg_chars;
        keep_from_end.push(msg);
    }
    keep_from_end.reverse();

    let trimmed_count = messages.len() - 2 - keep_from_end.len();
    let mut result = vec![first.clone()];

    if trimmed_count > 0 {
        result.push(json!({
            "role": "user",
            "content": format!("[前 {} 条消息已省略]", trimmed_count)
        }));
    }

    for msg in keep_from_end {
        result.push(msg.clone());
    }
    result.push(last.clone());

    result
}
```

In `execute_turn`, before calling LLM (around the `let response =` block), add:

```rust
// 上下文裁剪
let trimmed = trim_messages(&messages, DEFAULT_TOKEN_BUDGET);
// 使用 trimmed 替代 messages 调用 LLM（但保留 messages 的完整历史用于后续）
```

Replace `messages.clone()` in the LLM calls with `trimmed.clone()`.

**Step 4: Run tests**

Run: `cd apps/runtime/src-tauri && cargo test --test test_context_trimming -v 2>&1`
Expected: PASS

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/executor.rs \
       apps/runtime/src-tauri/tests/test_context_trimming.rs
git commit -m "feat(agent): 添加上下文裁剪（token 预算内保留首尾消息）"
```

---

### Task 4: TodoWrite 工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/todo_tool.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_todo_tool.rs`

**Step 1: Write the failing test**

Create `apps/runtime/src-tauri/tests/test_todo_tool.rs`:

```rust
use runtime_lib::agent::tools::TodoWriteTool;
use runtime_lib::agent::Tool;
use serde_json::json;

#[test]
fn test_todo_create_and_list() {
    let tool = TodoWriteTool::new();

    // 创建任务
    let result = tool.execute(json!({
        "action": "create",
        "subject": "实现 Edit 工具",
        "description": "精确替换文本"
    })).unwrap();
    assert!(result.contains("已创建"));

    // 列出任务
    let result = tool.execute(json!({"action": "list"})).unwrap();
    assert!(result.contains("实现 Edit 工具"));
    assert!(result.contains("pending"));
}

#[test]
fn test_todo_update_status() {
    let tool = TodoWriteTool::new();

    let result = tool.execute(json!({
        "action": "create",
        "subject": "Test task"
    })).unwrap();
    // 提取 ID
    let id = result.split("ID: ").nth(1).unwrap().trim();

    let result = tool.execute(json!({
        "action": "update",
        "id": id,
        "status": "in_progress"
    })).unwrap();
    assert!(result.contains("已更新"));

    let result = tool.execute(json!({"action": "list"})).unwrap();
    assert!(result.contains("in_progress"));
}

#[test]
fn test_todo_delete() {
    let tool = TodoWriteTool::new();

    let result = tool.execute(json!({
        "action": "create",
        "subject": "Will delete"
    })).unwrap();
    let id = result.split("ID: ").nth(1).unwrap().trim();

    let result = tool.execute(json!({
        "action": "delete",
        "id": id
    })).unwrap();
    assert!(result.contains("已删除"));

    let result = tool.execute(json!({"action": "list"})).unwrap();
    assert!(!result.contains("Will delete"));
}

#[test]
fn test_todo_missing_action() {
    let tool = TodoWriteTool::new();
    let result = tool.execute(json!({}));
    assert!(result.is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_todo_tool 2>&1`
Expected: FAIL

**Step 3: Write implementation**

Create `apps/runtime/src-tauri/src/agent/tools/todo_tool.rs`:

```rust
use crate::agent::types::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Debug)]
struct TodoItem {
    id: String,
    subject: String,
    description: String,
    status: String,
}

pub struct TodoWriteTool {
    items: RwLock<Vec<TodoItem>>,
}

impl TodoWriteTool {
    pub fn new() -> Self {
        Self {
            items: RwLock::new(Vec::new()),
        }
    }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todo_write"
    }

    fn description(&self) -> &str {
        "管理任务列表。支持 create/update/list/delete 操作。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "update", "list", "delete"],
                    "description": "操作类型"
                },
                "id": {
                    "type": "string",
                    "description": "任务 ID（update/delete 时必填）"
                },
                "subject": {
                    "type": "string",
                    "description": "任务标题（create 时必填）"
                },
                "description": {
                    "type": "string",
                    "description": "任务描述（可选）"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed"],
                    "description": "任务状态（update 时使用）"
                }
            },
            "required": ["action"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let action = input["action"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 action 参数"))?;

        match action {
            "create" => {
                let subject = input["subject"]
                    .as_str()
                    .ok_or_else(|| anyhow!("create 操作需要 subject 参数"))?;
                let description = input["description"].as_str().unwrap_or("").to_string();
                let id = Uuid::new_v4().to_string();
                let item = TodoItem {
                    id: id.clone(),
                    subject: subject.to_string(),
                    description,
                    status: "pending".to_string(),
                };
                self.items.write().unwrap().push(item);
                Ok(format!("已创建任务 ID: {}", id))
            }
            "update" => {
                let id = input["id"]
                    .as_str()
                    .ok_or_else(|| anyhow!("update 操作需要 id 参数"))?;
                let mut items = self.items.write().unwrap();
                let item = items.iter_mut().find(|i| i.id == id)
                    .ok_or_else(|| anyhow!("任务不存在: {}", id))?;
                if let Some(status) = input["status"].as_str() {
                    item.status = status.to_string();
                }
                if let Some(subject) = input["subject"].as_str() {
                    item.subject = subject.to_string();
                }
                if let Some(desc) = input["description"].as_str() {
                    item.description = desc.to_string();
                }
                Ok(format!("已更新任务: {}", id))
            }
            "list" => {
                let items = self.items.read().unwrap();
                if items.is_empty() {
                    return Ok("暂无任务".to_string());
                }
                let list: Vec<String> = items.iter().map(|item| {
                    format!("- [{}] {} (ID: {}){}",
                        item.status, item.subject, item.id,
                        if item.description.is_empty() { String::new() }
                        else { format!("\n  {}", item.description) })
                }).collect();
                Ok(list.join("\n"))
            }
            "delete" => {
                let id = input["id"]
                    .as_str()
                    .ok_or_else(|| anyhow!("delete 操作需要 id 参数"))?;
                let mut items = self.items.write().unwrap();
                let len_before = items.len();
                items.retain(|i| i.id != id);
                if items.len() == len_before {
                    return Err(anyhow!("任务不存在: {}", id));
                }
                Ok(format!("已删除任务: {}", id))
            }
            _ => Err(anyhow!("未知操作: {}", action)),
        }
    }
}
```

Modify `apps/runtime/src-tauri/src/agent/tools/mod.rs` — add:
```rust
mod todo_tool;
pub use todo_tool::TodoWriteTool;
```

Modify `apps/runtime/src-tauri/src/agent/registry.rs` — in `with_file_tools()` add:
```rust
use super::tools::TodoWriteTool;
// ...
registry.register(Arc::new(TodoWriteTool::new()));
```

**注意**: BashTool 也需要在 `with_file_tools()` 中注册（如果尚未注册）。检查现有代码确认。

**Step 4: Run tests**

Run: `cd apps/runtime/src-tauri && cargo test --test test_todo_tool -v 2>&1`
Expected: All PASS

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/todo_tool.rs \
       apps/runtime/src-tauri/src/agent/tools/mod.rs \
       apps/runtime/src-tauri/src/agent/registry.rs \
       apps/runtime/src-tauri/tests/test_todo_tool.rs
git commit -m "feat(agent): 添加 TodoWrite 工具（任务列表管理）"
```

---

## Checkpoint 1

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

Run: `cd apps/runtime && npx tsc --noEmit 2>&1`
Expected: Clean

Phase 1 交付：Edit 工具 + 输出截断 + 上下文裁剪 + TodoWrite 工具

---

## Phase 2: Skill 元数据与加载增强

### Task 5: Skill Frontmatter 解析

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/skill_config.rs`
- Modify: `apps/runtime/src-tauri/src/agent/mod.rs`
- Modify: `apps/runtime/src-tauri/Cargo.toml` (add serde_yaml)
- Test: `apps/runtime/src-tauri/tests/test_skill_config.rs`

**Step 1: Add serde_yaml dependency**

Modify `apps/runtime/src-tauri/Cargo.toml` — add to `[dependencies]`:
```toml
serde_yaml = "0.9"
```

Run: `cd apps/runtime/src-tauri && cargo check 2>&1`
Expected: PASS (dependency resolves)

**Step 2: Write the failing test**

Create `apps/runtime/src-tauri/tests/test_skill_config.rs`:

```rust
use runtime_lib::agent::skill_config::SkillConfig;

#[test]
fn test_parse_with_frontmatter() {
    let content = r#"---
name: test-skill
description: A test skill
allowed_tools:
  - read_file
  - edit
  - bash
model: gpt-4o
max_iterations: 5
---
You are a helpful assistant.

Do your best work.
"#;
    let config = SkillConfig::parse(content);
    assert_eq!(config.name.as_deref(), Some("test-skill"));
    assert_eq!(config.description.as_deref(), Some("A test skill"));
    assert_eq!(config.allowed_tools, Some(vec!["read_file".into(), "edit".into(), "bash".into()]));
    assert_eq!(config.model.as_deref(), Some("gpt-4o"));
    assert_eq!(config.max_iterations, Some(5));
    assert!(config.system_prompt.contains("You are a helpful assistant."));
    assert!(config.system_prompt.contains("Do your best work."));
}

#[test]
fn test_parse_without_frontmatter() {
    let content = "You are a helpful assistant.\n\nDo stuff.";
    let config = SkillConfig::parse(content);
    assert!(config.name.is_none());
    assert!(config.allowed_tools.is_none());
    assert_eq!(config.system_prompt, content);
}

#[test]
fn test_parse_empty_frontmatter() {
    let content = "---\n---\nJust a prompt.";
    let config = SkillConfig::parse(content);
    assert!(config.name.is_none());
    assert_eq!(config.system_prompt.trim(), "Just a prompt.");
}

#[test]
fn test_parse_empty_content() {
    let config = SkillConfig::parse("");
    assert!(config.name.is_none());
    assert_eq!(config.system_prompt, "");
}
```

**Step 3: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_skill_config 2>&1`
Expected: FAIL (module not found)

**Step 4: Write implementation**

Create `apps/runtime/src-tauri/src/agent/skill_config.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct SkillConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub max_iterations: Option<usize>,
    pub system_prompt: String,
}

#[derive(Deserialize, Default)]
struct FrontMatter {
    name: Option<String>,
    description: Option<String>,
    allowed_tools: Option<Vec<String>>,
    model: Option<String>,
    max_iterations: Option<usize>,
}

impl SkillConfig {
    pub fn parse(content: &str) -> Self {
        if !content.starts_with("---") {
            return Self {
                system_prompt: content.to_string(),
                ..Default::default()
            };
        }

        // 查找第二个 ---
        let rest = &content[3..];
        let end_pos = match rest.find("\n---") {
            Some(pos) => pos,
            None => {
                return Self {
                    system_prompt: content.to_string(),
                    ..Default::default()
                };
            }
        };

        let yaml_str = &rest[..end_pos];
        let prompt_start = 3 + end_pos + 4; // "---" + yaml + "\n---"
        let system_prompt = if prompt_start < content.len() {
            content[prompt_start..].trim_start_matches('\n').to_string()
        } else {
            String::new()
        };

        let fm: FrontMatter = serde_yaml::from_str(yaml_str).unwrap_or_default();

        Self {
            name: fm.name,
            description: fm.description,
            allowed_tools: fm.allowed_tools,
            model: fm.model,
            max_iterations: fm.max_iterations,
            system_prompt,
        }
    }
}
```

Modify `apps/runtime/src-tauri/src/agent/mod.rs` — add:
```rust
pub mod skill_config;
```

**Step 5: Run tests**

Run: `cd apps/runtime/src-tauri && cargo test --test test_skill_config -v 2>&1`
Expected: All PASS

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/skill_config.rs \
       apps/runtime/src-tauri/src/agent/mod.rs \
       apps/runtime/src-tauri/Cargo.toml \
       apps/runtime/src-tauri/tests/test_skill_config.rs
git commit -m "feat(agent): 添加 Skill Frontmatter 解析（YAML 元数据）"
```

---

### Task 6: 工具白名单执行

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Test: `apps/runtime/src-tauri/tests/test_tool_whitelist.rs`

**Step 1: Write the failing test**

Create `apps/runtime/src-tauri/tests/test_tool_whitelist.rs`:

```rust
use runtime_lib::agent::ToolRegistry;
use std::sync::Arc;

#[test]
fn test_get_filtered_definitions() {
    let registry = ToolRegistry::with_file_tools();

    // 无过滤
    let all = registry.get_tool_definitions();
    assert!(all.len() >= 5); // read_file, write_file, glob, grep, edit, todo_write, ...

    // 仅允许 read_file 和 glob
    let whitelist = vec!["read_file".to_string(), "glob".to_string()];
    let filtered = registry.get_filtered_tool_definitions(&whitelist);
    assert_eq!(filtered.len(), 2);
    let names: Vec<&str> = filtered.iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"glob"));
    assert!(!names.contains(&"write_file"));
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_tool_whitelist 2>&1`
Expected: FAIL (method not found)

**Step 3: Write implementation**

In `apps/runtime/src-tauri/src/agent/registry.rs`, add method:

```rust
/// 返回仅包含白名单中工具的定义
pub fn get_filtered_tool_definitions(&self, whitelist: &[String]) -> Vec<Value> {
    self.tools
        .read()
        .unwrap()
        .values()
        .filter(|t| whitelist.iter().any(|w| w == t.name()))
        .map(|t| {
            json!({
                "name": t.name(),
                "description": t.description(),
                "input_schema": t.input_schema(),
            })
        })
        .collect()
}
```

In `apps/runtime/src-tauri/src/commands/chat.rs`, modify `send_message`:
- After getting `system_prompt`, parse it with `SkillConfig::parse()`
- If `skill_config.allowed_tools` is `Some`, pass the whitelist to `execute_turn`

Modify `execute_turn` signature to accept optional `allowed_tools: Option<&[String]>`:
- If `allowed_tools` is `Some`, use `get_filtered_tool_definitions()` instead of `get_tool_definitions()`
- Tool execution also checks: if `allowed_tools` is `Some` and tool not in list, return error

**Step 4: Run tests**

Run: `cd apps/runtime/src-tauri && cargo test --test test_tool_whitelist -v 2>&1`
Expected: PASS

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/registry.rs \
       apps/runtime/src-tauri/src/agent/executor.rs \
       apps/runtime/src-tauri/src/commands/chat.rs \
       apps/runtime/src-tauri/tests/test_tool_whitelist.rs
git commit -m "feat(agent): 工具白名单执行（Skill 可限制可用工具）"
```

---

### Task 7: System Prompt 模板化

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Test: integration test via manual testing (模板化不需要独立单元测试，在 Task 6 的 `send_message` 修改中一起完成)

**Step 1: Modify send_message**

In `apps/runtime/src-tauri/src/commands/chat.rs`, after parsing `SkillConfig`, build the full system prompt:

```rust
use crate::agent::skill_config::SkillConfig;

// 在 send_message 中，替换原来的 system_prompt 处理
let skill_config = SkillConfig::parse(&system_prompt);

// 获取工具名称列表
let tool_names = if let Some(ref whitelist) = skill_config.allowed_tools {
    whitelist.join(", ")
} else {
    // 获取所有工具名
    agent_executor.registry().get_tool_definitions()
        .iter()
        .filter_map(|t| t["name"].as_str().map(String::from))
        .collect::<Vec<_>>()
        .join(", ")
};

let max_iter = skill_config.max_iterations.unwrap_or(10);

let full_system_prompt = format!(
    "{}\n\n---\n运行环境:\n- 可用工具: {}\n- 模型: {}\n- 最大迭代次数: {}",
    skill_config.system_prompt,
    tool_names,
    model_name,
    max_iter,
);
```

Also need to expose `registry()` on `AgentExecutor`:
```rust
// In executor.rs
pub fn registry(&self) -> &ToolRegistry {
    &self.registry
}
```

**Step 2: Run all tests**

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

**Step 3: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat.rs \
       apps/runtime/src-tauri/src/agent/executor.rs
git commit -m "feat(agent): System Prompt 模板化（注入运行环境信息）"
```

---

## Checkpoint 2

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

Run: `cd apps/runtime && npx tsc --noEmit 2>&1`
Expected: Clean

Phase 2 交付：Frontmatter 解析 + 工具白名单 + Prompt 模板化

---

## Phase 3: 多 Agent 协调

### Task 8: Task 工具（子 Agent 分发）

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/task_tool.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_task_tool.rs`

**Step 1: Write the failing test**

Create `apps/runtime/src-tauri/tests/test_task_tool.rs`:

```rust
use runtime_lib::agent::tools::TaskTool;
use runtime_lib::agent::{Tool, ToolRegistry, AgentExecutor};
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_task_tool_schema() {
    let registry = Arc::new(ToolRegistry::with_file_tools());
    let executor = Arc::new(AgentExecutor::new(Arc::clone(&registry)));
    let tool = TaskTool::new(
        executor,
        registry,
        "anthropic".to_string(),
        "http://mock".to_string(),
        "key".to_string(),
        "model".to_string(),
    );
    let schema = tool.input_schema();
    assert!(schema["properties"]["prompt"].is_object());
    assert!(schema["properties"]["agent_type"].is_object());
}

#[test]
fn test_task_tool_explore_tools() {
    // explore 类型应只有只读工具
    let restricted = TaskTool::get_explore_tools();
    assert!(restricted.contains(&"read_file".to_string()));
    assert!(restricted.contains(&"glob".to_string()));
    assert!(restricted.contains(&"grep".to_string()));
    assert!(!restricted.contains(&"write_file".to_string()));
    assert!(!restricted.contains(&"bash".to_string()));
    assert!(!restricted.contains(&"edit".to_string()));
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_task_tool 2>&1`
Expected: FAIL

**Step 3: Write implementation**

Create `apps/runtime/src-tauri/src/agent/tools/task_tool.rs`:

```rust
use crate::agent::types::Tool;
use crate::agent::{AgentExecutor, ToolRegistry};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct TaskTool {
    parent_executor: Arc<AgentExecutor>,
    registry: Arc<ToolRegistry>,
    api_format: String,
    base_url: String,
    api_key: String,
    model: String,
}

impl TaskTool {
    pub fn new(
        parent_executor: Arc<AgentExecutor>,
        registry: Arc<ToolRegistry>,
        api_format: String,
        base_url: String,
        api_key: String,
        model: String,
    ) -> Self {
        Self {
            parent_executor,
            registry,
            api_format,
            base_url,
            api_key,
            model,
        }
    }

    pub fn get_explore_tools() -> Vec<String> {
        vec![
            "read_file".to_string(),
            "glob".to_string(),
            "grep".to_string(),
        ]
    }

    pub fn get_plan_tools() -> Vec<String> {
        vec![
            "read_file".to_string(),
            "glob".to_string(),
            "grep".to_string(),
            "bash".to_string(),
        ]
    }
}

impl Tool for TaskTool {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "分发子 Agent 执行独立任务。子 Agent 拥有独立上下文，完成后返回结果。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "子 Agent 的任务描述"
                },
                "agent_type": {
                    "type": "string",
                    "enum": ["general-purpose", "explore", "plan"],
                    "description": "子 Agent 类型（默认 general-purpose）"
                }
            },
            "required": ["prompt"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let prompt = input["prompt"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 prompt 参数"))?;
        let agent_type = input["agent_type"].as_str().unwrap_or("general-purpose");

        // 根据类型确定工具集和迭代限制
        let (allowed_tools, max_iter) = match agent_type {
            "explore" => (Some(Self::get_explore_tools()), 5),
            "plan" => (Some(Self::get_plan_tools()), 10),
            _ => (None, 10), // general-purpose: 全部工具（除了 task 本身）
        };

        // 创建子 agent executor
        let sub_executor = AgentExecutor::with_max_iterations(
            Arc::clone(&self.registry),
            max_iter,
        );

        let system_prompt = format!(
            "你是一个专注的子 Agent (类型: {})。完成以下任务后返回结果。不要使用 task 工具创建更多子 Agent。",
            agent_type,
        );

        let messages = vec![json!({"role": "user", "content": prompt})];

        // 同步执行子 agent（使用 tokio runtime block_on）
        let rt = tokio::runtime::Handle::try_current();
        let result = match rt {
            Ok(handle) => {
                // 如果已在 tokio runtime 中，使用 spawn_blocking
                let api_format = self.api_format.clone();
                let base_url = self.base_url.clone();
                let api_key = self.api_key.clone();
                let model = self.model.clone();
                let allowed = allowed_tools.clone();

                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        sub_executor.execute_turn(
                            &api_format,
                            &base_url,
                            &api_key,
                            &model,
                            &system_prompt,
                            messages,
                            |_| {},
                            None,
                            None,
                        ).await
                    })
                }).join().map_err(|_| anyhow!("子 Agent 线程异常"))?
            }
            Err(_) => {
                // 不在 tokio runtime 中
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async {
                    sub_executor.execute_turn(
                        &self.api_format,
                        &self.base_url,
                        &self.api_key,
                        &self.model,
                        &system_prompt,
                        messages,
                        |_| {},
                        None,
                        None,
                    ).await
                })
            }
        };

        match result {
            Ok(final_messages) => {
                // 提取最后一条 assistant 消息
                let last_text = final_messages.iter().rev()
                    .find_map(|m| {
                        if m["role"].as_str() == Some("assistant") {
                            m["content"].as_str().map(String::from)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "子 Agent 未返回文本结果".to_string());

                Ok(format!("子 Agent ({}) 执行完成:\n\n{}", agent_type, last_text))
            }
            Err(e) => {
                if e.to_string().contains("最大迭代次数") {
                    Ok(format!("子 Agent ({}) 达到最大迭代次数 ({}):\n\n最后状态: 未完成",
                        agent_type, max_iter))
                } else {
                    Err(anyhow!("子 Agent 执行失败: {}", e))
                }
            }
        }
    }
}
```

Modify `apps/runtime/src-tauri/src/agent/tools/mod.rs`:
```rust
mod task_tool;
pub use task_tool::TaskTool;
```

**注意**: TaskTool 不在 `with_file_tools()` 中注册，因为它需要运行时参数（api_format, base_url 等）。它在 `send_message` 中动态创建并注册到临时 registry。

**Step 4: Run tests**

Run: `cd apps/runtime/src-tauri && cargo test --test test_task_tool -v 2>&1`
Expected: PASS (schema and tool list tests)

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/task_tool.rs \
       apps/runtime/src-tauri/src/agent/tools/mod.rs \
       apps/runtime/src-tauri/tests/test_task_tool.rs
git commit -m "feat(agent): 添加 Task 工具（子 Agent 分发）"
```

---

### Task 9: send_message 集成 Task 工具

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`

**Step 1: Modify send_message**

在 `send_message` 中，调用 `execute_turn` 之前，动态注册 TaskTool:

```rust
use crate::agent::tools::TaskTool;

// 在 send_message 中，execute_turn 之前
let task_tool = TaskTool::new(
    Arc::clone(&agent_executor),
    agent_executor.registry_arc(),
    api_format.clone(),
    base_url.clone(),
    api_key.clone(),
    model_name.clone(),
);
// 临时注册 Task 工具
agent_executor.registry().register(Arc::new(task_tool));
```

需要在 `AgentExecutor` 中添加 `registry_arc()`:
```rust
pub fn registry_arc(&self) -> Arc<ToolRegistry> {
    Arc::clone(&self.registry)
}
```

**Step 2: Run tests**

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

**Step 3: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat.rs \
       apps/runtime/src-tauri/src/agent/executor.rs
git commit -m "feat(agent): send_message 集成 Task 工具"
```

---

### Task 10: 前端子 Agent 展示

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: 修改 ChatView**

子 Agent 的 tool-call-event 中 `tool_name` 会以 `task` 开头。在 ToolCallCard 中：
- 如果 `tool_name === "task"`，显示为"子 Agent 任务"
- 输入参数展示 `prompt` 和 `agent_type`
- 输出结果使用 Markdown 渲染

```tsx
// 在 ToolCallCard 组件中增强
{toolCall.name === "task" && (
  <div className="text-xs text-blue-400 mb-1">
    子 Agent: {(toolCall.input as any)?.agent_type || "general-purpose"}
  </div>
)}
```

**Step 2: Run TypeScript check**

Run: `cd apps/runtime && npx tsc --noEmit 2>&1`
Expected: Clean

**Step 3: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx
git commit -m "feat(ui): 子 Agent 工具调用卡片增强展示"
```

---

## Checkpoint 3

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

Run: `cd apps/runtime && npx tsc --noEmit 2>&1`
Expected: Clean

Phase 3 交付：Task 工具 + send_message 集成 + 前端展示

---

## Phase 4: 高级特性

### Task 11: 权限模型

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/permissions.rs`
- Modify: `apps/runtime/src-tauri/src/agent/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Modify: `apps/runtime/src-tauri/src/db.rs` (新增 app_settings 表)
- Test: `apps/runtime/src-tauri/tests/test_permissions.rs`

**实现概要:**

```rust
pub enum PermissionMode {
    Default,       // Write/Edit/Bash 需确认
    AcceptEdits,   // Write/Edit 自动通过，Bash 仍需确认
    Unrestricted,  // 全部自动通过
}

impl PermissionMode {
    pub fn needs_confirmation(&self, tool_name: &str) -> bool {
        match self {
            Self::Unrestricted => false,
            Self::AcceptEdits => matches!(tool_name, "bash"),
            Self::Default => matches!(tool_name, "write_file" | "edit" | "bash"),
        }
    }
}
```

- executor 中执行工具前检查 `needs_confirmation()`
- 需要确认时发送 `permission-request` 事件到前端
- 用 `tokio::sync::oneshot` 等待前端响应
- DB 新增 `app_settings` 表存储权限模式
- SettingsView 增加权限模式选择

**测试**: 单元测试 `PermissionMode::needs_confirmation()` 的各种组合

**Commit**: `feat(agent): 添加权限模型（Default/AcceptEdits/Unrestricted）`

---

### Task 12: WebFetch 工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/web_fetch.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_web_fetch.rs`

**实现概要:**

```rust
pub struct WebFetchTool;

impl Tool for WebFetchTool {
    fn name(&self) -> &str { "web_fetch" }

    fn execute(&self, input: Value) -> Result<String> {
        let url = input["url"].as_str().ok_or(anyhow!("缺少 url"))?;
        let client = reqwest::blocking::Client::new();
        let resp = client.get(url)
            .header("User-Agent", "WorkClaw/1.0")
            .send()?;
        let body = resp.text()?;
        // 去除 script/style 标签
        let cleaned = strip_html_tags(&body);
        // 截断
        let result = truncate_tool_output(&cleaned, 30_000);
        Ok(result)
    }
}

fn strip_html_tags(html: &str) -> String {
    // 简单正则：移除 <script>...</script> 和 <style>...</style>
    // 然后移除所有 HTML 标签
    let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let no_script = re_script.replace_all(html, "");
    let no_style = re_style.replace_all(&no_script, "");
    let text = re_tags.replace_all(&no_style, "");
    // 压缩连续空行
    let re_lines = regex::Regex::new(r"\n{3,}").unwrap();
    re_lines.replace_all(&text, "\n\n").trim().to_string()
}
```

**测试**: 使用本地文件模拟 HTML 内容测试 `strip_html_tags()`

**Commit**: `feat(agent): 添加 WebFetch 工具（URL 内容获取）`

---

### Task 13: WebSearch 工具（Sidecar 代理）

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/web_search.rs`
- Modify: `apps/runtime/sidecar/src/index.ts` (新增 `/api/web/search` 端点)
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Test: `apps/runtime/src-tauri/tests/test_web_search.rs`

**实现概要:**

Rust 端：通过 SidecarBridgeTool 包装，调用 Sidecar 的搜索端点。

Sidecar 端：使用 DuckDuckGo HTML 搜索（不需要 API Key）：
```typescript
app.post('/api/web/search', async (c) => {
    const { query, count = 5 } = await c.req.json();
    // 使用 fetch 调用 DuckDuckGo HTML
    const url = `https://html.duckduckgo.com/html/?q=${encodeURIComponent(query)}`;
    const resp = await fetch(url, { headers: { 'User-Agent': 'WorkClaw/1.0' }});
    const html = await resp.text();
    // 解析结果...
    return c.json({ output: results });
});
```

**Commit**: `feat(agent): 添加 WebSearch 工具（DuckDuckGo 搜索）`

---

### Task 14: 持久内存工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/memory_tool.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Test: `apps/runtime/src-tauri/tests/test_memory_tool.rs`

**实现概要:**

```rust
pub struct MemoryTool {
    memory_dir: PathBuf,  // {app_data_dir}/memory/{skill_id}/
}

impl Tool for MemoryTool {
    fn name(&self) -> &str { "memory" }

    fn execute(&self, input: Value) -> Result<String> {
        let action = input["action"].as_str().ok_or(anyhow!("缺少 action"))?;
        match action {
            "read" => { /* 读取 {key}.md */ }
            "write" => { /* 写入 {key}.md */ }
            "list" => { /* 列出所有 .md 文件 */ }
            "delete" => { /* 删除 {key}.md */ }
            _ => Err(anyhow!("未知操作"))
        }
    }
}
```

在 `send_message` 中：如果 `memory/MEMORY.md` 存在，注入到 system prompt。

**Commit**: `feat(agent): 添加持久内存工具（跨会话知识存储）`

---

### Task 15: AskUser 工具

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/ask_user.rs`
- Modify: `apps/runtime/src-tauri/src/agent/types.rs`
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Test: `apps/runtime/src-tauri/tests/test_ask_user.rs`

**实现概要:**

1. 扩展 `LLMResponse` 枚举：
```rust
pub enum LLMResponse {
    Text(String),
    ToolCalls(Vec<ToolCall>),
}
```

2. 在 Tool trait 添加 `is_interactive()`:
```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: Value) -> Result<String>;
    fn is_interactive(&self) -> bool { false }
}
```

3. AskUser 执行时发送事件到前端，等待 `oneshot::channel` 响应。

4. 新增 `resume_message` Tauri command 从数据库恢复上下文并继续。

5. 前端展示问题卡片带选项按钮。

**这是最复杂的 Task，可能需要拆分为多个子步骤。**

**Commit**: `feat(agent): 添加 AskUser 工具（交互式用户问答）`

---

### Task 16: SettingsView 权限配置 UI

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`

**实现概要:**
- 添加权限模式选择区域（Default / AcceptEdits / Unrestricted）
- 调用新的 Tauri command `save_setting` / `get_setting`

**Commit**: `feat(ui): 设置页面添加权限模式配置`

---

### Task 17: AskUser 前端 UI

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`

**实现概要:**
- 监听 `ask-user-event` 事件
- 展示问题卡片（含预设选项按钮 + 自由输入框）
- 用户响应后调用 `resume_message`

**Commit**: `feat(ui): AskUser 工具前端问答界面`

---

## Final Checkpoint

Run: `cd apps/runtime/src-tauri && cargo test 2>&1`
Expected: All PASS

Run: `cd apps/runtime && npx tsc --noEmit 2>&1`
Expected: Clean

Run: `pnpm runtime` → 手动测试:
- Edit 工具精确替换
- TodoWrite 创建/更新/列出任务
- Skill frontmatter 解析和工具白名单
- Task 工具子 Agent 分发
- 权限确认对话框
- WebFetch 获取网页
- 持久内存读写
- AskUser 交互式问答

---

## 实施建议

1. **Phase 1 (Tasks 1-4)** 最为独立，可立即开始
2. **Phase 2 (Tasks 5-7)** 依赖 Phase 1 的 Edit/TodoWrite
3. **Phase 3 (Tasks 8-10)** 依赖 Phase 2 的 SkillConfig
4. **Phase 4 (Tasks 11-17)** 各 Task 之间较独立，可并行

推荐执行顺序严格按照 Task 编号。每个 Phase 完成后进行完整测试。
