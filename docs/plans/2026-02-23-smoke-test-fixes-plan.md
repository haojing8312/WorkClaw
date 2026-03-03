# Smoke Test 修复与增强 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 Sidebar 折叠 bug、安装后自动切换、工作目录沙箱、MCP 预设与依赖检查、内置通用 Skill。

**Architecture:** Sidebar 改为折叠/展开双模式渲染；每个会话绑定 work_dir，Agent 工具通过 ToolContext 接收并校验路径前缀；内置 Skill 在 init_db 时自动插入；MCP 依赖声明在 SKILL.md frontmatter 中，导入时对比 DB 已有配置。

**Tech Stack:** Rust (Tauri 2, sqlx, serde), TypeScript (React 18, Tailwind CSS), SQLite

---

### Task 1: Sidebar 折叠 — 窄侧边栏替代完全隐藏

**Files:**
- Modify: `apps/runtime/src/components/Sidebar.tsx`
- Modify: `apps/runtime/src/App.tsx`

**Step 1: 修改 Sidebar 组件，新增 collapsed prop，折叠时渲染窄版布局**

修改 `apps/runtime/src/components/Sidebar.tsx`：

Props 接口新增 `collapsed: boolean`：

```typescript
interface Props {
  skills: SkillManifest[];
  selectedSkillId: string | null;
  onSelectSkill: (id: string) => void;
  sessions: SessionInfo[];
  selectedSessionId: string | null;
  onSelectSession: (id: string) => void;
  onNewSession: () => void;
  onDeleteSession: (id: string) => void;
  onInstall: () => void;
  onSettings: () => void;
  onSearchSessions: (query: string) => void;
  onExportSession: (sessionId: string) => void;
  onCollapse: () => void;
  collapsed: boolean;  // 新增
}
```

组件函数接收 `collapsed`，顶部判断：

```typescript
export function Sidebar({
  skills,
  selectedSkillId,
  onSelectSkill,
  sessions,
  selectedSessionId,
  onSelectSession,
  onNewSession,
  onDeleteSession,
  onInstall,
  onSettings,
  onSearchSessions,
  onExportSession,
  onCollapse,
  collapsed,
}: Props) {
  const [searchQuery, setSearchQuery] = useState("");

  function handleSearchChange(value: string) {
    setSearchQuery(value);
    onSearchSessions(value);
  }

  // 折叠模式：窄侧边栏
  if (collapsed) {
    return (
      <div className="w-12 bg-slate-800 flex flex-col h-full border-r border-slate-700 items-center py-3 gap-3">
        <button
          onClick={onCollapse}
          className="text-slate-400 hover:text-slate-200 text-sm transition-colors"
          title="展开侧边栏"
        >
          ▶
        </button>
        <div className="flex-1" />
        <button
          onClick={onInstall}
          className="text-blue-400 hover:text-blue-300 text-lg transition-colors"
          title="安装 Skill"
        >
          +
        </button>
        <button
          onClick={onSettings}
          className="text-slate-400 hover:text-slate-200 text-sm transition-colors"
          title="设置"
        >
          ⚙
        </button>
      </div>
    );
  }

  // 展开模式：原有代码不变
  return (
    // ... 现有展开模式代码 ...
  );
}
```

**Step 2: 修改 App.tsx，移除 absolute 定位的 ☰ 按钮，始终渲染 Sidebar**

修改 `apps/runtime/src/App.tsx`：

移除 `{sidebarCollapsed && (<button ... ☰ ...>)}` 和 `{!sidebarCollapsed && (<Sidebar .../>)}`，改为始终渲染 Sidebar 并传递 `collapsed` prop：

```typescript
return (
  <div className="flex h-screen bg-slate-900 text-slate-100 overflow-hidden">
    <Sidebar
      skills={skills}
      selectedSkillId={selectedSkillId}
      onSelectSkill={setSelectedSkillId}
      sessions={sessions}
      selectedSessionId={selectedSessionId}
      onSelectSession={setSelectedSessionId}
      onNewSession={handleCreateSession}
      onDeleteSession={handleDeleteSession}
      onInstall={() => setShowInstall(true)}
      onSettings={() => setShowSettings(true)}
      onSearchSessions={handleSearchSessions}
      onExportSession={handleExportSession}
      onCollapse={() => setSidebarCollapsed(!sidebarCollapsed)}
      collapsed={sidebarCollapsed}
    />
    <div className="flex-1 overflow-hidden">
      {/* ... 主区域内容不变 ... */}
    </div>
    {showInstall && (
      <InstallDialog onInstalled={handleInstalled} onClose={() => setShowInstall(false)} />
    )}
  </div>
);
```

