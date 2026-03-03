# Skill Runtime 增强设计文档

**目标**：将 WorkClaw Runtime 的 Agent 系统从基础 ReAct 执行引擎提升到接近 Claude Code 的 Skill 运行体验。

**背景**：当前 Agent 有 5 个文件工具 + Bash + MCP 集成，能完成基本的工具调用循环。但与 Claude Code 对比，缺少 Edit 精确编辑、多 Agent 协调、Skill 元数据解析、权限模型、上下文管理等关键能力，无法完整运行 `.claude/skills` 下的 18 个 skill。

**方案**：自底向上分 4 个 Phase 逐步实现，每个 Phase 完成后独立可测。

---

## 能力差距分析

| 类别 | 当前能力 | Claude Code 能力 | 差距 |
|------|---------|-----------------|------|
| 工具层 | Read/Write/Glob/Grep/Bash | + Edit/TodoWrite/WebFetch/WebSearch/AskUser/Task | 缺 6 个关键工具 |
| Skill 加载 | SKILL.md 纯文本作为 system prompt | Frontmatter 解析、工具白名单、模型覆盖 | 无元数据支持 |
| 多 Agent | 无 | Task 工具、子 agent 隔离、并行执行 | 完全缺失 |
| 权限安全 | 无限制 | 5 种权限模式、沙箱、工具白名单 | 完全缺失 |
| 上下文管理 | 全量历史 | token 预算裁剪、输出截断 | 无裁剪机制 |
| 持久记忆 | 无 | memory 目录、跨会话知识 | 完全缺失 |
| 用户交互 | 仅文本输出 | AskUserQuestion 工具暂停等待 | 无交互式暂停 |

---

## Phase 1: 工具补齐

### 1.1 Edit 工具（精确文本替换）

**文件**: `agent/tools/edit_tool.rs`

**功能**：在文件中查找 `old_string`，验证唯一性后替换为 `new_string`。

**参数**：
- `path: String` — 文件路径
- `old_string: String` — 要替换的文本
- `new_string: String` — 替换后的文本
- `replace_all: bool`（可选，默认 false）— 是否替换所有匹配

**逻辑**：
1. 读取文件全部内容
2. 查找 `old_string` 出现次数
3. `replace_all=false` 时：出现 0 次报错"未找到"，出现 >1 次报错"不唯一，请提供更多上下文"
4. 执行替换并写回文件
5. 返回: `"成功替换 N 处，文件: {path}"`

### 1.2 上下文裁剪

**文件**: `agent/executor.rs`（`execute_turn` 方法中）

**逻辑**：
- 在调用 LLM 前估算消息总 token 数（字符数 / 4）
- 设 token 预算为模型上下文的 70%（默认 4096 * 0.7 ≈ 2800 tokens，可通过 Skill 配置调整）
- 超预算时从第 2 条消息开始裁剪（保留 system prompt + 最新消息）
- 被裁剪的消息替换为: `[前 N 条消息已省略]`

**后续增强**：可通过 model_configs 表增加 `context_size` 字段，让用户配置每个模型的上下文长度。

### 1.3 TodoWrite 工具

**文件**: `agent/tools/todo_tool.rs`

**状态存储**: `Arc<RwLock<Vec<TodoItem>>>` 作为 Tauri State 管理。

**TodoItem 结构**：
```rust
struct TodoItem {
    id: String,        // UUID
    subject: String,
    description: String,
    status: String,    // "pending" | "in_progress" | "completed"
}
```

**操作**：
- `create`: 创建任务，返回 id
- `update`: 更新状态/内容
- `list`: 列出所有任务
- `delete`: 删除任务

**注意**：TodoWrite 是会话级内存工具（不持久化到数据库），随会话结束清空。

### 1.4 工具输出截断

**文件**: `agent/executor.rs`

**逻辑**：
- 工具执行完成后，检查 `result.len()`
- 超过 30,000 字符时截断为前 30,000 字符 + `"\n\n[输出已截断，共 {total} 字符，已显示前 30000 字符]"`
- 此逻辑在 executor 中统一处理，不需要修改每个工具

---

## Phase 2: Skill 元数据与加载增强

### 2.1 Skill Frontmatter 解析

**文件**: 新建 `agent/skill_config.rs`

**SkillConfig 结构**：
```rust
pub struct SkillConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub max_iterations: Option<usize>,
    pub system_prompt: String,  // frontmatter 之后的正文
}
```

**解析逻辑**：
1. 检查 SKILL.md 是否以 `---` 开头
2. 如果是，提取两个 `---` 之间的 YAML
3. 解析 YAML 为 SkillConfig 字段
4. 正文部分（第二个 `---` 之后）作为 `system_prompt`
5. 如果没有 frontmatter，整个文件作为 `system_prompt`

**依赖**: 新增 `serde_yaml` crate。

### 2.2 工具白名单执行

**修改文件**: `commands/chat.rs`（`send_message`）

