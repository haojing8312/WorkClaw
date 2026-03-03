# WorkClaw Runtime Agent Capabilities 设计文档

**日期**: 2026-02-20
**状态**: 已批准
**范围**: Tool Calling + File/Bash Tools + Browser Control (Playwright) + MCP Protocol Support
**架构**: 混合 Rust + Node.js Sidecar

---

## 1. 背景与目标

### 1.1 当前状态
WorkClaw Runtime MVP 已完成基础聊天能力：
- ✅ Tauri 2 桌面应用框架
- ✅ SQLite 数据持久化 (installed_skills, sessions, messages, model_configs)
- ✅ LLM 适配器支持 9 大提供商 (OpenAI, Claude, MiniMax×2, DeepSeek, Qwen×2, Moonshot, Yi)
- ✅ SSE 流式响应
- ✅ Provider Preset 系统

### 1.2 目标
为 WorkClaw Runtime 添加完整的 Agent 执行能力，使其能够：
1. **Tool Calling**: 支持 Anthropic `tool_use` 和 OpenAI `function_calling` 格式
2. **文件操作**: ReadFile, WriteFile, Glob, Grep
3. **Bash 执行**: 跨平台命令执行 (Windows PowerShell / Unix bash)
4. **浏览器控制**: 通过 Playwright 实现网页自动化
5. **MCP 协议**: 连接外部 MCP 服务器扩展工具生态

### 1.3 参考实现
- **MiniMax Desktop Agent**: 15+ 工具、BrowserView、WebSocket+Protobuf、反侦测技术
- **WorkAny**: Claude Agent SDK、Tauri 2、Hono 后端、MCP 支持
- **Claude Code**: ReAct loop、丰富工具集、状态管理

---

## 2. 架构设计

### 2.1 三层架构

```
┌─────────────────────────────────────────────────┐
│         Tauri / Rust Backend                    │
│  - Agent Executor (ReAct Loop)                  │
│  - Tool Registry                                │
│  - Native Tools (File, Bash, SidecarBridge)     │
│  - LLM Adapters (anthropic.rs, openai.rs)       │
└─────────────────┬───────────────────────────────┘
                  │ HTTP REST (localhost:8765)
┌─────────────────▼───────────────────────────────┐
│         Node.js Sidecar (Hono Server)           │
│  - Playwright Browser Controller                │
│  - MCP Client Manager                           │
│  - REST API Endpoints (/api/browser/*, /api/mcp/*) │
└─────────────────────────────────────────────────┘
```

**职责划分**:
- **Rust**: 性能关键路径 (Agent 循环、文件 I/O、LLM 通信、数据库)
- **Node.js**: 生态依赖 (Playwright、MCP SDK、复杂 npm 包)
- **HTTP 通信**: RESTful JSON 请求/响应，统一错误处理

### 2.2 为什么选择混合架构？

| 需求          | 纯 Rust | 混合架构 | 纯 Node.js |
|---------------|---------|----------|------------|
| 性能          | ✅ 最优  | ✅ 良好   | ❌ 较差     |
| Playwright    | ❌ 困难  | ✅ 原生   | ✅ 原生     |
| MCP SDK       | ❌ 无    | ✅ 官方   | ✅ 官方     |
| 现有架构兼容  | ✅ 完美  | ✅ 良好   | ❌ 重写     |
| 开发速度      | ❌ 慢    | ✅ 快     | ✅ 快       |
| 维护成本      | ✅ 低    | ⚠️ 中等   | ❌ 高       |

**结论**: 混合架构在保持性能的同时，快速获得生态优势，是当前最佳平衡点。

---

## 3. Agent 执行引擎设计

### 3.1 Tool Trait 和工具注册表

```rust
// apps/runtime/src-tauri/src/agent/mod.rs
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn execute(&self, input: serde_json::Value) -> Result<String>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self { tools: HashMap::new() };
        // 注册内置工具
        registry.register(Arc::new(ReadFileTool));
        registry.register(Arc::new(WriteFileTool));
        registry.register(Arc::new(GlobTool));
        registry.register(Arc::new(GrepTool));
        registry.register(Arc::new(BashTool::new()));
        registry
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get_tool_definitions(&self) -> Vec<serde_json::Value> {
        self.tools.values().map(|t| json!({
            "name": t.name(),
            "description": t.description(),
            "input_schema": t.input_schema(),
        })).collect()
    }
}
```