**Step 3: 运行验证**

Run: `cd apps/runtime && pnpm build`
Expected: 编译通过无错误

**Step 4: Commit**

```bash
git add apps/runtime/src/components/Sidebar.tsx apps/runtime/src/App.tsx
git commit -m "fix(ui): Sidebar 折叠改为窄侧边栏，不再遮挡内容"
```

---

### Task 2: 安装 Skill 后自动切换 + 创建新会话

**Files:**
- Modify: `apps/runtime/src/components/InstallDialog.tsx`
- Modify: `apps/runtime/src/App.tsx`

**Step 1: 修改 InstallDialog，onInstalled 传递 skillId**

修改 `apps/runtime/src/components/InstallDialog.tsx`：

Props 接口改为：

```typescript
interface Props {
  onInstalled: (skillId: string) => void;  // 改为接收 skillId
  onClose: () => void;
}
```

`handleInstall` 函数中，安装成功后传递 skill ID：

```typescript
async function handleInstall() {
  setError("");
  setLoading(true);

  try {
    if (mode === "skillpack") {
      if (!packPath || !username.trim()) {
        setError("请选择文件并填写用户名");
        setLoading(false);
        return;
      }
      const manifest = await invoke<{ id: string }>("install_skill", { packPath, username });
      onInstalled(manifest.id);
    } else {
      if (!localDir) {
        setError("请选择包含 SKILL.md 的目录");
        setLoading(false);
        return;
      }
      const result = await invoke<{ manifest: { id: string }; missing_mcp: string[] }>("import_local_skill", { dirPath: localDir });
      // 如果有缺失 MCP，显示警告但仍完成安装
      if (result.missing_mcp.length > 0) {
        setMcpWarning(result.missing_mcp);
      }
      onInstalled(result.manifest.id);
    }
    onClose();
  } catch (e: unknown) {
    setError(String(e));
  } finally {
    setLoading(false);
  }
}
```

新增 MCP 缺失警告状态和展示（在 error 显示区域附近）：

```typescript
const [mcpWarning, setMcpWarning] = useState<string[]>([]);

// JSX 中 error 下方：
{mcpWarning.length > 0 && (
  <div className="text-amber-400 text-sm">
    此 Skill 需要以下 MCP 服务器，请在设置中配置：
    <ul className="list-disc list-inside mt-1">
      {mcpWarning.map((name) => (
        <li key={name} className="text-xs">{name}</li>
      ))}
    </ul>
  </div>
)}
```

**Step 2: 修改 App.tsx，新增 handleInstalled 自动切换逻辑**

修改 `apps/runtime/src/App.tsx`：

新增 `handleInstalled` 函数：

```typescript
async function handleInstalled(skillId: string) {
  await loadSkills();
  setSelectedSkillId(skillId);
  // 自动创建新会话
  const modelId = models[0]?.id;
  if (modelId) {
    try {
      const id = await invoke<string>("create_session", {
        skillId,
        modelId,
      });
      // 重新加载会话列表（useEffect 中 selectedSkillId 变化会触发，但此时还没更新完，手动加载）
      const sessions = await invoke<SessionInfo[]>("get_sessions", { skillId });
      setSessions(sessions);
      setSelectedSessionId(id);
    } catch (e) {
      console.error("自动创建会话失败:", e);
    }
  }
}
```

InstallDialog 调用处改为：

```typescript
{showInstall && (
  <InstallDialog onInstalled={handleInstalled} onClose={() => setShowInstall(false)} />
)}
```

**Step 3: 运行验证**

Run: `cd apps/runtime && pnpm build`
Expected: 编译通过无错误

**Step 4: Commit**

```bash
git add apps/runtime/src/components/InstallDialog.tsx apps/runtime/src/App.tsx
git commit -m "feat(ui): 安装 Skill 后自动切换并创建新会话"
```

