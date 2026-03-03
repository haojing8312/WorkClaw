# Agent 友好输出设计 - 双层 Prompt 架构

## 问题背景

### 当前问题
从用户截图 `temp/82480422f7658b3c46274f308aa85c26.png` 可以看到，WorkClaw Agent 在执行工具时缺少说明性文字：

```
执行工具: write_file
read_file path: L:\站点\README.md
glob pattern: **/*.py
todo_write action: create, subject: 创建 src/calculator.py - 实现加减乘除函数
write_file content: """Simple Calculator Module...
```

**用户体验问题**：
- 工具调用直接显示，缺少上下文说明
- 用户不理解为什么执行这个操作
- 交互感觉生硬、机械

### Claude Code 的成功模式

通过逆向分析 Claude Code（参考 `reference/docs/claude-code-reverse.md`），发现其核心策略：

**System Prompt 引导**：
```markdown
# 示例（来自 system-output-style-explanatory.prompt.md）

Let me start by researching the existing codebase...
I'm going to search for any existing metrics or telemetry code...
I've found some existing telemetry code. Let me mark the first todo as in_progress...
```

**关键发现**：
1. **工具调用前输出自然语言说明**（"Let me...", "I'm going to..."）
2. **解释工具的目的**（为什么要这样做）
3. **通过 system prompt 引导模型行为**（不依赖用户或 Skill 开发者）

## 设计目标

1. **系统级控制**：输出风格由 WorkClaw 系统控制，Skill 开发者无需关心
2. **所有 Skill 受益**：现有和未来的 Skill 自动获得友好输出
3. **可维护性**：系统级 Prompt 统一管理，易于升级优化
4. **参考最佳实践**：借鉴 Claude Code 的成熟 Prompt 工程

## 架构设计

### 1. 双层 Prompt 架构

```
┌─────────────────────────────────────────────────────┐
│              Final System Prompt                    │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌───────────────────────────────────────────┐    │
│  │   系统级 Prompt（WorkClaw 控制）           │    │
│  ├───────────────────────────────────────────┤    │
│  │  1. Agent Workflow（工作流程）            │    │
│  │  2. Output Style（输出风格）              │    │
│  │  3. Tool Usage Policy（工具使用策略）     │    │
│  │  4. Context Management（上下文管理）      │    │
│  └───────────────────────────────────────────┘    │
│                      ↓                              │
│  ┌───────────────────────────────────────────┐    │
│  │   Skill Prompt（开发者提供）               │    │
│  ├───────────────────────────────────────────┤    │
│  │  - 业务逻辑说明                            │    │
│  │  - 领域知识                                │    │
│  │  - 特定指令                                │    │
│  └───────────────────────────────────────────┘    │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### 2. 文件结构

```
apps/runtime/src-tauri/src/
├── agent/
│   ├── executor.rs              # 组合 system prompt
│   ├── system_prompts/          # 新增：系统级 Prompt 模块
│   │   ├── mod.rs               # 导出所有 prompts
│   │   ├── workflow.rs          # Agent 工作流程
│   │   ├── output_style.rs      # 输出风格（工具说明引导）
│   │   ├── tool_policy.rs       # 工具使用策略
│   │   └── context_mgmt.rs      # 上下文管理
│   └── ...
```

### 3. 核心实现

#### 3.1 系统 Prompt 模块（`agent/system_prompts/mod.rs`）

```rust
pub mod workflow;
pub mod output_style;
pub mod tool_policy;
pub mod context_mgmt;