### 3.2 ReAct Loop 实现

```rust
pub enum AgentState {
    Thinking,      // LLM 正在推理
    ToolCalling,   // 执行工具调用
    Finished,      // 对话完成
    Error(String), // 错误状态
}

pub struct AgentExecutor {
    registry: Arc<ToolRegistry>,
    max_iterations: usize,
}

impl AgentExecutor {
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

            // 1. 调用 LLM (带工具定义)
            let tools = self.registry.get_tool_definitions();
            let response = if api_format == "anthropic" {
                self.call_anthropic_with_tools(base_url, api_key, model, system_prompt, &messages, tools, on_token.clone()).await?
            } else {
                self.call_openai_with_tools(base_url, api_key, model, system_prompt, &messages, tools, on_token.clone()).await?
            };

            // 2. 解析响应
            match response {
                LLMResponse::Text(content) => {
                    // 纯文本响应，结束循环
                    messages.push(json!({"role": "assistant", "content": content}));
                    return Ok(messages);
                }
                LLMResponse::ToolCalls(tool_calls) => {
                    // 执行工具调用
                    let mut tool_results = vec![];
                    for call in tool_calls {
                        let result = self.registry.tools
                            .get(&call.name)
                            .ok_or_else(|| anyhow!("工具不存在: {}", call.name))?
                            .execute(call.input)?;
                        tool_results.push(ToolResult {
                            tool_use_id: call.id,
                            content: result,
                        });
                    }

                    // 将工具结果添加到消息历史
                    if api_format == "anthropic" {
                        messages.push(json!({
                            "role": "user",
                            "content": tool_results.iter().map(|r| json!({
                                "type": "tool_result",
                                "tool_use_id": r.tool_use_id,
                                "content": r.content,
                            })).collect::<Vec<_>>()
                        }));
                    } else {
                        // OpenAI 格式
                        for result in tool_results {
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": result.tool_use_id,
                                "content": result.content,
                            }));
                        }
                    }

                    // 继续下一轮循环
                    continue;
                }
            }
        }
    }
}
```

### 3.3 Anthropic tool_use 处理

```rust
async fn call_anthropic_with_tools(
    &self,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: Vec<Value>,
    on_token: impl Fn(String) + Send,
) -> Result<LLMResponse> {
    let body = json!({
        "model": model,
        "system": system_prompt,
        "messages": messages,
        "tools": tools,
        "max_tokens": 4096,
        "stream": true,
    });

    let mut tool_calls = vec![];
    let mut text_content = String::new();
    let mut current_tool_call: Option<ToolCall> = None;

    // SSE 流式解析
    // 处理事件类型: content_block_start, content_block_delta, content_block_stop, message_stop

    match event_type.as_str() {
        "content_block_start" => {
            if data["content_block"]["type"] == "tool_use" {
                current_tool_call = Some(ToolCall {
                    id: data["content_block"]["id"].as_str().unwrap().to_string(),
                    name: data["content_block"]["name"].as_str().unwrap().to_string(),
                    input: json!({}),
                });
            }
        }
        "content_block_delta" => {
            if data["delta"]["type"] == "text_delta" {
                let token = data["delta"]["text"].as_str().unwrap();
                text_content.push_str(token);
                on_token(token.to_string());
            } else if data["delta"]["type"] == "input_json_delta" {
                // 累积工具参数 JSON 片段
            }
        }
        "content_block_stop" => {
            if let Some(call) = current_tool_call.take() {
                tool_calls.push(call);
            }
        }
        "message_stop" => break,
        _ => {}
    }

    if !tool_calls.is_empty() {
        Ok(LLMResponse::ToolCalls(tool_calls))
    } else {
        Ok(LLMResponse::Text(text_content))
    }
}
```

### 3.4 OpenAI function_calling 处理