---

### Task 3: 内置通用 Skill

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src/components/Sidebar.tsx`

**Step 1: 在 init_db 中自动插入内置通用 Skill**

修改 `apps/runtime/src-tauri/src/db.rs`，在 `init_db` 函数的最后（所有 migration 之后）添加：

```rust
// 内置通用 Skill：始终存在，无需用户安装
let builtin_manifest = serde_json::json!({
    "id": "builtin-general",
    "name": "通用助手",
    "description": "通用 AI 助手，可以读写文件、执行命令、搜索代码、搜索网页",
    "version": "1.0.0",
    "author": "WorkClaw",
    "recommended_model": "",
    "tags": [],
    "created_at": "2026-01-01T00:00:00Z",
    "username_hint": null,
    "encrypted_verify": ""
});
let builtin_json = builtin_manifest.to_string();
let now = chrono::Utc::now().to_rfc3339();
let _ = sqlx::query(
    "INSERT OR IGNORE INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type) VALUES ('builtin-general', ?, ?, '', '', 'builtin')"
)
.bind(&builtin_json)
.bind(&now)
.execute(&pool)
.await;
```

需要在文件顶部确认有 `use chrono` 或内联获取时间。检查 Cargo.toml 是否已有 chrono 依赖（根据 chat.rs 中 `use chrono::Utc;` 判断已有）。

**Step 2: send_message 中处理 source_type = "builtin"**

修改 `apps/runtime/src-tauri/src/commands/chat.rs`，在 `send_message` 函数中读取 `raw_prompt` 的分支逻辑：

将：
```rust
let raw_prompt = if source_type == "local" {
```

改为：
```rust
let raw_prompt = if source_type == "builtin" {
    // 内置 Skill：使用硬编码的 system prompt
    "你是一个通用 AI 助手。你可以：\n\
    - 读取和编写文件\n\
    - 在终端中执行命令\n\
    - 搜索文件和代码\n\
    - 搜索网页获取信息\n\
    - 管理记忆和上下文\n\n\
    请根据用户的需求，自主分析、规划和执行任务。\n\
    工作目录为用户指定的目录，所有文件操作限制在该目录范围内。".to_string()
} else if source_type == "local" {
```

**Step 3: Sidebar 中内置 Skill 显示 [内置] 标签，排序置顶**

修改 `apps/runtime/src/components/Sidebar.tsx`，在 Skill 名称旁边添加内置标签：

```typescript
{s.id === "builtin-general" && (
  <span className="text-[10px] bg-blue-800/60 text-blue-300 px-1 py-0.5 rounded">
    内置
  </span>
)}
```

置顶逻辑：在渲染前排序，内置 Skill 排在最前面：

```typescript
const sortedSkills = [...skills].sort((a, b) => {
  if (a.id === "builtin-general") return -1;
  if (b.id === "builtin-general") return 1;
  return 0;
});
```

然后用 `sortedSkills.map(...)` 替代 `skills.map(...)`。

**Step 4: 运行验证**

Run: `cd apps/runtime/src-tauri && cargo check`
Run: `cd apps/runtime && pnpm build`
Expected: 两者均通过

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src/components/Sidebar.tsx
git commit -m "feat: 内置通用助手 Skill，无需安装即可使用"
```

---

### Task 4: 数据库 — sessions 表新增 work_dir 列

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Test: `apps/runtime/src-tauri/tests/test_e2e_flow.rs`

**Step 1: 新增 migration**

修改 `apps/runtime/src-tauri/src/db.rs`，在现有 migration 之后添加：

```rust
// Migration: add work_dir column to sessions（每会话独立工作目录）
let _ = sqlx::query("ALTER TABLE sessions ADD COLUMN work_dir TEXT NOT NULL DEFAULT ''")
    .execute(&pool)
    .await;
```

**Step 2: 更新测试 helpers 中的 schema**

修改 `apps/runtime/src-tauri/tests/helpers/mod.rs`，`setup_test_db` 中 sessions 表的 CREATE TABLE 语句新增 `work_dir`：

```sql
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL,
    title TEXT,
    created_at TEXT NOT NULL,
    model_id TEXT NOT NULL,
    permission_mode TEXT NOT NULL DEFAULT 'default',
    work_dir TEXT NOT NULL DEFAULT ''
)
```

**Step 3: 运行测试**

Run: `cd apps/runtime/src-tauri && cargo test --test test_e2e_flow`
Expected: 所有测试通过

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/tests/helpers/mod.rs
git commit -m "feat(db): sessions 表新增 work_dir 列"
```

---

### Task 5: ToolContext — 工具层路径沙箱

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/types.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/read_file.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/write_file.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/glob_tool.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/grep_tool.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/bash.rs`
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`

**Step 1: 定义 ToolContext 结构体并修改 Tool trait**

修改 `apps/runtime/src-tauri/src/agent/types.rs`：

```rust
use std::path::PathBuf;

/// 工具执行上下文
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    /// 工作目录路径，如有值则所有文件操作限制在此目录下
    pub work_dir: Option<PathBuf>,
}

impl ToolContext {
    /// 检查路径是否在工作目录范围内
    pub fn check_path(&self, path: &str) -> anyhow::Result<PathBuf> {
        let target = std::path::Path::new(path);
        let canonical = if target.is_absolute() {
            target.to_path_buf()
        } else if let Some(ref wd) = self.work_dir {
            wd.join(target)
        } else {
            std::env::current_dir()?.join(target)
        };

        // 如果设置了工作目录，检查路径前缀
        if let Some(ref wd) = self.work_dir {
            // 使用 canonicalize 处理 .. 和符号链接
            // 注意：目标文件可能尚不存在（WriteFile），所以先检查父目录
            let check_path = if canonical.exists() {
                canonical.canonicalize()?
            } else if let Some(parent) = canonical.parent() {
                if parent.exists() {
                    parent.canonicalize()?.join(canonical.file_name().unwrap_or_default())
                } else {
                    canonical.clone()
                }
            } else {
                canonical.clone()
            };

            let wd_canonical = wd.canonicalize().unwrap_or_else(|_| wd.clone());
            if !check_path.starts_with(&wd_canonical) {
                anyhow::bail!(
                    "路径 {} 不在工作目录 {} 范围内",
                    path,
                    wd.display()
                );
            }
        }
        Ok(canonical)
    }
}
```

修改 `Tool` trait 的 `execute` 方法签名：

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<String>;
}
```

**Step 2: 更新所有工具实现**

每个工具的 `execute` 方法签名加上 `ctx: &ToolContext` 参数。

**ReadFile** (`read_file.rs`):
- 签名加 `ctx: &ToolContext`
- 在读取前调用 `let path = ctx.check_path(path_str)?;`

**WriteFile** (`write_file.rs`):
- 同上，在写入前调用 `ctx.check_path(path_str)?;`

**GlobTool** (`glob_tool.rs`):
- 签名加 `ctx: &ToolContext`
- 如果有 `work_dir`，将 glob 的根目录限制为 `work_dir`

**GrepTool** (`grep_tool.rs`):
- 签名加 `ctx: &ToolContext`
- 如果有 `work_dir`，将搜索目录限制为 `work_dir`

**Bash** (`bash.rs`):
- 签名加 `ctx: &ToolContext`
- 如果有 `work_dir`，将命令的 `current_dir` 设为 `work_dir`

**其他工具**（CompactTool, TaskTool, MemoryTool, WebSearchTool, AskUserTool）：
- 签名加 `ctx: &ToolContext` 但内部不使用（这些工具不涉及文件操作）

**Step 3: 更新 AgentExecutor 中的工具调用**

修改 `apps/runtime/src-tauri/src/agent/executor.rs`：

`AgentExecutor` 新增 `tool_context` 字段（或在 `execute_turn` 中接收 `ToolContext` 参数），传递给 `tool.execute(input, &ctx)`。

在 `execute_turn` 方法签名新增 `work_dir: Option<String>` 参数：

```rust
pub async fn execute_turn(
    &self,
    // ... 现有参数 ...
    work_dir: Option<String>,  // 新增
) -> anyhow::Result<Vec<Value>> {
    let tool_ctx = ToolContext {
        work_dir: work_dir.map(PathBuf::from),
    };
    // ... 在调用 tool.execute 时传入 &tool_ctx ...
}
```

**Step 4: 运行验证**

Run: `cd apps/runtime/src-tauri && cargo check`
Expected: 编译通过

Run: `cd apps/runtime/src-tauri && cargo test`
Expected: 现有测试通过（测试中 work_dir 为 None，不影响现有行为）

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/
git commit -m "feat(agent): ToolContext 路径沙箱，工具执行限制在工作目录内"
```

---

### Task 6: 前端 — 新建会话时选择工作目录

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: create_session 后端接收 work_dir 参数**

修改 `apps/runtime/src-tauri/src/commands/chat.rs`，`create_session` 新增 `work_dir` 参数：

```rust
#[tauri::command]
pub async fn create_session(
    skill_id: String,
    model_id: String,
    work_dir: String,  // 新增
    db: State<'_, DbState>,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, work_dir) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&session_id)
    .bind(&skill_id)
    .bind("New Chat")
    .bind(&now)
    .bind(&model_id)
    .bind(&work_dir)
    .execute(&db.0)
    .await
    .map_err(|e| e.to_string())?;
    Ok(session_id)
}
```

**Step 2: send_message 读取 work_dir 并传递给 AgentExecutor**

修改 `send_message` 中加载会话信息的 SQL：

```rust
let (skill_id, model_id, perm_str, work_dir) = sqlx::query_as::<_, (String, String, String, String)>(
    "SELECT skill_id, model_id, permission_mode, COALESCE(work_dir, '') FROM sessions WHERE id = ?"
)
.bind(&session_id)
.fetch_one(&db.0)
.await
.map_err(|e| format!("会话不存在 (session_id={session_id}): {e}"))?;
```

在调用 `execute_turn` 时传递 `work_dir`：

```rust
let work_dir_opt = if work_dir.is_empty() { None } else { Some(work_dir.clone()) };

let final_messages = agent_executor
    .execute_turn(
        // ... 现有参数 ...
        work_dir_opt,  // 新增参数
    )
    .await
    .map_err(|e| e.to_string())?;
```

同时在 system_prompt 中注入工作目录信息：

```rust
let system_prompt = if work_dir.is_empty() {
    format!(
        "{}\n\n---\n运行环境:\n- 可用工具: {}\n- 模型: {}\n- 最大迭代次数: {}",
        skill_config.system_prompt, tool_names, model_name, max_iter,
    )
} else {
    format!(
        "{}\n\n---\n运行环境:\n- 工作目录: {}\n- 可用工具: {}\n- 模型: {}\n- 最大迭代次数: {}\n\n注意: 所有文件操作必须限制在工作目录范围内。",
        skill_config.system_prompt, work_dir, tool_names, model_name, max_iter,
    )
};
```

**Step 3: 前端 — 新建会话前弹出目录选择器**

修改 `apps/runtime/src/App.tsx`，`handleCreateSession` 改为先选目录：

```typescript
async function handleCreateSession() {
  const modelId = models[0]?.id;
  if (!selectedSkillId || !modelId) return;

  // 弹出目录选择器
  const dir = await open({ directory: true, title: "选择工作目录" });
  if (!dir || typeof dir !== "string") return;  // 用户取消

  try {
    const id = await invoke<string>("create_session", {
      skillId: selectedSkillId,
      modelId,
      workDir: dir,
    });
    setSelectedSessionId(id);
    if (selectedSkillId) await loadSessions(selectedSkillId);
  } catch (e) {
    console.error("创建会话失败:", e);
  }
}
```

同步更新 `handleInstalled` 中的 `create_session` 调用也加上 `workDir` 参数。

**Step 4: ChatView 顶部显示工作目录**

修改 `apps/runtime/src/components/ChatView.tsx`，Props 新增 `workDir`：

```typescript
interface Props {
  skill: SkillManifest;
  models: ModelConfig[];
  sessionId: string;
  workDir?: string;  // 新增
  onSessionUpdate?: () => void;
}
```

在头部栏中显示：

```typescript
<div className="flex items-center justify-between px-6 py-3 border-b border-slate-700 bg-slate-800">
  <div>
    <span className="font-medium">{skill.name}</span>
    <span className="text-xs text-slate-400 ml-2">v{skill.version}</span>
    {workDir && (
      <span className="text-xs text-slate-500 ml-3" title={workDir}>
        📁 {workDir.split(/[/\\]/).pop()}
      </span>
    )}
  </div>
  {currentModel && (
    <span className="text-xs text-slate-400">{currentModel.name}</span>
  )}
</div>
```

App.tsx 中需要传递 workDir。先从 get_sessions 返回的数据中获取，或新增一个 state 和加载逻辑。简化方案：在 `sessions` 数据中已包含 `work_dir`，或者新增一个 `currentWorkDir` state，在选中 session 时加载。

在 `get_sessions` 后端中补充返回 `work_dir`：

```rust
// commands/chat.rs get_sessions
let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
    "SELECT id, title, created_at, model_id, COALESCE(work_dir, '') FROM sessions WHERE skill_id = ? ORDER BY created_at DESC"
)
```

返回 JSON 中加入 `work_dir` 字段。

前端 `SessionInfo` 类型新增 `work_dir?: string`，App.tsx 中选中 session 时提取 workDir 传给 ChatView。

**Step 5: 运行验证**

Run: `cd apps/runtime/src-tauri && cargo check`
Run: `cd apps/runtime && pnpm build`
Expected: 均通过

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src/App.tsx apps/runtime/src/components/ChatView.tsx apps/runtime/src/types.ts
git commit -m "feat: 每会话工作目录选择 + 路径沙箱注入"
```

