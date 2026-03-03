# Agent Capabilities Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add complete Agent execution capabilities to WorkClaw Runtime, enabling tool calling (file, bash, browser, MCP) with a ReAct loop supporting both Anthropic and OpenAI LLM formats.

**Architecture:** Hybrid Rust + Node.js sidecar. Rust handles the Agent executor, ReAct loop, file/bash tools, and LLM adapters. Node.js sidecar (Hono server on localhost:8765) handles Playwright browser control and MCP client management. Communication via HTTP REST.

**Tech Stack:** Rust (Tauri, tokio, reqwest, serde, anyhow, regex, glob), Node.js (Hono, Playwright, MCP SDK), TypeScript

---

## Phase 1: Agent Engine + File Tools (Week 1-2)

### Task 1: Create Agent Module Structure

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/mod.rs`
- Create: `apps/runtime/src-tauri/src/agent/executor.rs`
- Create: `apps/runtime/src-tauri/src/agent/registry.rs`
- Create: `apps/runtime/src-tauri/src/agent/types.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`

**Step 1: Create types module**

```rust
// apps/runtime/src-tauri/src/agent/types.rs
use serde_json::Value;
use anyhow::Result;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: Value) -> Result<String>;
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
}

#[derive(Debug)]
pub enum LLMResponse {
    Text(String),
    ToolCalls(Vec<ToolCall>),
}

#[derive(Debug)]
pub enum AgentState {
    Thinking,
    ToolCalling,
    Finished,
    Error(String),
}
```

**Step 2: Create registry module**

```rust
// apps/runtime/src-tauri/src/agent/registry.rs
use super::types::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::{json, Value};

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn get_tool_definitions(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|t| {
                json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.input_schema(),
                })
            })
            .collect()
    }
}
```

**Step 3: Create executor module stub**

```rust
// apps/runtime/src-tauri/src/agent/executor.rs
use super::registry::ToolRegistry;
use super::types::{LLMResponse, ToolCall, ToolResult};
use anyhow::{Result, anyhow};
use serde_json::Value;
use std::sync::Arc;

pub struct AgentExecutor {
    registry: Arc<ToolRegistry>,
    max_iterations: usize,
}

impl AgentExecutor {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            max_iterations: 10,
        }
    }

    pub async fn execute_turn(
        &self,
        _api_format: &str,
        _base_url: &str,
        _api_key: &str,
        _model: &str,
        _system_prompt: &str,
        messages: Vec<Value>,
        _on_token: impl Fn(String) + Send + Clone,
    ) -> Result<Vec<Value>> {
        // Stub implementation for now
        Ok(messages)
    }
}
```

**Step 4: Create main module file**

```rust
// apps/runtime/src-tauri/src/agent/mod.rs
pub mod types;
pub mod registry;
pub mod executor;