```rust
async fn call_openai_with_tools(
    &self,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: Vec<Value>,
    on_token: impl Fn(String) + Send,
) -> Result<LLMResponse> {
    let mut all_messages = vec![json!({"role": "system", "content": system_prompt})];
    all_messages.extend_from_slice(messages);

    let body = json!({
        "model": model,
        "messages": all_messages,
        "tools": tools.iter().map(|t| json!({
            "type": "function",
            "function": {
                "name": t["name"],
                "description": t["description"],
                "parameters": t["input_schema"],
            }
        })).collect::<Vec<_>>(),
        "stream": true,
    });

    let mut tool_calls = vec![];
    let mut text_content = String::new();

    // 流式解析
    while let Some(chunk) = stream.next().await {
        let delta = &v["choices"][0]["delta"];

        if let Some(content) = delta["content"].as_str() {
            text_content.push_str(content);
            on_token(content.to_string());
        }

        if let Some(tool_call_array) = delta["tool_calls"].as_array() {
            for tc in tool_call_array {
                tool_calls.push(ToolCall {
                    id: tc["id"].as_str().unwrap().to_string(),
                    name: tc["function"]["name"].as_str().unwrap().to_string(),
                    input: serde_json::from_str(tc["function"]["arguments"].as_str().unwrap())?,
                });
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

### 3.5 集成到现有 send_message 命令

修改 `apps/runtime/src-tauri/src/commands/chat.rs`:

```rust
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    session_id: String,
    user_message: String,
    enable_tools: bool,  // 新参数：是否启用工具
    db: State<'_, DbState>,
    agent_executor: State<'_, Arc<AgentExecutor>>,  // 新依赖
) -> Result<(), String> {
    // ... 保存用户消息、加载会话、加载技能、加载模型配置 ...

    if enable_tools {
        // Agent 模式：支持工具调用
        let final_messages = agent_executor.execute_turn(
            &api_format,
            &base_url,
            &api_key,
            &model_name,
            &system_prompt,
            messages,
            |token| {
                full_response.push_str(&token);
                let _ = app_clone.emit("stream-token", StreamToken {
                    session_id: session_id_clone.clone(),
                    token,
                    done: false,
                });
            },
        ).await.map_err(|e| e.to_string())?;

        // 保存所有新消息（包括工具调用和结果）
        for msg in final_messages.iter().skip(messages.len()) {
            // ... 保存到 messages 表 ...
        }
    } else {
        // 原有的直接聊天模式（不带工具）
        // ... 现有逻辑保持不变 ...
    }

    Ok(())
}
```

---

## 4. Rust 原生工具实现

### 4.1 ReadFileTool

```rust
// apps/runtime/src-tauri/src/agent/tools/read_file.rs
pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }

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
        let path = input["path"].as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;

        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("读取文件失败: {}", e))?;

        Ok(content)
    }
}
```

### 4.2 WriteFileTool

```rust
pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }

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
        let path = input["path"].as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let content = input["content"].as_str()
            .ok_or_else(|| anyhow!("缺少 content 参数"))?;

        // 确保父目录存在
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, content)
            .map_err(|e| anyhow!("写入文件失败: {}", e))?;

        Ok(format!("成功写入 {} 字节到 {}", content.len(), path))
    }
}
```

### 4.3 GlobTool

```rust
pub struct GlobTool;

impl Tool for GlobTool {
    fn name(&self) -> &str { "glob" }

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
        let pattern = input["pattern"].as_str()
            .ok_or_else(|| anyhow!("缺少 pattern 参数"))?;
        let base_dir = input["base_dir"].as_str().unwrap_or(".");