---

### Task 7: MCP 常用预设

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`

**Step 1: 新增 MCP 预设数组和下拉框**

修改 `apps/runtime/src/components/SettingsView.tsx`，在 MCP 相关代码区域新增：

```typescript
const MCP_PRESETS = [
  { label: "— 快速选择 —", value: "", name: "", command: "", args: "", env: "" },
  { label: "Filesystem", value: "filesystem", name: "filesystem", command: "npx", args: "-y @anthropic/mcp-server-filesystem /tmp", env: "" },
  { label: "Brave Search", value: "brave-search", name: "brave-search", command: "npx", args: "-y @anthropic/mcp-server-brave-search", env: '{"BRAVE_API_KEY": ""}' },
  { label: "Memory", value: "memory", name: "memory", command: "npx", args: "-y @anthropic/mcp-server-memory", env: "" },
  { label: "Puppeteer", value: "puppeteer", name: "puppeteer", command: "npx", args: "-y @anthropic/mcp-server-puppeteer", env: "" },
  { label: "Fetch", value: "fetch", name: "fetch", command: "npx", args: "-y @anthropic/mcp-server-fetch", env: "" },
];

function applyMcpPreset(value: string) {
  const preset = MCP_PRESETS.find((p) => p.value === value);
  if (!preset || !preset.value) return;
  setMcpForm({
    name: preset.name,
    command: preset.command,
    args: preset.args,
    env: preset.env,
  });
}
```

在 MCP 表单的 `名称` 输入框上方添加下拉框：

```typescript
<div>
  <label className={labelCls}>快速选择 MCP 服务器</label>
  <select
    className={inputCls}
    defaultValue=""
    onChange={(e) => applyMcpPreset(e.target.value)}
  >
    {MCP_PRESETS.map((p) => (
      <option key={p.value} value={p.value}>{p.label}</option>
    ))}
  </select>