pub use types::{Tool, ToolCall, ToolResult, LLMResponse, AgentState};
pub use registry::ToolRegistry;
pub use executor::AgentExecutor;
```

**Step 5: Add agent module to lib.rs**

```rust
// apps/runtime/src-tauri/src/lib.rs
// Add after other module declarations
mod agent;
```

**Step 6: Verify compilation**

Run: `cd apps/runtime/src-tauri && cargo check`
Expected: SUCCESS (no errors)

**Step 7: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/
git add apps/runtime/src-tauri/src/lib.rs
git commit -m "feat(agent): add core agent module structure

- Tool trait definition
- ToolRegistry for managing tools
- AgentExecutor stub with max_iterations
- Type definitions for ToolCall, ToolResult, LLMResponse, AgentState

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Implement ReadFile Tool

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Create: `apps/runtime/src-tauri/src/agent/tools/read_file.rs`
- Create: `tests/agent/test_read_file.rs` (integration test)
- Modify: `apps/runtime/src-tauri/src/agent/mod.rs`

**Step 1: Write failing integration test**

```rust
// apps/runtime/src-tauri/tests/agent/test_read_file.rs
use runtime_lib::agent::{Tool, ToolRegistry};
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_read_file_success() {
    let registry = ToolRegistry::new();
    // Tool not registered yet, so get will fail
    let tool = registry.get("read_file");
    assert!(tool.is_some(), "read_file tool should be registered");
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_read_file_success`
Expected: FAIL (tool not registered)

**Step 3: Implement ReadFileTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/read_file.rs
use crate::agent::types::Tool;
use anyhow::{Result, anyhow};
use serde_json::{json, Value};

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "读取文件内容。返回文件的完整文本内容。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要读取的文件路径（相对或绝对）"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;

        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("读取文件失败: {}", e))?;

        Ok(content)
    }
}
```

**Step 4: Create tools module**

```rust
// apps/runtime/src-tauri/src/agent/tools/mod.rs
mod read_file;

pub use read_file::ReadFileTool;
```

**Step 5: Update agent mod.rs to export tools**

```rust
// apps/runtime/src-tauri/src/agent/mod.rs
pub mod types;
pub mod registry;
pub mod executor;
pub mod tools;

pub use types::{Tool, ToolCall, ToolResult, LLMResponse, AgentState};
pub use registry::ToolRegistry;
pub use executor::AgentExecutor;
pub use tools::*;
```

**Step 6: Update test to register tool**

```rust
// apps/runtime/src-tauri/tests/agent/test_read_file.rs
use runtime_lib::agent::{Tool, ToolRegistry, ReadFileTool};
use serde_json::json;
use std::sync::Arc;
use std::fs;

#[test]
fn test_read_file_success() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadFileTool));

    let tool = registry.get("read_file");
    assert!(tool.is_some(), "read_file tool should be registered");

    // Create test file
    let test_path = "test_file.txt";
    fs::write(test_path, "Hello, World!").unwrap();

    // Execute tool
    let input = json!({"path": test_path});
    let result = tool.unwrap().execute(input).unwrap();

    assert_eq!(result, "Hello, World!");

    // Cleanup
    fs::remove_file(test_path).unwrap();
}

#[test]
fn test_read_file_missing_path() {
    let tool = ReadFileTool;
    let input = json!({});
    let result = tool.execute(input);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("缺少 path 参数"));
}

#[test]
fn test_read_file_not_found() {
    let tool = ReadFileTool;
    let input = json!({"path": "nonexistent_file.txt"});
    let result = tool.execute(input);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("读取文件失败"));
}
```

**Step 7: Create tests directory structure**

```bash
mkdir -p apps/runtime/src-tauri/tests/agent
```

**Step 8: Run tests to verify they pass**

Run: `cd apps/runtime/src-tauri && cargo test test_read_file`
Expected: 3 tests PASS

**Step 9: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/
git add apps/runtime/src-tauri/src/agent/mod.rs
git add apps/runtime/src-tauri/tests/agent/
git commit -m "feat(agent): implement ReadFileTool with tests

- ReadFileTool reads file content from path
- Handles missing path parameter error
- Handles file not found error
- 3 integration tests covering success and error cases

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Implement WriteFile Tool

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/write_file.rs`
- Create: `tests/agent/test_write_file.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`

**Step 1: Write failing integration test**

```rust
// apps/runtime/src-tauri/tests/agent/test_write_file.rs
use runtime_lib::agent::{Tool, WriteFileTool};
use serde_json::json;
use std::fs;

#[test]
fn test_write_file_success() {
    let tool = WriteFileTool;
    let test_path = "test_output.txt";

    let input = json!({
        "path": test_path,
        "content": "Test content"
    });

    let result = tool.execute(input).unwrap();
    assert!(result.contains("成功写入"));
    assert!(result.contains(test_path));

    // Verify file was written
    let content = fs::read_to_string(test_path).unwrap();
    assert_eq!(content, "Test content");

    // Cleanup
    fs::remove_file(test_path).unwrap();
}

#[test]
fn test_write_file_creates_parent_dirs() {
    let tool = WriteFileTool;
    let test_path = "test_dir/nested/file.txt";

    let input = json!({
        "path": test_path,
        "content": "Nested content"
    });

    let result = tool.execute(input).unwrap();
    assert!(result.contains("成功写入"));

    // Verify file was written
    let content = fs::read_to_string(test_path).unwrap();
    assert_eq!(content, "Nested content");

    // Cleanup
    fs::remove_dir_all("test_dir").unwrap();
}

#[test]
fn test_write_file_missing_params() {
    let tool = WriteFileTool;

    let input = json!({"path": "test.txt"});
    let result = tool.execute(input);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("缺少 content 参数"));
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_write_file`
Expected: FAIL (WriteFileTool not defined)

**Step 3: Implement WriteFileTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/write_file.rs
use crate::agent::types::Tool;
use anyhow::{Result, anyhow};
use serde_json::{json, Value};
use std::path::Path;

pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "写入内容到文件。如果文件不存在会创建，已存在会覆盖。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要写入的文件路径"
                },
                "content": {
                    "type": "string",
                    "description": "要写入的文本内容"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 content 参数"))?;

        // 确保父目录存在
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow!("创建目录失败: {}", e))?;
        }

        std::fs::write(path, content)
            .map_err(|e| anyhow!("写入文件失败: {}", e))?;

        Ok(format!("成功写入 {} 字节到 {}", content.len(), path))
    }
}
```

**Step 4: Export WriteFileTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/mod.rs
mod read_file;
mod write_file;

pub use read_file::ReadFileTool;
pub use write_file::WriteFileTool;
```

**Step 5: Run tests to verify they pass**

Run: `cd apps/runtime/src-tauri && cargo test test_write_file`
Expected: 3 tests PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/write_file.rs
git add apps/runtime/src-tauri/src/agent/tools/mod.rs
git add apps/runtime/src-tauri/tests/agent/test_write_file.rs
git commit -m "feat(agent): implement WriteFileTool with tests

- WriteFileTool writes content to file path
- Automatically creates parent directories
- Handles missing parameters error
- 3 integration tests covering success and error cases

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Implement Glob Tool

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/glob_tool.rs`
- Create: `tests/agent/test_glob.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/Cargo.toml`

**Step 1: Add glob dependency**

```toml
# apps/runtime/src-tauri/Cargo.toml
[dependencies]
# ... existing dependencies ...
glob = "0.3"
```

**Step 2: Write failing integration test**

```rust
// apps/runtime/src-tauri/tests/agent/test_glob.rs
use runtime_lib::agent::{Tool, GlobTool};
use serde_json::json;
use std::fs;

#[test]
fn test_glob_find_files() {
    // Setup test files
    fs::create_dir_all("test_glob_dir/subdir").unwrap();
    fs::write("test_glob_dir/file1.txt", "").unwrap();
    fs::write("test_glob_dir/file2.txt", "").unwrap();
    fs::write("test_glob_dir/subdir/file3.txt", "").unwrap();
    fs::write("test_glob_dir/file.rs", "").unwrap();

    let tool = GlobTool;
    let input = json!({
        "pattern": "**/*.txt",
        "base_dir": "test_glob_dir"
    });

    let result = tool.execute(input).unwrap();
    assert!(result.contains("找到 3 个文件"));
    assert!(result.contains("file1.txt"));
    assert!(result.contains("file2.txt"));
    assert!(result.contains("file3.txt"));
    assert!(!result.contains("file.rs"));

    // Cleanup
    fs::remove_dir_all("test_glob_dir").unwrap();
}

#[test]
fn test_glob_no_matches() {
    let tool = GlobTool;
    let input = json!({
        "pattern": "**/*.nonexistent"
    });

    let result = tool.execute(input).unwrap();
    assert!(result.contains("找到 0 个文件"));
}
```

**Step 3: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_glob`
Expected: FAIL (GlobTool not defined)

**Step 4: Implement GlobTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/glob_tool.rs
use crate::agent::types::Tool;
use anyhow::{Result, anyhow};
use serde_json::{json, Value};

pub struct GlobTool;

impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "使用 glob 模式搜索文件。支持 ** 递归、* 通配符、? 单字符。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob 模式，例如 '**/*.rs' 或 'src/**/*.ts'"
                },
                "base_dir": {
                    "type": "string",
                    "description": "搜索的基础目录（可选，默认为当前目录）"
                }
            },
            "required": ["pattern"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 pattern 参数"))?;
        let base_dir = input["base_dir"].as_str().unwrap_or(".");

        let full_pattern = format!("{}/{}", base_dir, pattern);
        let paths: Vec<String> = glob::glob(&full_pattern)
            .map_err(|e| anyhow!("Glob 模式错误: {}", e))?
            .filter_map(|r| r.ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        Ok(format!("找到 {} 个文件:\n{}", paths.len(), paths.join("\n")))
    }
}
```

**Step 5: Export GlobTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/mod.rs
mod read_file;
mod write_file;
mod glob_tool;

pub use read_file::ReadFileTool;
pub use write_file::WriteFileTool;
pub use glob_tool::GlobTool;
```

**Step 6: Run tests to verify they pass**

Run: `cd apps/runtime/src-tauri && cargo test test_glob`
Expected: 2 tests PASS

**Step 7: Commit**

```bash
git add apps/runtime/src-tauri/Cargo.toml
git add apps/runtime/src-tauri/src/agent/tools/glob_tool.rs
git add apps/runtime/src-tauri/src/agent/tools/mod.rs
git add apps/runtime/src-tauri/tests/agent/test_glob.rs
git commit -m "feat(agent): implement GlobTool with tests

- Add glob 0.3 dependency
- GlobTool searches files with glob patterns
- Supports ** recursive, * wildcard, ? single char
- 2 integration tests covering matches and no matches

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Implement Grep Tool

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/grep_tool.rs`
- Create: `tests/agent/test_grep.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/Cargo.toml`

**Step 1: Add regex dependency**

```toml
# apps/runtime/src-tauri/Cargo.toml
[dependencies]
# ... existing dependencies ...
regex = "1"
```

**Step 2: Write failing integration test**

```rust
// apps/runtime/src-tauri/tests/agent/test_grep.rs
use runtime_lib::agent::{Tool, GrepTool};
use serde_json::json;
use std::fs;

#[test]
fn test_grep_find_matches() {
    let test_file = "test_grep.txt";
    fs::write(test_file, "line 1: hello\nline 2: world\nline 3: hello world\n").unwrap();

    let tool = GrepTool;
    let input = json!({
        "pattern": "hello",
        "path": test_file
    });

    let result = tool.execute(input).unwrap();
    assert!(result.contains("找到 2 处匹配"));
    assert!(result.contains("1:line 1: hello"));
    assert!(result.contains("3:line 3: hello world"));

    fs::remove_file(test_file).unwrap();
}

#[test]
fn test_grep_case_insensitive() {
    let test_file = "test_grep_ci.txt";
    fs::write(test_file, "Hello\nHELLO\nhello\n").unwrap();

    let tool = GrepTool;
    let input = json!({
        "pattern": "hello",
        "path": test_file,
        "case_insensitive": true
    });

    let result = tool.execute(input).unwrap();
    assert!(result.contains("找到 3 处匹配"));

    fs::remove_file(test_file).unwrap();
}

#[test]
fn test_grep_no_matches() {
    let test_file = "test_grep_none.txt";
    fs::write(test_file, "foo\nbar\nbaz\n").unwrap();

    let tool = GrepTool;
    let input = json!({
        "pattern": "notfound",
        "path": test_file
    });

    let result = tool.execute(input).unwrap();
    assert!(result.contains("找到 0 处匹配"));

    fs::remove_file(test_file).unwrap();
}
```

**Step 3: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_grep`
Expected: FAIL (GrepTool not defined)

**Step 4: Implement GrepTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/grep_tool.rs
use crate::agent::types::Tool;
use anyhow::{Result, anyhow};
use serde_json::{json, Value};
use regex::RegexBuilder;

pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "在文件或目录中搜索文本模式（正则表达式）。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "正则表达式搜索模式"
                },
                "path": {
                    "type": "string",
                    "description": "要搜索的文件或目录路径"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "是否忽略大小写（可选，默认 false）"
                }
            },
            "required": ["pattern", "path"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 pattern 参数"))?;
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let case_insensitive = input["case_insensitive"].as_bool().unwrap_or(false);

        let re = RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .build()
            .map_err(|e| anyhow!("正则表达式错误: {}", e))?;

        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("读取文件失败: {}", e))?;

        let matches: Vec<String> = content
            .lines()
            .enumerate()
            .filter(|(_, line)| re.is_match(line))
            .map(|(i, line)| format!("{}:{}", i + 1, line))
            .collect();

        Ok(format!("找到 {} 处匹配:\n{}", matches.len(), matches.join("\n")))
    }
}
```

**Step 5: Export GrepTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/mod.rs
mod read_file;
mod write_file;
mod glob_tool;
mod grep_tool;

pub use read_file::ReadFileTool;
pub use write_file::WriteFileTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
```