        let full_pattern = format!("{}/{}", base_dir, pattern);
        let paths: Vec<String> = glob::glob(&full_pattern)?
            .filter_map(|r| r.ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        Ok(format!("找到 {} 个文件:\n{}", paths.len(), paths.join("\n")))
    }
}
```

### 4.4 GrepTool

```rust
pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str { "grep" }

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
        let pattern = input["pattern"].as_str()
            .ok_or_else(|| anyhow!("缺少 pattern 参数"))?;
        let path = input["path"].as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let case_insensitive = input["case_insensitive"].as_bool().unwrap_or(false);

        let re = if case_insensitive {
            regex::RegexBuilder::new(pattern).case_insensitive(true).build()?
        } else {
            regex::Regex::new(pattern)?
        };

        let content = std::fs::read_to_string(path)?;
        let matches: Vec<String> = content.lines()
            .enumerate()
            .filter(|(_, line)| re.is_match(line))
            .map(|(i, line)| format!("{}:{}", i + 1, line))
            .collect();

        Ok(format!("找到 {} 处匹配:\n{}", matches.len(), matches.join("\n")))
    }
}
```

### 4.5 BashTool

```rust
// apps/runtime/src-tauri/src/agent/tools/bash.rs
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
    fn name(&self) -> &str { "bash" }

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
        let command = input["command"].as_str()
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
            self.background_processes.lock().unwrap().insert(pid.clone(), child);

            Ok(format!("后台进程已启动，PID: {}", pid))
        } else {
            let output = Command::new(shell)
                .arg(flag)
                .arg(command)
                .output()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !output.status.success() {
                Ok(format!("命令执行失败（退出码 {}）\nstderr:\n{}",
                    output.status.code().unwrap_or(-1), stderr))
            } else {
                Ok(format!("stdout:\n{}\nstderr:\n{}", stdout, stderr))
            }
        }
    }
}
```

### 4.6 SidecarBridgeTool 模式

```rust
// apps/runtime/src-tauri/src/agent/tools/sidecar_bridge.rs
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
        Self { sidecar_url, endpoint, name, description, schema }
    }
}

impl Tool for SidecarBridgeTool {
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { &self.description }
    fn input_schema(&self) -> Value { self.schema.clone() }

    fn execute(&self, input: Value) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        let url = format!("{}{}", self.sidecar_url, self.endpoint);

        let resp = client.post(&url)
            .json(&input)
            .send()?;

        if !resp.status().is_success() {
            return Err(anyhow!("Sidecar 调用失败: {}", resp.status()));
        }

        let result: Value = resp.json()?;
        Ok(result["output"].as_str().unwrap_or("").to_string())
    }
}

// 用法示例：注册 Playwright 工具
registry.register(Arc::new(SidecarBridgeTool::new(
    "http://localhost:8765".to_string(),
    "/api/browser/navigate".to_string(),
    "browser_navigate".to_string(),
    "导航浏览器到指定 URL".to_string(),
    json!({
        "type": "object",
        "properties": {
            "url": { "type": "string", "description": "目标 URL" }
        },
        "required": ["url"]
    }),
)));
```

---

## 5. Node.js Sidecar 架构

### 5.1 项目结构

```
apps/runtime/sidecar/
├── package.json
├── tsconfig.json
├── src/
│   ├── index.ts           # Hono server 入口
│   ├── browser.ts         # Playwright controller
│   ├── mcp.ts             # MCP client manager
│   └── types.ts
└── dist/                  # 编译产物
```

### 5.2 Hono Server 设置

```typescript
// apps/runtime/sidecar/src/index.ts
import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { BrowserController } from './browser';
import { MCPManager } from './mcp';

const app = new Hono();
const browser = new BrowserController();
const mcp = new MCPManager();

app.use('/*', cors());

// 健康检查
app.get('/health', (c) => {
  return c.json({ status: 'ok', uptime: process.uptime() });
});

// Playwright 端点
app.post('/api/browser/navigate', async (c) => {
  const { url } = await c.req.json();
  const result = await browser.navigate(url);
  return c.json({ output: result });
});

app.post('/api/browser/click', async (c) => {
  const { selector } = await c.req.json();
  const result = await browser.click(selector);
  return c.json({ output: result });
});

app.post('/api/browser/screenshot', async (c) => {
  const { path } = await c.req.json();
  const result = await browser.screenshot(path);
  return c.json({ output: result });
});

app.post('/api/browser/close', async (c) => {
  await browser.close();
  return c.json({ output: '浏览器已关闭' });
});

// MCP 端点
app.post('/api/mcp/add-server', async (c) => {
  const { name, command, args, env } = await c.req.json();
  await mcp.addServer(name, { command, args, env });
  return c.json({ output: `MCP 服务器 ${name} 已添加` });
});

app.post('/api/mcp/list-servers', async (c) => {
  const servers = mcp.listServers();
  return c.json({ output: JSON.stringify(servers) });
});