</div>
```

**Step 2: 运行验证**

Run: `cd apps/runtime && pnpm build`
Expected: 编译通过

**Step 3: Commit**

```bash
git add apps/runtime/src/components/SettingsView.tsx
git commit -m "feat(ui): MCP 服务器常用预设快速选择"
```

---

### Task 8: SKILL.md MCP 依赖声明 + 导入时检查

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/skill_config.rs`
- Modify: `apps/runtime/src-tauri/src/commands/skills.rs`
- Test: `apps/runtime/src-tauri/tests/test_e2e_flow.rs`

**Step 1: skill_config.rs 新增 McpServerDep 和 mcp_servers 字段**

修改 `apps/runtime/src-tauri/src/agent/skill_config.rs`：

新增结构体：

```rust
/// SKILL.md 中声明的 MCP 服务器依赖
#[derive(Deserialize, Debug, Clone, serde::Serialize)]
pub struct McpServerDep {
    pub name: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    /// 需要的环境变量名称列表
    #[serde(default)]
    pub env: Option<Vec<String>>,
}
```

`SkillConfig` 新增字段：

```rust
pub struct SkillConfig {
    // ... 现有字段 ...
    /// Skill 声明的 MCP 服务器依赖
    pub mcp_servers: Vec<McpServerDep>,
}
```