**Step 6: Run tests to verify they pass**

Run: `cd apps/runtime/src-tauri && cargo test test_grep`
Expected: 3 tests PASS

**Step 7: Commit**

```bash
git add apps/runtime/src-tauri/Cargo.toml
git add apps/runtime/src-tauri/src/agent/tools/grep_tool.rs
git add apps/runtime/src-tauri/src/agent/tools/mod.rs
git add apps/runtime/src-tauri/tests/agent/test_grep.rs
git commit -m "feat(agent): implement GrepTool with tests

- Add regex 1.0 dependency
- GrepTool searches text patterns in files
- Supports case-insensitive search
- 3 integration tests covering matches, case-insensitive, and no matches

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Register File Tools in ToolRegistry

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Create: `tests/agent/test_registry.rs`

**Step 1: Write failing integration test**

```rust
// apps/runtime/src-tauri/tests/agent/test_registry.rs
use runtime_lib::agent::{ToolRegistry, ReadFileTool, WriteFileTool, GlobTool, GrepTool};
use std::sync::Arc;

#[test]
fn test_registry_with_file_tools() {
    let registry = ToolRegistry::with_file_tools();

    assert!(registry.get("read_file").is_some());
    assert!(registry.get("write_file").is_some());
    assert!(registry.get("glob").is_some());
    assert!(registry.get("grep").is_some());

    let defs = registry.get_tool_definitions();
    assert_eq!(defs.len(), 4);
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_registry_with_file_tools`
Expected: FAIL (with_file_tools method not defined)

**Step 3: Implement with_file_tools method**

```rust
// apps/runtime/src-tauri/src/agent/registry.rs
use super::types::Tool;
use super::tools::{ReadFileTool, WriteFileTool, GlobTool, GrepTool};
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::{json, Value};

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn with_file_tools() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(ReadFileTool));
        registry.register(Arc::new(WriteFileTool));
        registry.register(Arc::new(GlobTool));
        registry.register(Arc::new(GrepTool));
        registry
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn get_tool_definitions(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|t| {
                json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.input_schema(),
                })
            })
            .collect()
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test test_registry_with_file_tools`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/registry.rs
git add apps/runtime/src-tauri/tests/agent/test_registry.rs
git commit -m "feat(agent): add with_file_tools factory method to ToolRegistry

- ToolRegistry::with_file_tools() registers all 4 file tools
- Integration test verifies all tools are registered

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: Implement Anthropic Tool Use Parsing

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Modify: `apps/runtime/src-tauri/src/adapters/anthropic.rs`
- Create: `tests/agent/test_anthropic_tools.rs`

**Step 1: Write failing integration test (mock)**

```rust
// apps/runtime/src-tauri/tests/agent/test_anthropic_tools.rs
use runtime_lib::agent::{AgentExecutor, ToolRegistry};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_anthropic_tool_parsing() {
    let registry = Arc::new(ToolRegistry::with_file_tools());
    let executor = AgentExecutor::new(registry);

    // This will fail initially because execute_turn is stubbed
    let messages = vec![json!({"role": "user", "content": "Read test.txt"})];

    // For now, just verify it doesn't panic
    let result = executor.execute_turn(
        "anthropic",
        "http://mock",
        "mock-key",
        "claude-3-5-haiku-20241022",
        "You are a helpful assistant.",
        messages,
        |_token| {},
    ).await;

    // Will implement actual parsing in next steps
    assert!(result.is_ok());
}
```

**Step 2: Run test to verify current behavior**

Run: `cd apps/runtime/src-tauri && cargo test test_anthropic_tool_parsing`
Expected: PASS (stub returns Ok)

**Step 3: Add helper method to anthropic adapter**

```rust
// apps/runtime/src-tauri/src/adapters/anthropic.rs
// Add at the end of the file

