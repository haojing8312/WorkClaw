# WorkClaw 全面增强设计文档

**日期**: 2026-02-23
**范围**: Studio 打包修复、Claude Code 兼容 Skill 支持、Runtime UI 打磨、E2E 测试、构建配置

---

## 1. Studio 打包修复与增强（高优先级）

### 现状问题

- `PackForm.tsx` 缺少 frontmatter 字段完整校验
- 打包成功只显示一行提示，无 manifest 预览
- 打包中无进度反馈
- `read_skill_dir` 没有过滤隐藏文件（`.git` 等）
- `pack_skill` 错误信息为原始英文

### 改动清单

| 改动 | 文件 | 说明 |
|------|------|------|
| 前端校验增强 | `PackForm.tsx` | name/username 必填高亮；version semver 校验；description 长度限制 |
| 打包结果预览 | `PackForm.tsx` | 成功后展示 manifest 摘要（id、文件数、加密文件数、大小） |
| 后端过滤 | `commands.rs` | `read_skill_dir` 排除 `.git/`、`node_modules/`、`.DS_Store` |
| 错误信息中文化 | `commands.rs` | `pack_skill` 中 map_err 补充中文上下文 |
| 打包状态细化 | `PackForm.tsx` + Tauri 事件 | "校验中…" → "加密中…" → "写入中…" 三阶段 |

---

## 2. Claude Code 兼容 Skill 支持（高优先级）

### 设计目标

凡是 Claude Code 支持的 Skill 格式，WorkClaw Runtime 都能直接导入运行。

### Skill 目录格式

与 Claude Code 完全一致：

```
my-skill/
├── SKILL.md              # 必需：YAML frontmatter + Markdown 指令
├── references/           # 可选：参考文档
│   └── *.md
├── templates/            # 可选：模板文件
│   └── *.md
├── examples/             # 可选：示例
│   └── *.md
├── scripts/              # 可选：可执行脚本
│   └── *.py / *.sh
└── assets/               # 可选：静态资源
    └── *
```

### Frontmatter 字段支持

| 字段 | 类型 | 处理方式 |
|------|------|----------|
| `name` | string | Skill 标识名，斜杠命令名 |
| `description` | string | 展示 + LLM 自动匹配 |
| `argument-hint` | string | `/` 自动补全时显示 |
| `disable-model-invocation` | bool | 仅用户可手动触发 |
| `user-invocable` | bool | 是否在 `/` 菜单中显示 |
| `allowed-tools` | string | 白名单工具列表 |
| `model` | string | 覆盖当前模型 |
| `context` | string | `fork` = 隔离子 Agent |
| `agent` | string | 子 Agent 类型 |

### 字符串替换

支持以下变量：
- `$ARGUMENTS` — 全部参数
- `$ARGUMENTS[N]` — 第 N 个参数（0-indexed）
- `$N` — `$ARGUMENTS[N]` 的简写
- `${CLAUDE_SESSION_ID}` — 当前会话 ID

### 数据库变更

```sql
ALTER TABLE installed_skills ADD COLUMN source_type TEXT DEFAULT 'encrypted';
-- 'encrypted' = .skillpack, 'local' = 本地目录
```

### 新增 Rust 结构体

```rust
// apps/runtime/src-tauri/src/types.rs (新文件或在现有位置)
pub struct LocalSkillMeta {
    pub disable_model_invocation: bool,
    pub user_invocable: bool,
    pub allowed_tools: Vec<String>,
    pub context: Option<String>,       // "fork" 或 None
    pub agent: Option<String>,
    pub argument_hint: Option<String>,
    pub model_override: Option<String>,
}
```

### 后端新增命令

1. **`import_local_skill(dir_path)`** — 选择目录，解析 SKILL.md frontmatter，注册到 DB
2. **`refresh_local_skill(skill_id)`** — 重新读取本地目录
3. **Skill 内容加载逻辑** — `send_message` 根据 `source_type` 选择读取方式：
   - `local`: 实时读取目录中所有 `.md` 文件
   - `encrypted`: 现有解密流程

### 前端变更

1. `InstallDialog.tsx` 增加「导入本地 Skill」按钮
2. `Sidebar.tsx` 本地 Skill 显示 `[本地]` 标签
3. 聊天输入框支持 `/` 前缀触发 Skill 列表（仅 `user_invocable=true`）
4. 传入 arguments 支持（`/skill-name arg1 arg2`）

### 暂不实现

- `` !`command` `` 动态命令执行语法
- 多级作用域发现（enterprise > personal > project）
- hooks 生命周期钩子
- `.claude/commands/` 向后兼容

---

## 3. MCP 服务器管理 UI（中优先级）

### 现状

后端已有完整 CRUD 命令：`add_mcp_server`、`list_mcp_servers`、`remove_mcp_server`。DB 表 `mcp_servers` 已就绪。缺前端 UI。

### UI 设计

在 `SettingsView.tsx` 新增 Tab「MCP 服务器」：