`FrontMatter` 新增字段：

```rust
struct FrontMatter {
    // ... 现有字段 ...
    #[serde(alias = "mcp-servers", default)]
    mcp_servers: Vec<McpServerDep>,
}
```

`SkillConfig::parse` 中赋值：

```rust
Self {
    // ... 现有字段 ...
    mcp_servers: fm.mcp_servers,
}
```

`Default` 实现中初始化为空 Vec。

**Step 2: import_local_skill 返回 ImportResult 含 missing_mcp**

修改 `apps/runtime/src-tauri/src/commands/skills.rs`：

新增返回结构体：

```rust
#[derive(serde::Serialize)]
pub struct ImportResult {
    pub manifest: skillpack_rs::SkillManifest,
    pub missing_mcp: Vec<String>,
}
```

修改 `import_local_skill` 返回类型为 `Result<ImportResult, String>`：

在保存到 DB 后，检查 MCP 依赖：

```rust
// 检查 MCP 依赖
let mut missing_mcp = Vec::new();
for dep in &config.mcp_servers {
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM mcp_servers WHERE name = ?"
    )
    .bind(&dep.name)
    .fetch_optional(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    if exists.is_none() {
        missing_mcp.push(dep.name.clone());
    }
}

Ok(ImportResult { manifest, missing_mcp })
```