/// 系统级 Prompt 组合器
pub struct SystemPromptBuilder {
    include_workflow: bool,
    include_output_style: bool,
    include_tool_policy: bool,
    include_context_mgmt: bool,
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self {
            include_workflow: true,
            include_output_style: true,
            include_tool_policy: true,
            include_context_mgmt: false, // 可选
        }
    }

    pub fn build(&self, skill_prompt: &str) -> String {
        let mut parts = Vec::new();

        if self.include_workflow {
            parts.push(workflow::AGENT_WORKFLOW_PROMPT);
        }
        if self.include_output_style {
            parts.push(output_style::OUTPUT_STYLE_PROMPT);
        }
        if self.include_tool_policy {
            parts.push(tool_policy::TOOL_USAGE_POLICY);
        }
        if self.include_context_mgmt {
            parts.push(context_mgmt::CONTEXT_MANAGEMENT_PROMPT);
        }

        // Skill prompt 放在最后，优先级最高
        parts.push(skill_prompt);

        parts.join("\n\n---\n\n")
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

#### 3.2 输出风格 Prompt（`agent/system_prompts/output_style.rs`）

```rust
/// 输出风格指导 - 确保 Agent 在工具调用前输出说明
///
/// 参考 Claude Code 的 system-output-style-explanatory.prompt.md
pub const OUTPUT_STYLE_PROMPT: &str = r#"# 输出风格

## 工具调用前的说明

**重要**：在调用任何工具前，请用自然语言简要说明你将要做什么以及为什么。

### 良好示例

✅ "让我先读取配置文件来了解当前设置"
（然后调用 `read_file`）

✅ "我将搜索所有 Python 文件来定位问题"
（然后调用 `glob`）

✅ "现在运行测试来验证修改是否正确"
（然后调用 `bash`）

✅ "我需要查看这个函数的实现细节"
（然后调用 `grep`）

### 避免的模式

❌ 直接调用工具，不输出任何文字
❌ 生硬的 "执行工具 xxx"
❌ 过于详细的技术描述

## 回复风格

1. **简洁清晰**：保持回复简洁（1-2 句话说明工具调用目的）
2. **自然流畅**：像人类专家一样交流，而非机械报告
3. **目的导向**：说明为什么要做，而非机械描述做什么
4. **渐进式输出**：
   - 先说明计划
   - 再调用工具
   - 最后总结结果

## 示例对话

<example>
用户: 帮我找出项目中所有的 TODO 注释

助手: 让我搜索整个代码库来找出所有的 TODO 注释。

[调用 grep 工具搜索 "TODO"]

我找到了 15 处 TODO 注释，分布在以下文件中：
- src/main.rs: 3 处
- src/utils.rs: 7 处
- tests/integration.rs: 5 处

需要我详细列出每一处的内容吗？
</example>

<example>
用户: 修复测试失败的问题

助手: 我先运行测试来看具体是哪里出错了。

[调用 bash 运行测试]

测试失败是因为 `calculate` 函数处理负数时有 bug。让我查看这个函数的实现。

[调用 read_file 读取源文件]

我发现了问题：第 42 行的条件判断有误。现在修复它。

[调用 write_file 修复代码]

修复完成！让我重新运行测试确认。

[调用 bash 运行测试]

测试通过了！✓
</example>
"#;
```

#### 3.3 Agent 工作流程 Prompt（`agent/system_prompts/workflow.rs`）

```rust
/// Agent 核心工作流程指导
///
/// 参考 Claude Code 的核心 workflow prompt
pub const AGENT_WORKFLOW_PROMPT: &str = r#"# Agent 工作流程

你是一个智能 AI 助手，运行在 WorkClaw 平台上。你的目标是帮助用户完成各种任务。

## 核心原则

1. **理解意图**：先理解用户的真实需求，而非字面指令
2. **渐进式执行**：将复杂任务拆解为可管理的步骤
3. **主动验证**：执行关键操作后主动验证结果
4. **清晰沟通**：用自然语言向用户解释你在做什么

## 工作流程

1. **分析任务**：理解用户需求，识别所需工具
2. **说明计划**：告诉用户你将如何处理
3. **执行操作**：调用工具完成任务
4. **验证结果**：确认操作成功
5. **总结汇报**：向用户报告结果

## 错误处理

- 遇到错误时，先尝试理解原因
- 如果可以自动修复，立即执行
- 如果需要用户决策，清晰地说明选项
- 避免重复相同的失败操作
"#;
```

#### 3.4 工具使用策略 Prompt（`agent/system_prompts/tool_policy.rs`）

```rust
/// 工具使用策略
pub const TOOL_USAGE_POLICY: &str = r#"# 工具使用策略

## 工具调用原则

1. **选择合适的工具**：
   - 文件搜索用 `glob`，内容搜索用 `grep`
   - 读取文件用 `read_file`，写入文件用 `write_file`
   - 执行命令用 `bash`

2. **并行调用**：
   - 如果多个工具调用之间没有依赖，可以并行调用
   - 示例：同时读取多个文件

3. **错误恢复**：
   - 工具调用失败时，分析错误原因
   - 尝试替代方案或请求用户帮助

4. **输出截断**：
   - 工具输出可能被截断（超过 30,000 字符）
   - 如遇截断，使用更精确的查询参数
"#;
```

#### 3.5 修改 `executor.rs`

```rust
use super::system_prompts::SystemPromptBuilder;

pub struct AgentExecutor {
    registry: Arc<ToolRegistry>,
    max_iterations: usize,
    system_prompt_builder: SystemPromptBuilder, // 新增
}

impl AgentExecutor {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            max_iterations: 50,
            system_prompt_builder: SystemPromptBuilder::default(),
        }
    }

    pub async fn execute_turn(
        &self,
        api_format: &str,
        base_url: &str,
        api_key: &str,
        model: &str,
        skill_system_prompt: &str,  // 重命名参数，更清晰
        mut messages: Vec<Value>,
        // ... 其他参数
    ) -> Result<Vec<Value>> {
        // 组合系统级 prompt 和 Skill prompt
        let final_system_prompt = self.system_prompt_builder
            .build(skill_system_prompt);

        // 使用组合后的 prompt 调用 LLM
        let response = if api_format == "anthropic" {
            adapters::anthropic::chat_stream_with_tools(
                base_url,
                api_key,
                model,
                &final_system_prompt,  // 使用组合后的 prompt
                trimmed.clone(),
                tools,
                on_token.clone(),
            )
            .await?
        } else {
            adapters::openai::chat_stream_with_tools(
                base_url,
                api_key,
                model,
                &final_system_prompt,
                trimmed.clone(),
                tools,
                on_token.clone(),
            )
            .await?
        };

        // ... 其余逻辑不变
    }
}
```

## 实现步骤

### Phase 1: 基础架构（1-2 小时）

1. ✅ 创建 `agent/system_prompts/` 模块
2. ✅ 实现 `SystemPromptBuilder`
3. ✅ 编写 `output_style.rs`（最关键）
4. ✅ 修改 `executor.rs` 集成新架构

### Phase 2: 完善系统 Prompt（2-3 小时）

5. ✅ 编写 `workflow.rs`
6. ✅ 编写 `tool_policy.rs`
7. ✅ 可选：编写 `context_mgmt.rs`（上下文管理指导）
8. ✅ 测试不同模型（Claude, DeepSeek, Qwen）的遵循程度

### Phase 3: 优化与测试（1-2 小时）

9. ✅ 使用真实 Skill 测试输出效果
10. ✅ 根据测试结果优化 prompt 措辞
11. ✅ 文档化系统 prompt 的设计思路

## 测试计划

### 测试场景

| 场景 | 期望行为 | 验证方式 |
|------|---------|---------|
| 读取文件 | "让我先读取这个文件..." | 检查 stream-token 事件 |
| 搜索代码 | "我将搜索所有 Python 文件..." | 检查工具调用前的文字 |
| 执行命令 | "现在运行测试..." | 检查 bash 调用前的说明 |
| 连续操作 | 每个工具调用前都有说明 | 检查完整对话流程 |
| 并行工具 | 批量说明多个操作 | 检查并行调用场景 |

### 模型兼容性测试

测试不同模型对 system prompt 的遵循程度：

- **Claude Sonnet 3.5+**：预期 95%+ 遵循
- **DeepSeek V3**：预期 80%+ 遵循
- **Qwen 2.5**：预期 70%+ 遵循
- **较弱模型**：如果遵循度低，考虑添加更强的引导

### 验证方法

```typescript
// 在 ChatView.tsx 中验证
useEffect(() => {
  listen<{ token: string }>("stream-token", ({ payload }) => {
    // 验证：工具调用前是否有文字说明
    if (isToolCallAboutToStart) {
      console.log("[验证] 工具调用前的说明:", bufferedText);
    }
  });
}, []);
```

## 收益分析

### 用户体验提升

**Before**（当前）：
```
执行工具: write_file
read_file path: L:\站点\README.md
glob pattern: **/*.py
```

**After**（实施后）：
```
让我先读取 README 文件来了解项目结构。

[read_file 工具卡片]

我看到这是一个 Python 计算器项目。现在搜索所有 Python 文件。

[glob 工具卡片]

找到了 3 个 Python 文件。让我创建计算器模块。

[write_file 工具卡片]
```

### 对比 Claude Code

| 特性 | Claude Code | WorkClaw（实施后） |
|------|-------------|-------------------|
| 工具调用说明 | ✅ | ✅ |
| 自然语言流畅性 | ✅ | ✅ |
| 系统级 prompt 控制 | ✅ | ✅ |
| Skill 开发者无感知 | ✅ | ✅ |
| 可定制化程度 | ⚠️ 无法定制 | ✅ 可配置 |

## 未来扩展

### 1. 可配置的输出风格

允许用户选择输出详细程度：

```rust
pub enum OutputVerbosity {
    Minimal,    // 只在关键操作时说明
    Standard,   // 默认，所有工具调用都说明
    Detailed,   // 详细解释每个步骤
}

impl SystemPromptBuilder {
    pub fn with_verbosity(mut self, verbosity: OutputVerbosity) -> Self {
        // 根据 verbosity 调整 output_style prompt
        self
    }
}
```

### 2. 多语言支持

根据用户语言自动切换 system prompt：

```rust
pub enum PromptLanguage {
    ZhCN,  // 简体中文
    EnUS,  // 英语
}

impl SystemPromptBuilder {
    pub fn with_language(mut self, lang: PromptLanguage) -> Self {
        // 加载对应语言的 prompt
        self
    }
}
```

### 3. A/B 测试框架

测试不同 prompt 版本的效果：

```rust
pub struct PromptExperiment {
    variant_a: String,
    variant_b: String,
    // 收集用户反馈和效果指标
}
```

## 参考资料

1. **Claude Code Reverse**: `reference/docs/claude-code-reverse.md`
   - `results/prompts/system-output-style-explanatory.prompt.md`
   - `results/prompts/system-workflow.prompt.md`

2. **Learn Claude Code**: `reference/docs/learn-claude-code.md`
   - s01: Agent 核心循环模式
   - s03: TodoWrite 任务管理

3. **Anthropic Prompt Engineering**:
   - Chain of Thought Prompting
   - Few-shot Examples

## 总结

**核心价值**：
- ✅ 系统级控制输出风格，Skill 开发者无需关心
- ✅ 所有 Skill 自动获得友好的用户交互体验
- ✅ 易于维护和升级，参考业界最佳实践
- ✅ 清晰的架构分层，职责明确

**实施优先级**：
1. **P0**：`output_style.rs`（立即见效）
2. **P1**：`workflow.rs` 和 `tool_policy.rs`（完善体验）
3. **P2**：可配置化、多语言支持（长期优化）