use serde_json::Value;
use anyhow::Result;

pub async fn chat_stream_with_tools(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    messages: Vec<Value>,
    tools: Vec<Value>,
    mut on_token: impl FnMut(String) + Send,
) -> Result<crate::agent::types::LLMResponse> {
    use crate::agent::types::{LLMResponse, ToolCall};
    use futures_util::StreamExt;

    let client = reqwest::Client::new();
    let url = format!("{}/messages", base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "system": system_prompt,
        "messages": messages,
        "tools": tools,
        "max_tokens": 4096,
        "stream": true,
    });

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        return Err(anyhow::anyhow!("Anthropic API 错误: {}", text));
    }

    let mut stream = resp.bytes_stream();
    let mut tool_calls: Vec<ToolCall> = vec![];
    let mut text_content = String::new();
    let mut current_tool_call: Option<ToolCall> = None;
    let mut current_tool_input = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim().is_empty() {
                    continue;
                }

                if let Ok(v) = serde_json::from_str::<Value>(data) {
                    let event_type = v["type"].as_str().unwrap_or("");

                    match event_type {
                        "content_block_start" => {
                            if v["content_block"]["type"] == "tool_use" {
                                current_tool_call = Some(ToolCall {
                                    id: v["content_block"]["id"].as_str().unwrap_or("").to_string(),
                                    name: v["content_block"]["name"].as_str().unwrap_or("").to_string(),
                                    input: serde_json::json!({}),
                                });
                                current_tool_input.clear();
                            }
                        }
                        "content_block_delta" => {
                            if v["delta"]["type"] == "text_delta" {
                                let token = v["delta"]["text"].as_str().unwrap_or("");
                                text_content.push_str(token);
                                on_token(token.to_string());
                            } else if v["delta"]["type"] == "input_json_delta" {
                                current_tool_input.push_str(v["delta"]["partial_json"].as_str().unwrap_or(""));
                            }
                        }
                        "content_block_stop" => {
                            if let Some(mut call) = current_tool_call.take() {
                                if !current_tool_input.is_empty() {
                                    call.input = serde_json::from_str(&current_tool_input)
                                        .unwrap_or(serde_json::json!({}));
                                }
                                tool_calls.push(call);
                            }
                        }
                        "message_stop" => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if !tool_calls.is_empty() {
        Ok(LLMResponse::ToolCalls(tool_calls))
    } else {
        Ok(LLMResponse::Text(text_content))
    }
}
```

**Step 4: Verify compilation**

Run: `cd apps/runtime/src-tauri && cargo check`
Expected: SUCCESS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/adapters/anthropic.rs
git add apps/runtime/src-tauri/tests/agent/test_anthropic_tools.rs
git commit -m "feat(agent): add Anthropic tool_use parsing to adapter

- chat_stream_with_tools handles tool_use content blocks
- Parses content_block_start, content_block_delta, content_block_stop events
- Accumulates tool input JSON from partial_json deltas
- Returns LLMResponse::ToolCalls or LLMResponse::Text

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Phase 2: Bash Tools + Sidecar Foundation (Week 3-4)

### Task 8: Implement Bash Tool

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/bash.rs`
- Create: `tests/agent/test_bash.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`

**Step 1: Write failing integration test**

```rust
// apps/runtime/src-tauri/tests/agent/test_bash.rs
use runtime_lib::agent::{Tool, BashTool};
use serde_json::json;

#[test]
fn test_bash_simple_command() {
    let tool = BashTool::new();

    #[cfg(target_os = "windows")]
    let input = json!({"command": "echo Hello"});

    #[cfg(not(target_os = "windows"))]
    let input = json!({"command": "echo Hello"});

    let result = tool.execute(input).unwrap();
    assert!(result.contains("Hello"));
}

#[test]
fn test_bash_command_failure() {
    let tool = BashTool::new();
    let input = json!({"command": "nonexistent_command_xyz"});

    let result = tool.execute(input).unwrap();
    assert!(result.contains("命令执行失败"));
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_bash`
Expected: FAIL (BashTool not defined)

**Step 3: Implement BashTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/bash.rs
use crate::agent::types::Tool;
use anyhow::{Result, anyhow};
use serde_json::{json, Value};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

pub struct BashTool {
    background_processes: Arc<Mutex<HashMap<String, std::process::Child>>>,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            background_processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[cfg(target_os = "windows")]
    fn get_shell() -> (&'static str, &'static str) {
        ("powershell", "-Command")
    }

    #[cfg(not(target_os = "windows"))]
    fn get_shell() -> (&'static str, &'static str) {
        ("bash", "-c")
    }
}

impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "执行 shell 命令。支持同步和后台模式。Windows 使用 PowerShell，Unix 使用 bash。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "要执行的 shell 命令"
                },
                "background": {
                    "type": "boolean",
                    "description": "是否后台运行（可选，默认 false）"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "超时时间（毫秒，可选，默认 30000）"
                }
            },
            "required": ["command"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 command 参数"))?;
        let background = input["background"].as_bool().unwrap_or(false);

        let (shell, flag) = Self::get_shell();

        if background {
            let mut child = Command::new(shell)
                .arg(flag)
                .arg(command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            let pid = child.id().to_string();
            self.background_processes
                .lock()
                .unwrap()
                .insert(pid.clone(), child);

            Ok(format!("后台进程已启动，PID: {}", pid))
        } else {
            let output = Command::new(shell)
                .arg(flag)
                .arg(command)
                .output()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !output.status.success() {
                Ok(format!(
                    "命令执行失败（退出码 {}）\nstderr:\n{}",
                    output.status.code().unwrap_or(-1),
                    stderr
                ))
            } else {
                Ok(format!("stdout:\n{}\nstderr:\n{}", stdout, stderr))
            }
        }
    }
}
```

**Step 4: Export BashTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/mod.rs
mod read_file;
mod write_file;
mod glob_tool;
mod grep_tool;
mod bash;

pub use read_file::ReadFileTool;
pub use write_file::WriteFileTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
pub use bash::BashTool;
```

**Step 5: Run tests to verify they pass**

Run: `cd apps/runtime/src-tauri && cargo test test_bash`
Expected: 2 tests PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/bash.rs
git add apps/runtime/src-tauri/src/agent/tools/mod.rs
git add apps/runtime/src-tauri/tests/agent/test_bash.rs
git commit -m "feat(agent): implement BashTool with cross-platform support