app.post('/api/mcp/call-tool', async (c) => {
  const { server_name, tool_name, arguments: args } = await c.req.json();
  const result = await mcp.callTool(server_name, tool_name, args);
  return c.json({ output: JSON.stringify(result) });
});

const PORT = process.env.PORT || 8765;
console.log(`Sidecar server starting on http://localhost:${PORT}`);
app.fire();

// 优雅关闭
process.on('SIGINT', async () => {
  await browser.close();
  await mcp.closeAll();
  process.exit(0);
});
```

### 5.3 Playwright Controller

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

### 5.4 MCP Client Manager

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
  private servers: Map<string, { client: Client; transport: StdioClientTransport }> = new Map();

  async addServer(name: string, config: MCPServerConfig) {
    const transport = new StdioClientTransport({
      command: config.command,
      args: config.args || [],
      env: { ...process.env, ...config.env },
    });

    const client = new Client({
      name: 'workclaw-runtime',
      version: '1.0.0',
    }, {
      capabilities: {},
    });

    await client.connect(transport);
    this.servers.set(name, { client, transport });
  }

  listServers(): string[] {
    return Array.from(this.servers.keys());
  }

  async callTool(serverName: string, toolName: string, args: any): Promise<any> {
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

### 5.5 Rust Sidecar 生命周期管理

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
            return Ok(()); // 已启动
        }

        // 启动 Node.js sidecar
        let child = Command::new("node")
            .arg("sidecar/dist/index.js")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        *proc = Some(child);

        // 等待服务器就绪
        for _ in 0..30 {
            if self.health_check().await.is_ok() {
                eprintln!("[sidecar] 服务已启动: {}", self.url);
                return Ok(());
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Err(anyhow::anyhow!("Sidecar 启动超时"))
    }

    async fn health_check(&self) -> Result<()> {
        let resp = reqwest::get(&format!("{}/health", self.url)).await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("健康检查失败"))
        }
    }

    pub fn stop(&self) {
        let mut proc = self.process.lock().unwrap();
        if let Some(mut child) = proc.take() {
            let _ = child.kill();
            eprintln!("[sidecar] 服务已停止");
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

```rust
// apps/runtime/src-tauri/src/lib.rs 中集成
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let sidecar = Arc::new(SidecarManager::new());
    let sidecar_clone = sidecar.clone();

    tauri::Builder::default()
        .setup(move |app| {
            // 启动 sidecar
            tauri::async_runtime::spawn(async move {
                if let Err(e) = sidecar_clone.start().await {
                    eprintln!("[sidecar] 启动失败: {}", e);
                }
            });

            // ... 现有 setup 逻辑 ...
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ... 现有命令 ...
        ])
        .manage(sidecar)  // 提供给命令使用
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 5.6 Rust ↔ Node.js 通信协议

所有 HTTP 请求/响应使用统一格式：

**请求**:
```json
POST /api/{category}/{action}
Content-Type: application/json

{
  "param1": "value1",
  "param2": "value2"
}
```

**响应**:
```json
{
  "output": "结果字符串或 JSON 字符串"
}
```

**错误**:
```json
{
  "error": "错误信息"
}
```

---

## 6. 实施路线图 (8-10 周)

### Phase 1: Agent Engine + File Tools (Week 1-2)
1. 实现 `Tool` trait 和 `ToolRegistry`
2. 构建 `AgentExecutor` 的 ReAct loop
3. 支持 Anthropic `tool_use` 和 OpenAI `function_calling`
4. 实现文件工具: `ReadFileTool`, `WriteFileTool`, `GlobTool`, `GrepTool`
5. 添加工具调用可视化 UI 组件
6. 端到端测试：文件操作 Agent

**交付物**: 能够读写文件、搜索文件的 Agent 原型

---

### Phase 2: Bash Tools + Sidecar Foundation (Week 3-4)
1. 实现 `BashTool` 跨平台命令执行
2. 添加后台进程管理
3. 构建 Node.js sidecar 骨架 (Hono + 健康检查)
4. 实现 `SidecarManager` 生命周期管理
5. 创建健康检查机制
6. 端到端测试：Bash 命令执行 + Sidecar 通信

**交付物**: 支持 Bash 的 Agent + 运行的 Sidecar 服务

---