```
┌──────────────────────────────────────┐
│  设置                                │
│  [模型配置]  [MCP 服务器]            │
├──────────────────────────────────────┤
│  + 添加 MCP 服务器                   │
│                                      │
│  ┌──────────────────────────────┐    │
│  │ filesystem                    │    │
│  │ 命令: npx @mcp/filesystem     │    │
│  │ 状态: ● 已连接                │    │
│  │               [删除]          │    │
│  └──────────────────────────────┘    │
│                                      │
│  添加表单（展开时显示）：             │
│  名称: [________]                    │
│  命令: [________]                    │
│  参数: [________] (JSON 数组)         │
│  环境变量: [________] (JSON 对象)     │
│  [添加]                              │
└──────────────────────────────────────┘
```

### 文件变更

- 新增: `apps/runtime/src/components/McpServerPanel.tsx` (~150 行)
- 改动: `SettingsView.tsx` 增加 Tab 切换逻辑

---

## 4. 会话搜索/导出（中优先级）

### 搜索

- Sidebar 会话列表顶部增加搜索框
- 后端新增 `search_sessions` 命令
- SQL `LIKE` 匹配 `sessions.title` 和 `messages.content`
- 前端 300ms debounce 防抖

### 导出

- 会话操作增加「导出为 Markdown」
- 后端新增 `export_session` 命令，返回格式化 Markdown 字符串
- 前端通过 `dialog.save()` 选择路径，`fs.writeTextFile()` 写入

---

## 5. Markdown 代码高亮（中优先级）

### 方案

集成 `react-syntax-highlighter`（Prism 引擎 + oneDark 主题）。

```tsx
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";

// ChatView.tsx 中 ReactMarkdown components prop
components={{
  code({ className, children, ...props }) {
    const match = /language-(\w+)/.exec(className || "");
    return match ? (
      <SyntaxHighlighter style={oneDark} language={match[1]}>
        {String(children).replace(/\n$/, "")}
      </SyntaxHighlighter>
    ) : (
      <code className={className} {...props}>{children}</code>
    );
  },
}}
```

改动: `ChatView.tsx` ~20 行，新增 npm 依赖 `react-syntax-highlighter`。

---

## 6. 响应式布局（中优先级）

### 方案

- Sidebar 可折叠：窄屏（< 768px）默认折叠，仅显示图标
- 折叠按钮切换显示/隐藏
- 消息气泡 `max-w-2xl` → `max-w-[80%]`
- 输入框移动端占满宽度

改动: `Sidebar.tsx`、`App.tsx`、`ChatView.tsx`，约 50 行。

---

## 7. 端到端集成测试（中优先级）

### 测试架构

```
apps/runtime/src-tauri/tests/
├── test_e2e_flow.rs          # 完整生命周期测试
├── helpers/
│   └── mod.rs                # 共享测试工具
└── fixtures/
    └── test-skill/           # 测试用 Skill 目录
        ├── SKILL.md
        └── templates/
            └── greeting.md
```

### Mock LLM 适配器

```rust
pub struct MockLlmAdapter {
    responses: Vec<String>,  // 预设回复队列
}
```

不调用真实 API，返回预定义响应（支持 tool_use 格式）。

### 测试场景

| 测试 | 覆盖链路 |
|------|---------|
| `test_install_and_chat` | 安装 .skillpack → 创建会话 → 发消息 → 收到回复 |
| `test_import_local_skill` | 导入本地目录 → 验证注册 → 发消息 |
| `test_agent_tool_execution` | 发消息 → Agent 调用 ReadFile → 返回结果 |
| `test_session_lifecycle` | 创建 → 发多条消息 → 删除会话 |
| `test_mcp_server_crud` | 添加 → 列表 → 删除 MCP 服务器 |

### 测试辅助函数

```rust
pub async fn setup_test_db() -> SqlitePool { ... }
pub fn create_test_skillpack(dir: &Path) -> PathBuf { ... }
pub fn mock_adapter(responses: Vec<&str>) -> MockLlmAdapter { ... }
```

---

## 8. 基础构建配置（低优先级）

| 项目 | 检查/修复内容 |
|------|-------------|
| `tauri.conf.json` bundle | 确认 NSIS installer 配置（图标、产品名、版本号） |
| Sidecar 打包 | 确认 `externalBin` 配置，Node.js sidecar 被包含 |
| 图标文件 | 检查 `icons/` 目录格式（.ico, .png） |
| 签名配置 | 暂不配置，预留位置 |

---

## 实施优先级

```
Phase 1 (高优先级)
├── 1. Studio 打包修复
└── 2. Claude Code 兼容 Skill 支持

Phase 2 (中优先级)
├── 3. MCP 服务器管理 UI
├── 4. 会话搜索/导出
├── 5. Markdown 代码高亮
├── 6. 响应式布局
└── 7. E2E 集成测试

Phase 3 (低优先级)
└── 8. 基础构建配置
```