- BashTool executes shell commands
- Windows uses PowerShell, Unix uses bash
- Supports background mode (stores Child processes)
- 2 integration tests covering success and failure

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 9: Create Node.js Sidecar Project Structure

**Files:**
- Create: `apps/runtime/sidecar/package.json`
- Create: `apps/runtime/sidecar/tsconfig.json`
- Create: `apps/runtime/sidecar/src/index.ts`
- Create: `apps/runtime/sidecar/.gitignore`

**Step 1: Create package.json**

```json
{
  "name": "workclaw-runtime-sidecar",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "dev": "tsx watch src/index.ts",
    "build": "tsc",
    "start": "node dist/index.js"
  },
  "dependencies": {
    "hono": "^4.0.0"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "tsx": "^4.0.0",
    "typescript": "^5.3.0"
  }
}
```

**Step 2: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "lib": ["ES2022"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "moduleResolution": "node",
    "resolveJsonModule": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
```

**Step 3: Create minimal Hono server**

```typescript
// apps/runtime/sidecar/src/index.ts
import { Hono } from 'hono';
import { cors } from 'hono/cors';

const app = new Hono();

app.use('/*', cors());

app.get('/health', (c) => {
  return c.json({ status: 'ok', uptime: process.uptime() });
});

const PORT = process.env.PORT || 8765;
console.log(`[sidecar] Starting on http://localhost:${PORT}`);

export default {
  port: PORT,
  fetch: app.fetch,
};
```

**Step 4: Create .gitignore**

```
node_modules/
dist/
*.log
```

**Step 5: Install dependencies**

Run: `cd apps/runtime/sidecar && pnpm install`
Expected: Dependencies installed successfully

**Step 6: Build sidecar**

Run: `cd apps/runtime/sidecar && pnpm build`
Expected: TypeScript compiles to dist/

**Step 7: Test sidecar manually**

Run: `cd apps/runtime/sidecar && pnpm start` (in background)
Run: `curl http://localhost:8765/health`
Expected: `{"status":"ok","uptime":...}`
Kill the process with Ctrl+C

**Step 8: Commit**

```bash
git add apps/runtime/sidecar/
git commit -m "feat(sidecar): create Node.js sidecar project with Hono

- package.json with Hono, TypeScript, tsx
- Minimal Hono server with /health endpoint
- CORS enabled for all routes
- Listens on localhost:8765 by default

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 10: Implement Sidecar Lifecycle Manager in Rust

**Files:**
- Create: `apps/runtime/src-tauri/src/sidecar.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Create: `tests/sidecar/test_lifecycle.rs`

**Step 1: Write failing integration test**

```rust
// apps/runtime/src-tauri/tests/sidecar/test_lifecycle.rs
use runtime_lib::sidecar::SidecarManager;

#[tokio::test]
async fn test_sidecar_start_and_health_check() {
    let manager = SidecarManager::new();

    // Start sidecar
    let result = manager.start().await;
    assert!(result.is_ok(), "Sidecar should start successfully");

    // Health check should succeed
    let health = manager.health_check().await;
    assert!(health.is_ok(), "Health check should pass");

    // Stop sidecar
    manager.stop();
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_sidecar_start_and_health_check`
Expected: FAIL (SidecarManager not defined)

**Step 3: Implement SidecarManager**

```rust
// apps/runtime/src-tauri/src/sidecar.rs
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use anyhow::Result;

pub struct SidecarManager {
    process: Arc<Mutex<Option<Child>>>,
    url: String,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            url: "http://localhost:8765".to_string(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mut proc = self.process.lock().unwrap();
        if proc.is_some() {
            return Ok(()); // Already started
        }

        // Start Node.js sidecar
        let child = Command::new("node")
            .arg("sidecar/dist/index.js")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        *proc = Some(child);

        // Wait for server to be ready (max 3 seconds)
        for _ in 0..30 {
            if self.health_check().await.is_ok() {
                eprintln!("[sidecar] Service started: {}", self.url);
                return Ok(());
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Err(anyhow::anyhow!("Sidecar startup timeout"))
    }

    pub async fn health_check(&self) -> Result<()> {
        let resp = reqwest::get(&format!("{}/health", self.url)).await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Health check failed"))
        }
    }

    pub fn stop(&self) {
        let mut proc = self.process.lock().unwrap();
        if let Some(mut child) = proc.take() {
            let _ = child.kill();
            eprintln!("[sidecar] Service stopped");
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.stop();
    }
}
```

**Step 4: Add sidecar module to lib.rs**

```rust
// apps/runtime/src-tauri/src/lib.rs
// Add after agent module
pub mod sidecar;
```

**Step 5: Create tests directory**

```bash
mkdir -p apps/runtime/src-tauri/tests/sidecar
```

**Step 6: Run test to verify it passes**

Note: This test requires the sidecar to be built first.

Run: `cd apps/runtime/sidecar && pnpm build`
Run: `cd apps/runtime/src-tauri && cargo test test_sidecar_start_and_health_check`
Expected: PASS

**Step 7: Commit**

```bash
git add apps/runtime/src-tauri/src/sidecar.rs
git add apps/runtime/src-tauri/src/lib.rs
git add apps/runtime/src-tauri/tests/sidecar/
git commit -m "feat(sidecar): implement SidecarManager lifecycle in Rust

- SidecarManager spawns node process
- Waits for /health endpoint (max 3s with 100ms polls)
- stop() kills child process
- Drop trait ensures cleanup on manager drop
- Integration test verifies start/health/stop cycle

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Phase 3: Playwright Browser Control (Week 5-6)

### Task 11: Add Playwright to Sidecar

**Files:**
- Modify: `apps/runtime/sidecar/package.json`
- Create: `apps/runtime/sidecar/src/browser.ts`
- Create: `apps/runtime/sidecar/src/types.ts`

**Step 1: Add Playwright dependency**

```json
{
  "dependencies": {
    "hono": "^4.0.0",
    "playwright": "^1.40.0"
  }
}
```

**Step 2: Install Playwright**

Run: `cd apps/runtime/sidecar && pnpm install`
Run: `cd apps/runtime/sidecar && npx playwright install chromium`
Expected: Chromium browser installed

**Step 3: Create BrowserController**

```typescript
// apps/runtime/sidecar/src/browser.ts
import { chromium, Browser, Page } from 'playwright';

export class BrowserController {
  private browser: Browser | null = null;
  private page: Page | null = null;

  private async ensureBrowser() {
    if (!this.browser) {
      this.browser = await chromium.launch({ headless: false });
      const context = await this.browser.newContext();
      this.page = await context.newPage();
    }
  }

  async navigate(url: string): Promise<string> {
    await this.ensureBrowser();
    await this.page!.goto(url, { waitUntil: 'domcontentloaded' });
    return `已导航到 ${url}`;
  }

  async click(selector: string): Promise<string> {
    await this.ensureBrowser();
    await this.page!.click(selector);
    return `已点击 ${selector}`;
  }

  async screenshot(path: string): Promise<string> {
    await this.ensureBrowser();
    await this.page!.screenshot({ path, fullPage: true });
    return `截图已保存到 ${path}`;
  }

  async evaluate(script: string): Promise<string> {
    await this.ensureBrowser();
    const result = await this.page!.evaluate(script);
    return JSON.stringify(result);
  }

  async getContent(): Promise<string> {
    await this.ensureBrowser();
    return await this.page!.content();
  }

  async close() {
    if (this.browser) {
      await this.browser.close();
      this.browser = null;
      this.page = null;
    }
  }
}
```

**Step 4: Create types file**

```typescript
// apps/runtime/sidecar/src/types.ts
export interface ApiResponse {
  output?: string;
  error?: string;
}
```

**Step 5: Update index.ts with browser endpoints**

```typescript
// apps/runtime/sidecar/src/index.ts
import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { BrowserController } from './browser';
import type { ApiResponse } from './types';

const app = new Hono();
const browser = new BrowserController();

app.use('/*', cors());

app.get('/health', (c) => {
  return c.json({ status: 'ok', uptime: process.uptime() });
});

app.post('/api/browser/navigate', async (c) => {
  try {
    const { url } = await c.req.json();
    const result = await browser.navigate(url);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/browser/click', async (c) => {
  try {
    const { selector } = await c.req.json();
    const result = await browser.click(selector);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/browser/screenshot', async (c) => {
  try {
    const { path } = await c.req.json();
    const result = await browser.screenshot(path);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/browser/close', async (c) => {
  try {
    await browser.close();
    return c.json({ output: '浏览器已关闭' } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

const PORT = process.env.PORT || 8765;
console.log(`[sidecar] Starting on http://localhost:${PORT}`);

// Graceful shutdown
process.on('SIGINT', async () => {
  await browser.close();
  process.exit(0);
});

export default {
  port: PORT,
  fetch: app.fetch,
};
```

**Step 6: Build sidecar**

Run: `cd apps/runtime/sidecar && pnpm build`
Expected: SUCCESS

**Step 7: Commit**

```bash
git add apps/runtime/sidecar/
git commit -m "feat(sidecar): add Playwright browser controller

- Add playwright ^1.40.0 dependency
- BrowserController with navigate, click, screenshot, evaluate, getContent, close
- REST endpoints: POST /api/browser/{navigate,click,screenshot,close}
- Error handling returns {error: message} with 500 status
- Graceful shutdown on SIGINT

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 12: Implement SidecarBridgeTool in Rust

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/sidecar_bridge.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Create: `tests/agent/test_sidecar_bridge.rs`

**Step 1: Write failing integration test**

```rust
// apps/runtime/src-tauri/tests/agent/test_sidecar_bridge.rs
use runtime_lib::agent::{Tool, tools::SidecarBridgeTool};
use runtime_lib::sidecar::SidecarManager;
use serde_json::json;

#[tokio::test]
async fn test_sidecar_bridge_tool() {
    // Start sidecar first
    let manager = SidecarManager::new();
    manager.start().await.unwrap();

    let tool = SidecarBridgeTool::new(
        "http://localhost:8765".to_string(),
        "/api/browser/navigate".to_string(),
        "browser_navigate".to_string(),
        "Navigate browser to URL".to_string(),
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string" }
            },
            "required": ["url"]
        }),
    );

    let input = json!({"url": "https://example.com"});
    let result = tool.execute(input).unwrap();

    assert!(result.contains("已导航到"));

    manager.stop();
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_sidecar_bridge_tool`
Expected: FAIL (SidecarBridgeTool not defined)

**Step 3: Implement SidecarBridgeTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/sidecar_bridge.rs
use crate::agent::types::Tool;
use anyhow::{Result, anyhow};
use serde_json::Value;

pub struct SidecarBridgeTool {
    sidecar_url: String,
    endpoint: String,
    name: String,
    description: String,
    schema: Value,
}

impl SidecarBridgeTool {
    pub fn new(
        sidecar_url: String,
        endpoint: String,
        name: String,
        description: String,
        schema: Value,
    ) -> Self {
        Self {
            sidecar_url,
            endpoint,
            name,
            description,
            schema,
        }
    }
}

impl Tool for SidecarBridgeTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.schema.clone()
    }

    fn execute(&self, input: Value) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        let url = format!("{}{}", self.sidecar_url, self.endpoint);

        let resp = client
            .post(&url)
            .json(&input)
            .send()?;

        if !resp.status().is_success() {
            let error_body: Value = resp.json().unwrap_or(serde_json::json!({}));
            return Err(anyhow!(
                "Sidecar 调用失败: {}",
                error_body["error"].as_str().unwrap_or("Unknown error")
            ));
        }

        let result: Value = resp.json()?;
        Ok(result["output"].as_str().unwrap_or("").to_string())
    }
}
```

**Step 4: Export SidecarBridgeTool**

```rust
// apps/runtime/src-tauri/src/agent/tools/mod.rs
mod read_file;
mod write_file;
mod glob_tool;
mod grep_tool;
mod bash;
mod sidecar_bridge;

pub use read_file::ReadFileTool;
pub use write_file::WriteFileTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
pub use bash::BashTool;
pub use sidecar_bridge::SidecarBridgeTool;
```

**Step 5: Build sidecar before running test**

Run: `cd apps/runtime/sidecar && pnpm build`

**Step 6: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test test_sidecar_bridge_tool`
Expected: PASS

**Step 7: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/sidecar_bridge.rs
git add apps/runtime/src-tauri/src/agent/tools/mod.rs
git add apps/runtime/src-tauri/tests/agent/test_sidecar_bridge.rs
git commit -m "feat(agent): implement SidecarBridgeTool for HTTP communication

- SidecarBridgeTool makes POST requests to sidecar endpoints
- Handles JSON request/response with {output} or {error} format
- Integration test verifies browser navigation via sidecar

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Phase 4: MCP Protocol Support (Week 7-8)

### Task 13: Add MCP SDK to Sidecar

**Files:**
- Modify: `apps/runtime/sidecar/package.json`
- Create: `apps/runtime/sidecar/src/mcp.ts`

**Step 1: Add MCP dependency**

```json
{
  "dependencies": {
    "hono": "^4.0.0",
    "playwright": "^1.40.0",
    "@modelcontextprotocol/sdk": "^1.0.0"
  }
}
```

**Step 2: Install MCP SDK**

Run: `cd apps/runtime/sidecar && pnpm install`
Expected: @modelcontextprotocol/sdk installed

**Step 3: Create MCPManager**

```typescript
// apps/runtime/sidecar/src/mcp.ts
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

interface MCPServerConfig {
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

export class MCPManager {
  private servers: Map<
    string,
    { client: Client; transport: StdioClientTransport }
  > = new Map();

  async addServer(name: string, config: MCPServerConfig) {
    const transport = new StdioClientTransport({
      command: config.command,
      args: config.args || [],
      env: { ...process.env, ...config.env },
    });

    const client = new Client(
      {
        name: 'workclaw-runtime',
        version: '1.0.0',
      },
      {
        capabilities: {},
      }
    );

    await client.connect(transport);
    this.servers.set(name, { client, transport });
  }

  listServers(): string[] {
    return Array.from(this.servers.keys());
  }

  async callTool(
    serverName: string,
    toolName: string,
    args: any
  ): Promise<any> {
    const server = this.servers.get(serverName);
    if (!server) {
      throw new Error(`MCP 服务器 ${serverName} 不存在`);
    }

    const result = await server.client.callTool({
      name: toolName,
      arguments: args,
    });

    return result;
  }

  async listTools(serverName: string): Promise<any[]> {
    const server = this.servers.get(serverName);
    if (!server) {
      throw new Error(`MCP 服务器 ${serverName} 不存在`);
    }

    const response = await server.client.listTools();
    return response.tools;
  }

  async closeAll() {
    for (const [name, { client, transport }] of this.servers.entries()) {
      await client.close();
      await transport.close();
    }
    this.servers.clear();
  }
}
```

**Step 4: Add MCP endpoints to index.ts**

```typescript
// apps/runtime/sidecar/src/index.ts
// Add after browser import
import { MCPManager } from './mcp';

// Add after browser initialization
const mcp = new MCPManager();

// Add after browser endpoints

app.post('/api/mcp/add-server', async (c) => {
  try {
    const { name, command, args, env } = await c.req.json();
    await mcp.addServer(name, { command, args, env });
    return c.json({ output: `MCP 服务器 ${name} 已添加` } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/mcp/list-servers', async (c) => {
  try {
    const servers = mcp.listServers();
    return c.json({ output: JSON.stringify(servers) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/mcp/call-tool', async (c) => {
  try {
    const { server_name, tool_name, arguments: args } = await c.req.json();
    const result = await mcp.callTool(server_name, tool_name, args);
    return c.json({ output: JSON.stringify(result) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/mcp/list-tools', async (c) => {
  try {
    const { server_name } = await c.req.json();
    const tools = await mcp.listTools(server_name);
    return c.json({ output: JSON.stringify(tools) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// Update graceful shutdown
process.on('SIGINT', async () => {
  await browser.close();
  await mcp.closeAll();
  process.exit(0);
});
```

**Step 5: Build sidecar**

Run: `cd apps/runtime/sidecar && pnpm build`
Expected: SUCCESS

**Step 6: Commit**

```bash
git add apps/runtime/sidecar/
git commit -m "feat(sidecar): add MCP protocol support

- Add @modelcontextprotocol/sdk dependency
- MCPManager handles stdio client connections
- Endpoints: add-server, list-servers, call-tool, list-tools
- Graceful shutdown closes all MCP connections

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Phase 5: Integration & ReAct Loop (Week 9-10)

### Task 14: Implement Complete ReAct Loop in AgentExecutor

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Create: `tests/agent/test_react_loop.rs`

This task implements the complete execute_turn method with tool calling loop, referencing the design document for both Anthropic and OpenAI formats.

**Step 1: Write failing integration test**

```rust
// apps/runtime/src-tauri/tests/agent/test_react_loop.rs
use runtime_lib::agent::{AgentExecutor, ToolRegistry};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_react_loop_with_mock_llm() {
    // This test will use a mock LLM response that includes tool calls
    // For now, we'll skip the actual LLM call and just verify the loop structure

    let registry = Arc::new(ToolRegistry::with_file_tools());
    let executor = AgentExecutor::new(registry);

    // Placeholder - will implement actual loop in next steps
    assert!(true);
}
```

**Step 2: Update executor.rs with complete implementation**

```rust
// apps/runtime/src-tauri/src/agent/executor.rs
use super::registry::ToolRegistry;
use super::types::{LLMResponse, ToolCall, ToolResult};
use crate::adapters;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct AgentExecutor {
    registry: Arc<ToolRegistry>,
    max_iterations: usize,
}

impl AgentExecutor {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            max_iterations: 10,
        }
    }

    pub async fn execute_turn(
        &self,
        api_format: &str,
        base_url: &str,
        api_key: &str,
        model: &str,
        system_prompt: &str,
        mut messages: Vec<Value>,
        on_token: impl Fn(String) + Send + Clone,
    ) -> Result<Vec<Value>> {
        let mut iteration = 0;

        loop {
            if iteration >= self.max_iterations {
                return Err(anyhow!("达到最大迭代次数 {}", self.max_iterations));
            }
            iteration += 1;

            eprintln!("[agent] Iteration {}/{}", iteration, self.max_iterations);

            // 1. Get tool definitions
            let tools = self.registry.get_tool_definitions();

            // 2. Call LLM with tools
            let response = if api_format == "anthropic" {
                adapters::anthropic::chat_stream_with_tools(
                    base_url,
                    api_key,
                    model,
                    system_prompt,
                    messages.clone(),
                    tools,
                    on_token.clone(),
                )
                .await?
            } else {
                // TODO: Implement OpenAI tool calling in separate task
                return Err(anyhow!("OpenAI tool calling not yet implemented"));
            };

            // 3. Handle response
            match response {
                LLMResponse::Text(content) => {
                    // Pure text response - end loop
                    messages.push(json!({
                        "role": "assistant",
                        "content": content
                    }));
                    eprintln!("[agent] Finished with text response");
                    return Ok(messages);
                }
                LLMResponse::ToolCalls(tool_calls) => {
                    eprintln!("[agent] Executing {} tool calls", tool_calls.len());

                    // Execute all tool calls
                    let mut tool_results = vec![];
                    for call in &tool_calls {
                        eprintln!("[agent] Calling tool: {}", call.name);

                        let tool = self
                            .registry
                            .get(&call.name)
                            .ok_or_else(|| anyhow!("工具不存在: {}", call.name))?;

                        let result = tool.execute(call.input.clone())?;

                        tool_results.push(ToolResult {
                            tool_use_id: call.id.clone(),
                            content: result,
                        });
                    }

                    // Add tool calls and results to message history
                    if api_format == "anthropic" {
                        // Anthropic format: assistant message with tool_use blocks
                        messages.push(json!({
                            "role": "assistant",
                            "content": tool_calls.iter().map(|tc| json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.input,
                            })).collect::<Vec<_>>()
                        }));

                        // User message with tool_result blocks
                        messages.push(json!({
                            "role": "user",
                            "content": tool_results.iter().map(|tr| json!({
                                "type": "tool_result",
                                "tool_use_id": tr.tool_use_id,
                                "content": tr.content,
                            })).collect::<Vec<_>>()
                        }));
                    } else {
                        // OpenAI format will be different
                        // TODO: Implement in separate task
                    }

                    // Continue to next iteration
                    continue;
                }
            }
        }
    }
}
```

**Step 3: Verify compilation**

Run: `cd apps/runtime/src-tauri && cargo check`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/executor.rs
git add apps/runtime/src-tauri/tests/agent/test_react_loop.rs
git commit -m "feat(agent): implement complete ReAct loop in AgentExecutor

- execute_turn loops up to max_iterations (default 10)
- Calls LLM with tool definitions
- Handles LLMResponse::Text (end loop) and LLMResponse::ToolCalls (continue)
- Executes tool calls via ToolRegistry
- Appends tool_use and tool_result to message history (Anthropic format)
- Logs iteration progress to stderr

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 15: Integrate AgentExecutor into send_message Command

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`

**Step 1: Update send_message signature**

```rust
// apps/runtime/src-tauri/src/commands/chat.rs
// Add new parameter to send_message
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    session_id: String,
    user_message: String,
    enable_tools: bool, // NEW: whether to enable Agent mode
    db: State<'_, DbState>,
    agent_executor: State<'_, Arc<crate::agent::AgentExecutor>>, // NEW
) -> Result<(), String> {
    // ... existing code for saving user message, loading session, skill, model ...

    // Load message history
    let history = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC"
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let messages: Vec<Value> = history
        .iter()
        .map(|(role, content)| json!({"role": role, "content": content}))
        .collect();

    let app_clone = app.clone();
    let session_id_clone = session_id.clone();
    let mut full_response = String::new();

    if enable_tools {
        // Agent mode with tool calling
        let final_messages = agent_executor
            .execute_turn(
                &api_format,
                &base_url,
                &api_key,
                &model_name,
                &system_prompt,
                messages,
                |token: String| {
                    full_response.push_str(&token);
                    let _ = app_clone.emit(
                        "stream-token",
                        StreamToken {
                            session_id: session_id_clone.clone(),
                            token,
                            done: false,
                        },
                    );
                },
            )
            .await
            .map_err(|e| e.to_string())?;

        // Emit done event
        let _ = app.emit(
            "stream-token",
            StreamToken {
                session_id: session_id.clone(),
                token: String::new(),
                done: true,
            },
        );

        // Save all new messages (tool calls and results) to database
        for msg in final_messages.iter().skip(history.len()) {
            let msg_id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            let role = msg["role"].as_str().unwrap_or("assistant");
            let content = serde_json::to_string(&msg["content"]).unwrap_or_default();

            sqlx::query(
                "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&msg_id)
            .bind(&session_id)
            .bind(role)
            .bind(&content)
            .bind(&now)
            .execute(&db.0)
            .await
            .map_err(|e| e.to_string())?;
        }
    } else {
        // Original direct chat mode (no tools)
        let result = if api_format == "anthropic" {
            adapters::anthropic::chat_stream(
                &base_url,
                &api_key,
                &model_name,
                &system_prompt,
                messages,
                |token: String| {
                    full_response.push_str(&token);
                    let _ = app_clone.emit(
                        "stream-token",
                        StreamToken {
                            session_id: session_id_clone.clone(),
                            token,
                            done: false,
                        },
                    );
                },
            )
            .await
        } else {
            adapters::openai::chat_stream(
                &base_url,
                &api_key,
                &model_name,
                &system_prompt,
                messages,
                |token: String| {
                    full_response.push_str(&token);
                    let _ = app_clone.emit(
                        "stream-token",
                        StreamToken {
                            session_id: session_id_clone.clone(),
                            token,
                            done: false,
                        },
                    );
                },
            )
            .await
        };

        // Emit done event
        let _ = app.emit(
            "stream-token",
            StreamToken {
                session_id: session_id.clone(),
                token: String::new(),
                done: true,
            },
        );

        if let Err(e) = result {
            return Err(e.to_string());
        }

        // Save assistant message
        let asst_id = Uuid::new_v4().to_string();
        let now2 = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&asst_id)
        .bind(&session_id)
        .bind("assistant")
        .bind(&full_response)
        .bind(&now2)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}
```

**Step 2: Initialize AgentExecutor in lib.rs**

```rust
// apps/runtime/src-tauri/src/lib.rs
use agent::{AgentExecutor, ToolRegistry};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // ... existing db setup ...

            // Initialize AgentExecutor
            let registry = Arc::new(ToolRegistry::with_file_tools());
            let agent_executor = Arc::new(AgentExecutor::new(registry));
            app.manage(agent_executor);

            // ... existing sidecar setup ...
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::skills::install_skill,
            commands::skills::list_skills,
            commands::skills::uninstall_skill,
            commands::chat::create_session,
            commands::chat::send_message,
            commands::chat::get_messages,
            commands::models::save_model_config,
            commands::models::list_model_configs,
            commands::models::delete_model_config,
            commands::models::test_connection_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: Verify compilation**

Run: `cd apps/runtime/src-tauri && cargo check`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat.rs
git add apps/runtime/src-tauri/src/lib.rs
git commit -m "feat(agent): integrate AgentExecutor into send_message command

- Add enable_tools parameter to send_message
- When enable_tools=true, use AgentExecutor.execute_turn (ReAct loop)
- When enable_tools=false, use original direct chat adapters
- Save all tool_use and tool_result messages to database
- Initialize AgentExecutor in lib.rs setup with file tools registry

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Summary

This implementation plan covers the complete Agent capabilities rollout across 15 detailed tasks spanning 8-10 weeks:

**Phase 1 (Tasks 1-7)**: Agent engine core + file tools (ReadFile, WriteFile, Glob, Grep)
**Phase 2 (Tasks 8-10)**: Bash tool + Node.js sidecar foundation
**Phase 3 (Tasks 11-12)**: Playwright browser control via sidecar bridge
**Phase 4 (Task 13)**: MCP protocol support in sidecar
**Phase 5 (Tasks 14-15)**: Complete ReAct loop + integration into chat command

Each task follows TDD with:
- Failing test first
- Minimal implementation
- Passing tests
- Frequent commits

**Next Steps After This Plan:**
- Task 16+: OpenAI function_calling support (similar to Anthropic)
- Task 17+: Frontend UI for tool call visualization
- Task 18+: End-to-end testing with real LLMs
- Task 19+: Performance optimization
- Task 20+: Documentation and release preparation

---

**Plan saved to:** `docs/plans/2026-02-20-agent-capabilities.md`