**逻辑**：
- `send_message` 中解析 SKILL.md 时使用 `SkillConfig::parse()`
- 如果 `allowed_tools` 非空，构建工具白名单集合
- 在 `execute_turn` 前，只传入白名单内的工具定义
- executor 执行工具时也检查白名单，非法工具返回 `"此 Skill 不允许使用工具: {name}"`

### 2.3 System Prompt 模板化

**修改文件**: `commands/chat.rs`（`send_message`）

**模板**：
```
{skill_config.system_prompt}

---
运行环境:
- 工作目录: {skill_work_dir 或 app_data_dir}
- 可用工具: {filtered_tool_names.join(", ")}
- 模型: {model_name}
- 最大迭代次数: {max_iterations}
```

---

## Phase 3: 多 Agent 协调

### 3.1 Task 工具（子 Agent 分发）

**文件**: `agent/tools/task_tool.rs`

**参数**：
- `prompt: String` — 子 agent 任务描述
- `agent_type: String` — `"general-purpose"` | `"explore"` | `"plan"`（默认 `"general-purpose"`）
- `allowed_tools: Vec<String>`（可选）— 覆盖子 agent 工具集

**子 Agent 类型对应**：

| agent_type | 工具限制 | 模型 | 最大迭代 |
|-----------|---------|------|---------|
| `explore` | Read/Glob/Grep 只读 | 同主 agent | 5 |
| `plan` | Read/Glob/Grep/Bash(只读命令) | 同主 agent | 10 |
| `general-purpose` | 全部工具 | 同主 agent | 10 |

**执行流程**：
1. 根据 `agent_type` 确定工具集和迭代限制
2. 构建子 agent 的 system prompt：`"你是一个专注的子 agent。完成以下任务后返回结果。"`
3. 创建空的消息历史：`[{"role": "user", "content": prompt}]`
4. 创建临时 AgentExecutor（使用过滤后的 ToolRegistry）
5. 调用 `execute_turn`（子 agent 的 on_token 发送独立事件）
6. 提取最终 assistant 文本，作为工具结果返回

**关键约束**：
- 子 agent 不能再创建子 agent（Task 工具不在子 agent 工具集中）
- 子 agent 的 session_id 使用临时 UUID（不持久化）
- 子 agent 的 tool-call-event 附带 `is_sub_agent: true` 标记

### 3.2 子 Agent 上下文隔离

**实现方式**：
- Task 工具执行时完全不传入主 agent 的消息历史
- 子 agent 使用独立的 system prompt（不包含主 agent 的 skill 上下文）
- 子 agent 的工具调用日志不混入主 agent 的 session

### 3.3 子 Agent 结果聚合

**返回格式**：
```
子 Agent ({agent_type}) 执行完成:

{final_assistant_text}
```

如果子 agent 达到最大迭代次数未完成，返回：
```
子 Agent ({agent_type}) 达到最大迭代次数 ({max}):

最后输出: {last_text}
```

### 3.4 后台子 Agent（并行执行）

**参数扩展**：
- `run_in_background: bool`（默认 false）— 是否后台运行

**后台模式**：
- 使用 `tokio::spawn` 启动子 agent
- 立即返回 `task_id`（UUID）
- 子 agent 完成后将结果存入 `Arc<RwLock<HashMap<String, String>>>` (BackgroundTasks state)

**TaskOutput 工具**：
```rust
// 参数: task_id: String, block: bool (默认 true)
// block=true: 等待任务完成后返回结果
// block=false: 立即返回当前状态（"running" 或结果）
```

### 3.5 前端子 Agent 展示

**新增 Tauri 事件**: `sub-agent-event`

```rust
#[derive(serde::Serialize, Clone)]
struct SubAgentEvent {
    session_id: String,
    task_id: String,
    agent_type: String,
    prompt: String,
    status: String,      // "started" | "completed" | "error"
    result: Option<String>,
}
```

**ChatView 展示**：
- 子 agent 开始时显示折叠卡片（类似 ToolCallCard）
- 卡片标题: `"子 Agent: {agent_type}"`
- 展开后显示 prompt 和 result

---

## Phase 4: 高级特性

### 4.1 权限模型

**文件**: 新建 `agent/permissions.rs`

**权限模式**：
```rust
pub enum PermissionMode {
    Default,       // Write/Edit/Bash 需要用户确认
    AcceptEdits,   // Write/Edit 自动通过，Bash 仍需确认
    Unrestricted,  // 全部自动通过
}
```

**确认机制**：
1. executor 执行工具前检查权限
2. 需要确认时发送 `permission-request` 事件到前端
3. 前端弹出确认对话框（显示工具名、参数预览）
4. 用户点击"允许"或"拒绝"
5. 通过 `oneshot::channel` 等待前端响应
6. 拒绝时返回 `"用户拒绝了此操作"`

**配置**：权限模式在 SettingsView 中配置，存入数据库 `app_settings` 表。