**Step 3: 新增测试**

修改 `apps/runtime/src-tauri/tests/test_e2e_flow.rs`，新增测试：

```rust
#[tokio::test]
async fn test_skill_config_mcp_dependency() {
    let content = r#"---
name: test-mcp-skill
description: Test MCP dependency
mcp-servers:
  - name: brave-search
    command: npx
    args: ["@anthropic/mcp-server-brave-search"]
    env: ["BRAVE_API_KEY"]
  - name: memory
---
Test skill with MCP dependencies."#;

    let config = runtime_lib::agent::skill_config::SkillConfig::parse(content);
    assert_eq!(config.mcp_servers.len(), 2);
    assert_eq!(config.mcp_servers[0].name, "brave-search");
    assert_eq!(config.mcp_servers[0].env, Some(vec!["BRAVE_API_KEY".to_string()]));
    assert_eq!(config.mcp_servers[1].name, "memory");
}
```

**Step 4: 运行测试**

Run: `cd apps/runtime/src-tauri && cargo test --test test_e2e_flow test_skill_config_mcp_dependency`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/skill_config.rs apps/runtime/src-tauri/src/commands/skills.rs apps/runtime/src-tauri/tests/test_e2e_flow.rs
git commit -m "feat: SKILL.md MCP 依赖声明 + 导入时检查缺失"
```

---

### Task 9: 集成验证 — 全链路 smoke test

**Step 1: 运行全部 Rust 测试**

Run: `cd apps/runtime/src-tauri && cargo test`
Expected: 所有测试通过（test_task_tool 已知 DLL 问题除外）

**Step 2: 运行前端构建**

Run: `cd apps/runtime && pnpm build`
Expected: 编译通过无错误

**Step 3: 启动开发模式验证**

Run: `pnpm runtime`（后台）
Expected: 应用正常启动

验证清单：
- [ ] Sidebar 折叠后显示窄侧边栏（3个图标按钮），不遮挡主区域
- [ ] 安装/导入 Skill 后自动切换到新会话
- [ ] 首次启动看到「通用助手 [内置]」Skill
- [ ] 新建会话时弹出目录选择器
- [ ] ChatView 顶部显示工作目录名称
- [ ] MCP 服务器快速选择预设可用
- [ ] 导入含 mcp-servers 的 Skill 时显示缺失警告

**Step 4: Commit（如有任何修复）**

```bash
git add -A
git commit -m "fix: 集成验证修复"
```