### Phase 3: Playwright Browser Control (Week 5-6)
1. 集成 Playwright 到 Node.js sidecar
2. 实现 `BrowserController` 核心方法 (navigate, click, screenshot, evaluate, getContent)
3. 创建 REST API 端点 (`/api/browser/*`)
4. 创建 Rust 桥接工具 (`SidecarBridgeTool`)
5. 注册浏览器工具到 Agent
6. 端到端测试：网页自动化任务

**交付物**: 能够控制浏览器的 Agent

---

### Phase 4: MCP Protocol Support (Week 7-8)
1. 集成 MCP SDK 到 Node.js sidecar
2. 实现 `MCPManager` (添加/移除/列出服务器)
3. 创建 MCP 工具注册桥接
4. 支持 stdio 和 HTTP 传输
5. 添加 MCP 工具到 Agent 注册表
6. 端到端测试：连接外部 MCP 服务器

**交付物**: 支持 MCP 协议的可扩展 Agent

---

### Phase 5: Integration & Polish (Week 9-10)
1. 所有工具类型的端到端测试
2. 性能优化 (缓存、连接池)
3. 错误处理改进
4. 用户文档和示例
5. 发布准备

**交付物**: 生产就绪的 WorkClaw Runtime Agent 系统

---

## 7. 依赖清单

### Rust 新增依赖
```toml
# apps/runtime/src-tauri/Cargo.toml
[dependencies]
glob = "0.3"
regex = "1"
# ... 现有依赖保持不变 ...
```

### Node.js Sidecar 依赖
```json
{
  "name": "workclaw-runtime-sidecar",
  "version": "1.0.0",
  "dependencies": {
    "hono": "^4.0.0",
    "playwright": "^1.40.0",
    "@modelcontextprotocol/sdk": "^1.0.0"
  },
  "devDependencies": {
    "typescript": "^5.3.0",
    "@types/node": "^20.0.0"
  }
}
```

---

## 8. 风险与缓解

| 风险                          | 影响 | 概率 | 缓解措施                                      |
|-------------------------------|------|------|-----------------------------------------------|
| Sidecar 进程管理不稳定        | 高   | 中   | 增加进程监控、自动重启、健康检查               |
| 工具调用循环无限递归          | 高   | 中   | 设置 `max_iterations` 限制 (默认 10)          |
| MCP 服务器兼容性问题          | 中   | 高   | 严格遵循 MCP 规范，添加版本检查                |
| 浏览器自动化被反爬            | 中   | 中   | 参考 MiniMax 反侦测技术 (后续优化)            |
| HTTP 通信延迟影响性能         | 低   | 低   | 使用连接池、批量请求优化                      |

---

## 9. 成功标准

1. ✅ Agent 能够正确执行 Anthropic 和 OpenAI 格式的 tool_use/function_calling
2. ✅ 文件工具能够读写、搜索本地文件系统
3. ✅ Bash 工具能够跨平台执行命令 (Windows PowerShell / Unix bash)
4. ✅ Playwright 工具能够导航、点击、截图网页
5. ✅ MCP 工具能够连接外部 MCP 服务器并调用其工具
6. ✅ Sidecar 进程在 Tauri 应用启动时自动启动，关闭时自动停止
7. ✅ 所有工具调用结果正确返回给 LLM 并继续对话循环
8. ✅ UI 能够可视化工具调用过程 (正在调用哪个工具、参数、结果)

---

## 10. 附录：参考资料

- **Anthropic Messages API**: https://docs.anthropic.com/en/api/messages-tools
- **OpenAI Function Calling**: https://platform.openai.com/docs/guides/function-calling
- **MCP Protocol Spec**: https://modelcontextprotocol.io/docs
- **Playwright API**: https://playwright.dev/docs/api/class-browser
- **Hono Documentation**: https://hono.dev/
- **MiniMax 逆向分析报告**: `reference/minimax/` (15+ 工具、BrowserView、反侦测)
- **WorkAny 开源实现**: Claude Agent SDK + Tauri 2 + Hono + MCP

---

**批准日期**: 2026-02-20
**批准人**: 用户 (选择 Option D - 全功能支持, Option 2 - 混合架构)
**下一步**: 调用 `writing-plans` skill 创建详细实施计划