### 4.2 WebFetch 工具

**文件**: `agent/tools/web_fetch.rs`

**参数**：
- `url: String` — 要获取的 URL
- `prompt: String`（可选）— 对获取内容的处理指示

**逻辑**：
1. 使用 `reqwest` GET 请求获取 URL
2. HTML 内容去除 `<script>`、`<style>` 标签
3. 提取纯文本内容
4. 截断到 30,000 字符
5. 如有 `prompt`，将内容和 prompt 合并返回

### 4.3 WebSearch 工具

**文件**: `agent/tools/web_search.rs`

**参数**：
- `query: String` — 搜索关键词

**实现方式**：通过 Sidecar 代理搜索请求

**Sidecar 端点**: `POST /api/web/search`
```json
{ "query": "search terms", "count": 5 }
```

**返回格式**：
```
搜索结果:

1. [标题](URL)
   摘要文本...

2. [标题](URL)
   摘要文本...
```

**搜索 Provider**：初期使用 DuckDuckGo（无需 API Key），后续可配置 Google/Bing。

### 4.4 持久内存

**文件**: `agent/tools/memory_tool.rs`

**存储位置**: `{app_data_dir}/memory/{skill_id}/`

**操作**：
- `read(key)` — 读取 `{key}.md` 文件
- `write(key, content)` — 写入 `{key}.md` 文件
- `list()` — 列出所有内存文件
- `delete(key)` — 删除 `{key}.md` 文件

**自动注入**：
- `send_message` 中，如果 `memory/MEMORY.md` 存在
- 将其内容追加到 system prompt 末尾：
```
---
持久记忆 (MEMORY.md):
{memory_content}
```

### 4.5 AskUser 工具

**文件**: `agent/tools/ask_user.rs`

**这是最复杂的工具**，因为需要暂停 ReAct 循环等待用户输入。

**参数**：
- `question: String` — 要问用户的问题
- `options: Vec<String>`（可选）— 预设选项

**实现方案**：

**修改 executor 的 Tool trait**：
```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: Value) -> Result<String>;
    fn is_interactive(&self) -> bool { false }  // 新增
}
```

**AskUser 执行流程**：
1. AskUser 工具标记为 `is_interactive() = true`
2. executor 检测到交互式工具时：
   - 发送 `ask-user-event` 到前端
   - 返回 `LLMResponse::Pending(tool_call_id, question)` 新状态
3. `send_message` 捕获 Pending 状态，保存当前上下文到数据库
4. 前端展示问题 UI，用户回答后调用 `resume_message(session_id, answer)`
5. `resume_message` 从数据库恢复上下文，将 answer 作为 tool_result 继续循环

**前端 UI**：
- 在 ChatView 中展示问题卡片（带选项按钮）
- 用户选择/输入后发送 `resume_message`

---

## 实现优先级总结

| Phase | 预估 Tasks | 关键交付 | 可运行 Skill 示例 |
|-------|----------|---------|------------------|
| Phase 1 | 4 | Edit/TodoWrite/上下文裁剪/输出截断 | `verification-before-completion` |
| Phase 2 | 3 | Frontmatter 解析/工具白名单/Prompt 模板 | `test-driven-development` |
| Phase 3 | 5 | Task 工具/子 agent/并行/前端展示 | `subagent-driven-development` |
| Phase 4 | 5 | 权限/WebFetch/WebSearch/持久内存/AskUser | `brainstorming`（完整） |
| **合计** | **17** | | 18 个 skill 全部可运行 |

---

## 技术依赖

**Rust crate 新增**：
- `serde_yaml` — YAML frontmatter 解析
- `scraper` 或 `select` — HTML 文本提取（WebFetch）
- `tokio::sync::oneshot` — 权限确认通道

**Sidecar 新增端点**：
- `POST /api/web/search` — Web 搜索代理

**数据库变更**：
- 新增 `app_settings` 表（权限模式等全局设置）
- `model_configs` 表新增 `context_size` 列（可选）

---

## 与 Claude Code 的对比（实现后）

| 能力 | Claude Code | WorkClaw (实现后) | 差异 |
|------|-----------|-------------------|------|
| 核心工具 | 17 个 | 14 个 | 缺少 LS/NotebookEdit/EnterPlanMode |
| Skill 元数据 | 完整 frontmatter | 基础 frontmatter | 缺少 `context: fork`、`memory` 等高级字段 |
| 多 Agent | Subagent + Teams | Subagent（基础） | 无 Agent Teams |
| 权限模型 | 5 种模式 + 沙箱 | 3 种模式 | 无沙箱隔离 |
| 上下文管理 | 自动压缩 | 裁剪 | 无智能压缩 |
| 加密分发 | 无 | 有（.skillpack） | WorkClaw 独有优势 |

实现后覆盖 Claude Code 约 80% 的核心能力，足以运行绝大多数 skill。
