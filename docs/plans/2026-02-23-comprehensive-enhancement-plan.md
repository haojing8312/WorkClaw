# WorkClaw 全面增强实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 增强 Studio 打包流程、实现 Claude Code 兼容的本地 Skill 支持、完善 Runtime UI（MCP 管理、会话搜索导出、Markdown 高亮、响应式布局）、添加 E2E 集成测试、配置构建打包。

**Architecture:** 分 8 个任务模块，按优先级顺序执行。后端改动集中在 `apps/runtime/src-tauri/src/`，前端改动集中在 `apps/runtime/src/`，Studio 改动在 `apps/studio/`。每个模块独立可提交。

**Tech Stack:** Rust (Tauri 2, sqlx, serde_yaml), TypeScript (React 18, Tailwind CSS, react-syntax-highlighter), SQLite

---

## Task 1: Studio 打包修复 — 后端过滤与错误中文化

**Files:**
- Modify: `apps/studio/src-tauri/src/commands.rs`

**Step 1: 修改 `read_skill_dir` 过滤隐藏文件和无关目录**

在 `commands.rs` 的 `WalkDir` 遍历中添加过滤逻辑：

```rust
let files: Vec<String> = WalkDir::new(skill_dir)
    .into_iter()
    .filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        // 过滤隐藏文件/目录和 node_modules
        !name.starts_with('.') && name != "node_modules" && name != "__pycache__"
    })
    .filter_map(|e| e.ok())
    .filter(|e| e.file_type().is_file())
    .map(|e| {
        e.path()
            .strip_prefix(skill_dir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    })
    .collect();
```

**Step 2: 中文化 `pack_skill` 错误信息**

```rust
#[tauri::command]
pub async fn pack_skill(
    dir_path: String,
    name: String,
    description: String,
    version: String,
    author: String,
    username: String,
    recommended_model: String,
    output_path: String,
) -> Result<(), String> {
    let config = PackConfig {
        dir_path,
        name,
        description,
        version,
        author,
        username,
        recommended_model,
        output_path,
    };
    pack(&config).map_err(|e| format!("打包失败: {}", e))
}
```

**Step 3: 验证变更**

Run: `cd apps/studio/src-tauri && cargo check`
Expected: 编译通过

**Step 4: 提交**

```bash
git add apps/studio/src-tauri/src/commands.rs
git commit -m "fix(studio): 过滤隐藏文件 + 错误信息中文化"
```

---

## Task 2: Studio 打包修复 — 前端校验增强

**Files:**
- Modify: `apps/studio/src/components/PackForm.tsx`

**Step 1: 添加 semver 校验和表单校验增强**

在 `PackForm.tsx` 的 `handlePack` 函数顶部添加校验：

```tsx
async function handlePack() {
  // 校验必填项
  if (!name.trim()) {
    setErrorMsg("请填写 Skill 名称");
    setStatus("error");
    return;
  }
  if (!username.trim()) {
    setErrorMsg("请填写客户用户名");
    setStatus("error");
    return;
  }
  // 校验版本号格式 (semver)
  if (!/^\d+\.\d+\.\d+/.test(version.trim())) {
    setErrorMsg("版本号格式不正确，请使用 x.y.z 格式（如 1.0.0）");
    setStatus("error");
    return;
  }
  // ... 后续打包逻辑不变
}
```

**Step 2: 添加打包成功后的 manifest 预览**

替换 `status === "done"` 的渲染块，添加 `packResult` state：

```tsx
const [packResult, setPackResult] = useState<{
  id: string;
  fileCount: number;
} | null>(null);

// 在 handlePack 的 try 块中，打包成功后：
setStatus("done");
setPackResult({
  id: `${name.trim().toLowerCase().replace(/\s+/g, "-")}-${version}`,
  fileCount: skillInfo?.files.length ?? 0,
});
```

渲染：
```tsx
{status === "done" && packResult && (
  <div className="text-green-400 text-sm bg-green-950/50 border border-green-800/50 rounded-md p-3 space-y-1">
    <div className="font-medium">打包成功！</div>
    <div className="text-xs text-green-300/80">
      <div>Skill ID: {packResult.id}</div>
      <div>文件数: {packResult.fileCount}</div>
      <div>.skillpack 文件已保存</div>
    </div>
  </div>
)}
```

注意：`PackForm` 需要接收 `skillInfo` 的 files 数据，需在 props 中添加 `fileCount: number`。

**Step 3: 验证变更**

Run: `cd apps/studio && pnpm build`
Expected: TypeScript 编译通过

**Step 4: 提交**

```bash
git add apps/studio/src/components/PackForm.tsx apps/studio/src/App.tsx
git commit -m "fix(studio): 表单校验增强 + 打包结果预览"
```

---

## Task 3: Claude Code 兼容 Skill — 扩展 SkillConfig 解析

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/skill_config.rs`

**Step 1: 扩展 FrontMatter 和 SkillConfig 结构体**

```rust
use serde::Deserialize;

/// 从 SKILL.md 解析出的 Skill 配置（兼容 Claude Code 格式）
#[derive(Debug, Clone, Default)]
pub struct SkillConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub max_iterations: Option<usize>,
    pub system_prompt: String,
    // Claude Code 兼容字段
    pub argument_hint: Option<String>,
    pub disable_model_invocation: bool,
    pub user_invocable: bool,
    pub context: Option<String>,  // "fork" 表示隔离子 Agent
    pub agent: Option<String>,    // 子 Agent 类型
}

/// YAML frontmatter 的反序列化结构
#[derive(Deserialize, Default)]
struct FrontMatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    #[serde(alias = "allowed-tools")]
    allowed_tools: Option<AllowedToolsValue>,
    model: Option<String>,
    max_iterations: Option<usize>,
    // Claude Code 兼容字段
    #[serde(alias = "argument-hint")]
    argument_hint: Option<String>,
    #[serde(default)]
    #[serde(alias = "disable-model-invocation")]
    disable_model_invocation: bool,
    #[serde(default = "default_true")]
    #[serde(alias = "user-invocable")]
    user_invocable: bool,
    context: Option<String>,
    agent: Option<String>,
}

fn default_true() -> bool { true }

/// Claude Code 的 allowed-tools 可以是逗号分隔的字符串或数组
#[derive(Deserialize)]
#[serde(untagged)]
enum AllowedToolsValue {
    String(String),
    Array(Vec<String>),
}
```

**Step 2: 更新 `parse` 方法，处理 `AllowedToolsValue` 和新字段**

```rust
impl SkillConfig {
    pub fn parse(content: &str) -> Self {
        if !content.starts_with("---") {
            return Self {
                user_invocable: true,
                system_prompt: content.to_string(),
                ..Default::default()
            };
        }

        let rest = &content[3..];
        let end_pos = match rest.find("\n---") {
            Some(pos) => pos,
            None => {
                return Self {
                    user_invocable: true,
                    system_prompt: content.to_string(),
                    ..Default::default()
                };
            }
        };

        let yaml_str = &rest[..end_pos];
        let prompt_start = 3 + end_pos + 4;
        let system_prompt = if prompt_start < content.len() {
            content[prompt_start..].trim_start_matches('\n').to_string()
        } else {
            String::new()
        };

        let fm: FrontMatter = serde_yaml::from_str(yaml_str).unwrap_or_default();

        // 解析 allowed_tools：支持逗号分隔字符串和数组
        let allowed_tools = fm.allowed_tools.map(|v| match v {
            AllowedToolsValue::String(s) => {
                s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
            }
            AllowedToolsValue::Array(a) => a,
        });

        Self {
            name: fm.name,
            description: fm.description,
            allowed_tools,
            model: fm.model,
            max_iterations: fm.max_iterations,
            system_prompt,
            argument_hint: fm.argument_hint,
            disable_model_invocation: fm.disable_model_invocation,
            user_invocable: fm.user_invocable,
            context: fm.context,
            agent: fm.agent,
        }
    }

    /// 替换 Skill 内容中的 $ARGUMENTS 变量
    pub fn substitute_arguments(&self, content: &str, arguments: &[String]) -> String {
        let mut result = content.to_string();
        // $ARGUMENTS → 全部参数空格拼接
        result = result.replace("$ARGUMENTS", &arguments.join(" "));
        // $ARGUMENTS[N] → 第 N 个参数
        for (i, arg) in arguments.iter().enumerate() {
            result = result.replace(&format!("$ARGUMENTS[{}]", i), arg);
            result = result.replace(&format!("${}", i), arg);
        }
        result
    }
}
```

**Step 3: 运行现有测试确保不破坏**

Run: `cd apps/runtime/src-tauri && cargo test test_skill_config`
Expected: PASS

**Step 4: 提交**

```bash
git add apps/runtime/src-tauri/src/agent/skill_config.rs
git commit -m "feat(agent): 扩展 SkillConfig 兼容 Claude Code 格式"
```

---

## Task 4: Claude Code 兼容 Skill — DB 迁移 + import_local_skill 命令

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/src/commands/skills.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`

**Step 1: 添加 DB 迁移**

在 `db.rs` 的 `init_db` 函数末尾、`Ok(pool)` 前添加：

```rust
// Migration: add source_type column to installed_skills
let _ = sqlx::query("ALTER TABLE installed_skills ADD COLUMN source_type TEXT NOT NULL DEFAULT 'encrypted'")
    .execute(&pool)
    .await;
```

**Step 2: 新增 `import_local_skill` 命令**

在 `commands/skills.rs` 中添加：

```rust
use crate::agent::skill_config::SkillConfig;
use std::path::Path;

#[tauri::command]
pub async fn import_local_skill(
    dir_path: String,
    db: State<'_, DbState>,
) -> Result<SkillManifest, String> {
    let skill_dir = Path::new(&dir_path);
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_md_path.exists() {
        return Err("所选目录中未找到 SKILL.md 文件".to_string());
    }

    let content = std::fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("读取 SKILL.md 失败: {}", e))?;

    let config = SkillConfig::parse(&content);

    // 用目录名作为 fallback name
    let dir_name = skill_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed-skill".to_string());

    let skill_name = config.name.unwrap_or(dir_name);
    let skill_id = format!("local-{}", skill_name.to_lowercase().replace(' ', "-"));

    let manifest = SkillManifest {
        id: skill_id.clone(),
        name: skill_name,
        description: config.description.unwrap_or_default(),
        version: "local".to_string(),
        author: String::new(),
        recommended_model: config.model.unwrap_or_default(),
        tags: vec![],
        created_at: chrono::Utc::now(),
        username_hint: None,
        encrypted_verify: String::new(),
    };

    let manifest_json = serde_json::to_string(&manifest)
        .map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT OR REPLACE INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&manifest.id)
    .bind(&manifest_json)
    .bind(&now)
    .bind("")  // 本地 Skill 无需 username
    .bind(&dir_path)
    .bind("local")
    .execute(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(manifest)
}

#[tauri::command]
pub async fn refresh_local_skill(
    skill_id: String,
    db: State<'_, DbState>,
) -> Result<SkillManifest, String> {
    // 获取 pack_path（即目录路径）
    let (pack_path,): (String,) = sqlx::query_as(
        "SELECT pack_path FROM installed_skills WHERE id = ? AND source_type = 'local'"
    )
    .bind(&skill_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| format!("本地 Skill 不存在: {}", e))?;

    let skill_md_path = Path::new(&pack_path).join("SKILL.md");
    let content = std::fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("读取 SKILL.md 失败: {}", e))?;

    let config = SkillConfig::parse(&content);

    let dir_name = Path::new(&pack_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let skill_name = config.name.unwrap_or(dir_name);

    let manifest = SkillManifest {
        id: skill_id.clone(),
        name: skill_name,
        description: config.description.unwrap_or_default(),
        version: "local".to_string(),
        author: String::new(),
        recommended_model: config.model.unwrap_or_default(),
        tags: vec![],
        created_at: chrono::Utc::now(),
        username_hint: None,
        encrypted_verify: String::new(),
    };

    let manifest_json = serde_json::to_string(&manifest).map_err(|e| e.to_string())?;

    sqlx::query("UPDATE installed_skills SET manifest = ? WHERE id = ?")
        .bind(&manifest_json)
        .bind(&skill_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(manifest)
}
```

**Step 3: 注册新命令到 lib.rs**

在 `lib.rs` 的 `invoke_handler` 中添加：

```rust
commands::skills::import_local_skill,
commands::skills::refresh_local_skill,
```

**Step 4: 修改 `send_message` 支持本地 Skill**

在 `commands/chat.rs` 中，修改 Skill 内容加载逻辑（约第 108-130 行）：

```rust
// 加载 Skill 信息
let (manifest_json, username, pack_path, source_type) = sqlx::query_as::<_, (String, String, String, String)>(
    "SELECT manifest, username, pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?"
)
.bind(&skill_id)
.fetch_one(&db.0)
.await
.map_err(|e| format!("Skill 不存在 (skill_id={skill_id}): {e}"))?;

// 根据 source_type 获取 SKILL.md 内容
let raw_prompt = if source_type == "local" {
    // 本地 Skill：直接从目录读取
    let skill_md_path = std::path::Path::new(&pack_path).join("SKILL.md");
    std::fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("读取本地 Skill 失败: {}", e))?
} else {
    // 加密 Skill：解包获取
    match skillpack_rs::verify_and_unpack(&pack_path, &username) {
        Ok(unpacked) => {
            String::from_utf8_lossy(
                unpacked.files.get("SKILL.md").map(|v| v.as_slice()).unwrap_or_default()
            ).to_string()
        }
        Err(_) => {
            let manifest: skillpack_rs::SkillManifest = serde_json::from_str(&manifest_json)
                .map_err(|e| e.to_string())?;
            manifest.description
        }
    }
};
```

**Step 5: 验证编译**

Run: `cd apps/runtime/src-tauri && cargo check`
Expected: 编译通过

**Step 6: 提交**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/commands/skills.rs apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src-tauri/src/lib.rs
git commit -m "feat(runtime): 支持导入本地 Skill（Claude Code 兼容）"
```

---

## Task 5: Claude Code 兼容 Skill — 前端 UI

**Files:**
- Modify: `apps/runtime/src/components/InstallDialog.tsx`
- Modify: `apps/runtime/src/components/Sidebar.tsx`
- Modify: `apps/runtime/src/types.ts`

**Step 1: 扩展 types.ts**

在 `SkillManifest` 接口中，不需要改结构（`version: "local"` 足以区分），但前端需要知道 source_type。在 `list_skills` 返回的数据中已经有 `version`，可以用 `version === "local"` 判断。

或者更好的方案：让后端 `list_skills` 返回 `source_type`。修改 `types.ts`：

```typescript
export interface SkillManifest {
  id: string;
  name: string;
  description: string;
  version: string;
  author: string;
  recommended_model: string;
  tags: string[];
  created_at: string;
  username_hint?: string;
  source_type?: string;  // "encrypted" | "local"
}
```

**Step 2: 修改 InstallDialog 添加「导入本地 Skill」**

在 `InstallDialog.tsx` 中添加 Tab 切换或第二按钮：

```tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { SkillManifest } from "../types";

interface Props {
  onInstalled: () => void;
  onClose: () => void;
}

export function InstallDialog({ onInstalled, onClose }: Props) {
  const [mode, setMode] = useState<"encrypted" | "local">("encrypted");
  const [packPath, setPackPath] = useState("");
  const [username, setUsername] = useState("");
  const [localDir, setLocalDir] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function pickFile() {
    const f = await open({ filters: [{ name: "SkillPack", extensions: ["skillpack"] }] });
    if (f && typeof f === "string") setPackPath(f);
  }

  async function pickDir() {
    const d = await open({ directory: true, multiple: false });
    if (d && typeof d === "string") setLocalDir(d);
  }

  async function handleInstall() {
    setLoading(true);
    setError("");
    try {
      if (mode === "encrypted") {
        if (!packPath || !username.trim()) {
          setError("请选择文件并填写用户名");
          setLoading(false);
          return;
        }
        await invoke("install_skill", { packPath, username });
      } else {
        if (!localDir) {
          setError("请选择 Skill 目录");
          setLoading(false);
          return;
        }
        await invoke<SkillManifest>("import_local_skill", { dirPath: localDir });
      }
      onInstalled();
      onClose();
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-slate-800 rounded-lg p-6 w-96 space-y-4 border border-slate-600">
        <h2 className="font-semibold text-lg">安装 Skill</h2>

        {/* 模式切换 */}
        <div className="flex gap-2">
          <button
            onClick={() => setMode("encrypted")}
            className={"flex-1 py-1.5 rounded text-sm transition-colors " +
              (mode === "encrypted" ? "bg-blue-600 text-white" : "bg-slate-700 text-slate-300 hover:bg-slate-600")}
          >
            加密 .skillpack
          </button>
          <button
            onClick={() => setMode("local")}
            className={"flex-1 py-1.5 rounded text-sm transition-colors " +
              (mode === "local" ? "bg-blue-600 text-white" : "bg-slate-700 text-slate-300 hover:bg-slate-600")}
          >
            本地目录
          </button>
        </div>

        {mode === "encrypted" ? (
          <>
            <div>
              <button
                onClick={pickFile}
                className="w-full border border-dashed border-slate-500 rounded p-3 text-sm text-slate-400 hover:border-blue-500 hover:text-blue-400 transition-colors"
              >
                {packPath ? packPath.split(/[/\\]/).pop() : "选择 .skillpack 文件"}
              </button>
            </div>
            <div>
              <label className="block text-xs text-slate-400 mb-1">用户名（创作者提供）</label>
              <input
                className="w-full bg-slate-700 border border-slate-600 rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
              />
            </div>
          </>
        ) : (
          <div>
            <button
              onClick={pickDir}
              className="w-full border border-dashed border-slate-500 rounded p-3 text-sm text-slate-400 hover:border-blue-500 hover:text-blue-400 transition-colors"
            >
              {localDir ? localDir.split(/[/\\]/).slice(-2).join("/") : "选择包含 SKILL.md 的目录"}
            </button>
            <p className="text-xs text-slate-500 mt-1.5">
              兼容 Claude Code Skill 格式（SKILL.md + 可选 templates/examples/references 目录）
            </p>
          </div>
        )}

        {error && <div className="text-red-400 text-sm">{error}</div>}
        <div className="flex gap-2">
          <button
            onClick={onClose}
            className="flex-1 bg-slate-700 hover:bg-slate-600 py-2 rounded text-sm transition-colors"
          >
            取消
          </button>
          <button
            onClick={handleInstall}
            disabled={loading}
            className="flex-1 bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 py-2 rounded text-sm transition-colors"
          >
            {loading ? "安装中..." : "安装"}
          </button>
        </div>
      </div>
    </div>
  );
}
```

**Step 3: 修改 Sidebar 显示 [本地] 标签**

在 `Sidebar.tsx` 中，Skill 列表项添加标签：

```tsx
{skills.map((s) => (
  <button
    key={s.id}
    onClick={() => onSelectSkill(s.id)}
    className={
      "w-full text-left px-4 py-2 text-sm transition-colors " +
      (selectedSkillId === s.id
        ? "bg-blue-600/30 text-blue-300"
        : "text-slate-300 hover:bg-slate-700")
    }
  >
    <div className="font-medium truncate flex items-center gap-1.5">
      {s.name}
      {s.id.startsWith("local-") && (
        <span className="text-[10px] bg-green-800/60 text-green-300 px-1 py-0.5 rounded">本地</span>
      )}
    </div>
    <div className="text-xs text-slate-500 truncate">{s.version}</div>
  </button>
))}
```

**Step 4: 验证编译**

Run: `cd apps/runtime && pnpm build`
Expected: TypeScript 编译通过

**Step 5: 提交**

```bash
git add apps/runtime/src/components/InstallDialog.tsx apps/runtime/src/components/Sidebar.tsx apps/runtime/src/types.ts
git commit -m "feat(ui): 本地 Skill 导入界面 + Sidebar 标签"
```

---

## Task 6: Markdown 代码高亮

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: 添加代码高亮**

`react-syntax-highlighter` 已在 `package.json` 中。在 `ChatView.tsx` 顶部添加 import：

```tsx
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
```

**Step 2: 创建自定义 Markdown 渲染组件映射**

在 `ChatView.tsx` 中，在组件函数内部创建 components 配置（仅定义一次）：

```tsx
const markdownComponents = {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  code({ className, children, ...props }: any) {
    const match = /language-(\w+)/.exec(className || "");
    const codeString = String(children).replace(/\n$/, "");
    return match ? (
      <SyntaxHighlighter
        style={oneDark}
        language={match[1]}
        PreTag="div"
        customStyle={{ margin: 0, borderRadius: "0.375rem", fontSize: "0.8125rem" }}
      >
        {codeString}
      </SyntaxHighlighter>
    ) : (
      <code className={"bg-slate-600/50 px-1.5 py-0.5 rounded text-sm " + (className || "")} {...props}>
        {children}
      </code>
    );
  },
};
```

**Step 3: 应用到所有 ReactMarkdown**

将文件中所有 `<ReactMarkdown>` 替换为：

```tsx
<ReactMarkdown components={markdownComponents}>
```

搜索替换原则：
- `<ReactMarkdown>{m.content}</ReactMarkdown>` → `<ReactMarkdown components={markdownComponents}>{m.content}</ReactMarkdown>`
- `<ReactMarkdown>{streamBuffer}</ReactMarkdown>` → `<ReactMarkdown components={markdownComponents}>{streamBuffer}</ReactMarkdown>`

**Step 4: 验证编译**

Run: `cd apps/runtime && pnpm build`
Expected: 编译通过

**Step 5: 提交**

```bash
git add apps/runtime/src/components/ChatView.tsx
git commit -m "feat(ui): Markdown 代码语法高亮（Prism + oneDark）"
```

---

## Task 7: MCP 服务器管理 UI 增强

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`

**Step 1: 添加 Tab 切换**

`SettingsView.tsx` 已经包含了 MCP 服务器管理代码（在同一个页面底部）。需要改为 Tab 布局。

在 `SettingsView` 组件内添加 tab state：

```tsx
const [activeTab, setActiveTab] = useState<"models" | "mcp">("models");
```

修改渲染结构，在标题行后添加 Tab 切换：

```tsx
<div className="flex items-center justify-between mb-6">
  <div className="flex items-center gap-4">
    <button
      onClick={() => setActiveTab("models")}
      className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
        (activeTab === "models" ? "text-white border-blue-500" : "text-slate-400 border-transparent hover:text-slate-200")}
    >
      模型配置
    </button>
    <button
      onClick={() => setActiveTab("mcp")}
      className={"text-sm font-medium pb-1 border-b-2 transition-colors " +
        (activeTab === "mcp" ? "text-white border-blue-500" : "text-slate-400 border-transparent hover:text-slate-200")}
    >
      MCP 服务器
    </button>
  </div>
  <button onClick={onClose} className="text-slate-400 hover:text-white text-sm">
    返回
  </button>
</div>
```

然后用 `{activeTab === "models" && (...)}` 和 `{activeTab === "mcp" && (...)}` 包裹对应内容。

**Step 2: 增强 MCP 表单**

添加环境变量输入字段：

```tsx
const [mcpForm, setMcpForm] = useState({ name: "", command: "", args: "", env: "" });
```

添加环境变量输入框（在参数输入后面）：

```tsx
<div>
  <label className={labelCls}>环境变量（JSON 格式，可选）</label>
  <input
    className={inputCls}
    placeholder='例: {"API_KEY": "xxx"}'
    value={mcpForm.env}
    onChange={(e) => setMcpForm({ ...mcpForm, env: e.target.value })}
  />
</div>
```

修改 `handleAddMcp` 解析环境变量：

```tsx
async function handleAddMcp() {
  setMcpError("");
  try {
    const args = mcpForm.args.split(/\s+/).filter(Boolean);
    let env: Record<string, string> = {};
    if (mcpForm.env.trim()) {
      try {
        env = JSON.parse(mcpForm.env);
      } catch {
        setMcpError("环境变量格式不正确，请使用 JSON 格式");
        return;
      }
    }
    await invoke("add_mcp_server", {
      name: mcpForm.name,
      command: mcpForm.command,
      args,
      env,
    });
    setMcpForm({ name: "", command: "", args: "", env: "" });
    loadMcpServers();
  } catch (e) {
    setMcpError(String(e));
  }
}
```

**Step 3: 验证编译**

Run: `cd apps/runtime && pnpm build`
Expected: 编译通过

**Step 4: 提交**

```bash
git add apps/runtime/src/components/SettingsView.tsx
git commit -m "feat(ui): 设置页 Tab 布局 + MCP 环境变量输入"
```

---

## Task 8: 会话搜索与导出

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Modify: `apps/runtime/src/components/Sidebar.tsx`
- Modify: `apps/runtime/src/App.tsx`

**Step 1: 后端 — 添加 `search_sessions` 命令**

在 `commands/chat.rs` 中添加：

```rust
#[tauri::command]
pub async fn search_sessions(
    skill_id: String,
    query: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    let pattern = format!("%{}%", query);
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT DISTINCT s.id, s.title, s.created_at, s.model_id
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
         WHERE s.skill_id = ? AND (s.title LIKE ? OR m.content LIKE ?)
         ORDER BY s.created_at DESC"
    )
    .bind(&skill_id)
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.iter().map(|(id, title, created_at, model_id)| {
        json!({
            "id": id,
            "title": title,
            "created_at": created_at,
            "model_id": model_id,
        })
    }).collect())
}
```

**Step 2: 后端 — 添加 `export_session` 命令**

```rust
#[tauri::command]
pub async fn export_session(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    // 获取会话标题
    let (title,): (String,) = sqlx::query_as(
        "SELECT title FROM sessions WHERE id = ?"
    )
    .bind(&session_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    // 获取所有消息
    let messages = sqlx::query_as::<_, (String, String, String)>(
        "SELECT role, content, created_at FROM messages WHERE session_id = ? ORDER BY created_at ASC"
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let mut markdown = format!("# {}\n\n", title);
    for (role, content, created_at) in &messages {
        let label = if role == "user" { "用户" } else { "助手" };
        markdown.push_str(&format!("## {} ({})\n\n{}\n\n---\n\n", label, created_at, content));
    }

    Ok(markdown)
}
```

**Step 3: 注册命令**

在 `lib.rs` 的 `invoke_handler` 中添加：

```rust
commands::chat::search_sessions,
commands::chat::export_session,
```

**Step 4: 前端 — Sidebar 添加搜索框**

修改 `Sidebar.tsx`，在 Props 中添加：

```typescript
interface Props {
  // ... 现有 props
  onSearchSessions: (query: string) => void;
  onExportSession: (sessionId: string) => void;
}
```

在会话历史标题行下方添加搜索框：

```tsx
{selectedSkillId && (
  <>
    <div className="px-4 py-2 text-xs font-medium text-slate-400 border-t border-b border-slate-700 flex items-center justify-between">
      <span>会话历史</span>
      <button onClick={onNewSession} className="text-blue-400 hover:text-blue-300 text-xs">
        + 新建
      </button>
    </div>
    <div className="px-3 py-2">
      <input
        type="text"
        placeholder="搜索会话..."
        onChange={(e) => onSearchSessions(e.target.value)}
        className="w-full bg-slate-700 border border-slate-600 rounded px-2 py-1 text-xs focus:outline-none focus:border-blue-500 placeholder-slate-500"
      />
    </div>
    {/* 会话列表 */}
    <div className="flex-1 overflow-y-auto py-1">
      {sessions.map((s) => (
        <div key={s.id} className={"group flex items-center px-4 py-2 text-sm cursor-pointer transition-colors " + (selectedSessionId === s.id ? "bg-blue-600/20 text-blue-300" : "text-slate-300 hover:bg-slate-700")} onClick={() => onSelectSession(s.id)}>
          <div className="flex-1 min-w-0">
            <div className="truncate text-xs">{s.title || "New Chat"}</div>
          </div>
          <button
            onClick={(e) => { e.stopPropagation(); onExportSession(s.id); }}
            title="导出"
            className="hidden group-hover:block text-slate-400 hover:text-blue-300 text-xs ml-1 flex-shrink-0"
          >
            ↓
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onDeleteSession(s.id); }}
            className="hidden group-hover:block text-red-400 hover:text-red-300 text-xs ml-1 flex-shrink-0"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  </>
)}
```

**Step 5: 前端 — App.tsx 添加搜索和导出逻辑**

在 `App.tsx` 中添加：

```tsx
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/api/fs";

// 搜索防抖
const searchTimerRef = useRef<ReturnType<typeof setTimeout>>();

async function handleSearchSessions(query: string) {
  if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
  if (!query.trim() || !selectedSkillId) {
    if (selectedSkillId) loadSessions(selectedSkillId);
    return;
  }
  searchTimerRef.current = setTimeout(async () => {
    try {
      const list = await invoke<SessionInfo[]>("search_sessions", {
        skillId: selectedSkillId,
        query: query.trim(),
      });
      setSessions(list);
    } catch (e) {
      console.error("搜索会话失败:", e);
    }
  }, 300);
}

async function handleExportSession(sessionId: string) {
  try {
    const markdown = await invoke<string>("export_session", { sessionId });
    const filePath = await save({
      defaultPath: "session-export.md",
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (filePath) {
      await invoke("write_export_file", { path: filePath, content: markdown });
    }
  } catch (e) {
    console.error("导出会话失败:", e);
  }
}
```

注意：Tauri 2 的前端 `fs` 模块可能需要通过后端命令来写文件。如果 `@tauri-apps/api/fs` 不可用，添加一个简单的后端命令：

```rust
// commands/chat.rs
#[tauri::command]
pub async fn write_export_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, &content).map_err(|e| format!("写入失败: {}", e))
}
```

在 `lib.rs` 注册 `commands::chat::write_export_file`。

将 `onSearchSessions` 和 `onExportSession` 传递给 `Sidebar` 组件。

**Step 6: 验证编译**

Run: `cd apps/runtime/src-tauri && cargo check && cd .. && pnpm build`
Expected: 两者均通过

**Step 7: 提交**

```bash
git add apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src/components/Sidebar.tsx apps/runtime/src/App.tsx
git commit -m "feat(ui): 会话搜索 + Markdown 导出"
```

---

## Task 9: 响应式布局

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/components/Sidebar.tsx`
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: App.tsx — 添加 Sidebar 折叠状态**

```tsx
const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

// 在渲染中
<div className="flex h-screen bg-slate-900 text-slate-100 overflow-hidden">
  {!sidebarCollapsed && (
    <Sidebar
      // ... 现有 props
      onCollapse={() => setSidebarCollapsed(true)}
    />
  )}
  <div className="flex-1 overflow-hidden flex flex-col">
    {sidebarCollapsed && (
      <button
        onClick={() => setSidebarCollapsed(false)}
        className="absolute top-3 left-3 z-20 bg-slate-800 border border-slate-700 rounded p-1.5 text-slate-400 hover:text-white"
        title="展开侧边栏"
      >
        ☰
      </button>
    )}
    {/* 现有内容 */}
  </div>
</div>
```

**Step 2: Sidebar.tsx — 添加折叠按钮**

在 Props 中添加 `onCollapse: () => void`。

在 Sidebar 顶部或底部添加折叠按钮：

```tsx
<div className="px-4 py-3 text-xs font-medium text-slate-400 border-b border-slate-700 flex items-center justify-between">
  <span>已安装 Skill</span>
  <button
    onClick={onCollapse}
    className="text-slate-500 hover:text-slate-300 text-xs"
    title="收起侧边栏"
  >
    ◀
  </button>
</div>
```

**Step 3: ChatView.tsx — 消息宽度自适应**

将 `max-w-2xl` 替换为 `max-w-[80%]`：

```tsx
// 消息气泡
<div className={"max-w-[80%] rounded-lg px-4 py-2 text-sm " + ...}>

// 流式输出区域
<div className="max-w-[80%] bg-slate-700 rounded-lg px-4 py-2 text-sm text-slate-100">
```

**Step 4: 验证编译**

Run: `cd apps/runtime && pnpm build`
Expected: 编译通过

**Step 5: 提交**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/components/Sidebar.tsx apps/runtime/src/components/ChatView.tsx
git commit -m "feat(ui): 响应式布局 — 侧边栏折叠 + 消息宽度自适应"
```

---

## Task 10: E2E 集成测试 — 测试基础设施

**Files:**
- Create: `apps/runtime/src-tauri/tests/helpers/mod.rs`
- Create: `apps/runtime/src-tauri/tests/fixtures/test-skill/SKILL.md`

**Step 1: 创建测试辅助模块**

```rust
// tests/helpers/mod.rs
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tempfile::TempDir;
use std::path::PathBuf;

/// 创建内存 SQLite 数据库，用于测试
pub async fn setup_test_db() -> (SqlitePool, TempDir) {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .unwrap();

    // 创建所有表
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS installed_skills (
            id TEXT PRIMARY KEY,
            manifest TEXT NOT NULL,
            installed_at TEXT NOT NULL,
            last_used_at TEXT,
            username TEXT NOT NULL,
            pack_path TEXT NOT NULL DEFAULT '',
            source_type TEXT NOT NULL DEFAULT 'encrypted'
        )"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            skill_id TEXT NOT NULL,
            title TEXT,
            created_at TEXT NOT NULL,
            model_id TEXT NOT NULL,
            permission_mode TEXT NOT NULL DEFAULT 'default'
        )"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS model_configs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            api_format TEXT NOT NULL,
            base_url TEXT NOT NULL,
            model_name TEXT NOT NULL,
            is_default INTEGER DEFAULT 0,
            api_key TEXT NOT NULL DEFAULT ''
        )"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS mcp_servers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            command TEXT NOT NULL,
            args TEXT NOT NULL DEFAULT '[]',
            env TEXT NOT NULL DEFAULT '{}',
            enabled INTEGER DEFAULT 1,
            created_at TEXT NOT NULL
        )"
    ).execute(&pool).await.unwrap();

    (pool, tmp)
}

/// 创建测试用的 Skill 目录
pub fn create_test_skill_dir() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let skill_dir = tmp.path().join("test-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();

    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: test-skill\ndescription: A test skill\nallowed-tools: ReadFile, Glob\n---\n\nYou are a helpful test assistant.\n"
    ).unwrap();

    let templates_dir = skill_dir.join("templates");
    std::fs::create_dir_all(&templates_dir).unwrap();
    std::fs::write(
        templates_dir.join("greeting.md"),
        "Hello, {{name}}!"
    ).unwrap();

    (tmp, skill_dir)
}
```

**Step 2: 创建 fixture Skill 文件**

```markdown
<!-- tests/fixtures/test-skill/SKILL.md -->
---
name: test-skill
description: A test skill for E2E testing
allowed-tools: ReadFile, Glob
user-invocable: true
---

You are a helpful test assistant. Answer questions concisely.
```

**Step 3: 验证编译**

Run: `cd apps/runtime/src-tauri && cargo check --tests`
Expected: 编译通过

**Step 4: 提交**

```bash
git add apps/runtime/src-tauri/tests/helpers/ apps/runtime/src-tauri/tests/fixtures/
git commit -m "test: E2E 测试基础设施 — 辅助函数和 fixture"
```

---

## Task 11: E2E 集成测试 — 核心测试用例

**Files:**
- Create: `apps/runtime/src-tauri/tests/test_e2e_flow.rs`

**Step 1: 编写测试文件**

```rust
// tests/test_e2e_flow.rs
mod helpers;

use helpers::{setup_test_db, create_test_skill_dir};
use runtime_lib::agent::skill_config::SkillConfig;
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;

#[tokio::test]
async fn test_import_local_skill_and_read() {
    let (pool, _tmp_db) = setup_test_db().await;
    let (_tmp_skill, skill_dir) = create_test_skill_dir();

    let dir_path = skill_dir.to_string_lossy().to_string();

    // 读取 SKILL.md
    let content = std::fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
    let config = SkillConfig::parse(&content);

    assert_eq!(config.name.as_deref(), Some("test-skill"));
    assert_eq!(config.description.as_deref(), Some("A test skill for E2E testing"));
    assert!(config.user_invocable);

    // 模拟 import_local_skill 逻辑
    let skill_name = config.name.unwrap_or_default();
    let skill_id = format!("local-{}", skill_name.to_lowercase().replace(' ', "-"));

    let manifest = json!({
        "id": skill_id,
        "name": skill_name,
        "description": config.description.unwrap_or_default(),
        "version": "local",
        "author": "",
        "recommended_model": "",
        "tags": [],
        "created_at": Utc::now().to_rfc3339(),
    });
    let manifest_json = serde_json::to_string(&manifest).unwrap();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&skill_id)
    .bind(&manifest_json)
    .bind(&now)
    .bind("")
    .bind(&dir_path)
    .bind("local")
    .execute(&pool)
    .await
    .unwrap();

    // 验证已安装
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM installed_skills WHERE source_type = 'local'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // 验证可以读取回来
    let (stored_manifest,): (String,) = sqlx::query_as(
        "SELECT manifest FROM installed_skills WHERE id = ?"
    )
    .bind(&skill_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&stored_manifest).unwrap();
    assert_eq!(parsed["name"], "test-skill");
}

#[tokio::test]
async fn test_session_lifecycle() {
    let (pool, _tmp) = setup_test_db().await;

    let skill_id = "test-skill-001";
    let model_id = "test-model-001";
    let now = Utc::now().to_rfc3339();

    // 插入 Skill
    sqlx::query(
        "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(skill_id)
    .bind(r#"{"id":"test-skill-001","name":"Test","description":"","version":"1.0.0","author":"","recommended_model":"","tags":[],"created_at":"2026-01-01T00:00:00Z"}"#)
    .bind(&now)
    .bind("testuser")
    .bind("/fake/path.skillpack")
    .execute(&pool)
    .await
    .unwrap();

    // 创建会话
    let session_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&session_id)
    .bind(skill_id)
    .bind("Test Chat")
    .bind(&now)
    .bind(model_id)
    .execute(&pool)
    .await
    .unwrap();

    // 插入消息
    let msg_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&msg_id)
    .bind(&session_id)
    .bind("user")
    .bind("你好")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 验证消息计数
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM messages WHERE session_id = ?"
    )
    .bind(&session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);

    // 删除会话及消息
    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(&session_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&session_id)
        .execute(&pool)
        .await
        .unwrap();

    // 验证清理
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_search_sessions() {
    let (pool, _tmp) = setup_test_db().await;

    let skill_id = "search-test-skill";
    let now = Utc::now().to_rfc3339();

    // 创建两个会话
    let s1 = Uuid::new_v4().to_string();
    let s2 = Uuid::new_v4().to_string();

    for (sid, title) in [(&s1, "Rust 编程讨论"), (&s2, "Python 数据分析")] {
        sqlx::query(
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(sid)
        .bind(skill_id)
        .bind(title)
        .bind(&now)
        .bind("model-1")
        .execute(&pool)
        .await
        .unwrap();
    }

    // 搜索 "Rust"
    let pattern = "%Rust%";
    let results = sqlx::query_as::<_, (String, String)>(
        "SELECT id, title FROM sessions WHERE skill_id = ? AND title LIKE ?"
    )
    .bind(skill_id)
    .bind(pattern)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "Rust 编程讨论");
}

#[tokio::test]
async fn test_mcp_server_crud() {
    let (pool, _tmp) = setup_test_db().await;
    let now = Utc::now().to_rfc3339();

    let server_id = Uuid::new_v4().to_string();

    // 添加
    sqlx::query(
        "INSERT INTO mcp_servers (id, name, command, args, env, enabled, created_at) VALUES (?, ?, ?, ?, ?, 1, ?)"
    )
    .bind(&server_id)
    .bind("test-server")
    .bind("npx")
    .bind(r#"["@test/mcp-server"]"#)
    .bind("{}")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 列表
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mcp_servers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // 删除
    sqlx::query("DELETE FROM mcp_servers WHERE id = ?")
        .bind(&server_id)
        .execute(&pool)
        .await
        .unwrap();

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mcp_servers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_skill_config_claude_code_compat() {
    let content = r#"---
name: deploy-production
description: Deploy application to production
disable-model-invocation: true
allowed-tools: Bash, Read
model: claude-sonnet-4-5
context: fork
agent: Plan
argument-hint: "[environment]"
---

Deploy to $ARGUMENTS[0] environment.
"#;

    let config = SkillConfig::parse(content);
    assert_eq!(config.name.as_deref(), Some("deploy-production"));
    assert!(config.disable_model_invocation);
    assert_eq!(config.context.as_deref(), Some("fork"));
    assert_eq!(config.agent.as_deref(), Some("Plan"));
    assert_eq!(config.argument_hint.as_deref(), Some("[environment]"));

    // 测试 allowed_tools 逗号分隔解析
    let tools = config.allowed_tools.unwrap();
    assert_eq!(tools, vec!["Bash", "Read"]);

    // 测试参数替换
    let substituted = config.substitute_arguments(
        "Deploy to $ARGUMENTS[0] environment.",
        &["production".to_string()],
    );
    assert_eq!(substituted, "Deploy to production environment.");
}
```

**Step 2: 运行测试**

Run: `cd apps/runtime/src-tauri && cargo test test_e2e`
Expected: 全部 PASS

**Step 3: 提交**

```bash
git add apps/runtime/src-tauri/tests/test_e2e_flow.rs
git commit -m "test: E2E 集成测试 — Skill 导入、会话生命周期、搜索、MCP CRUD"
```

---

## Task 12: 基础构建配置检查

**Files:**
- Verify: `apps/runtime/src-tauri/tauri.conf.json`
- Verify: `apps/studio/src-tauri/tauri.conf.json`

**Step 1: 检查 Runtime tauri.conf.json**

确认以下字段正确：
- `productName`: "WorkClaw Runtime"
- `version`: 与 Cargo.toml 一致
- `bundle.targets`: 包含 `"nsis"` (Windows)
- `bundle.icon`: 指向有效图标文件
- `bundle.identifier`: "dev.workclaw.runtime"

**Step 2: 检查 Studio tauri.conf.json**

同上检查 Studio 配置。

**Step 3: 检查图标文件**

确认 `apps/runtime/src-tauri/icons/` 和 `apps/studio/src-tauri/icons/` 存在必需格式：
- `icon.ico` (Windows)
- `icon.png` (各种尺寸)

如果缺失，Tauri 会使用默认图标。记录需要替换但不阻塞构建。

**Step 4: 验证构建命令**

Run: `cd apps/runtime && pnpm build` (仅前端构建验证)
Run: `cd apps/runtime/src-tauri && cargo check --release` (Rust release 编译检查)
Expected: 两者均通过

**Step 5: 提交（仅当有配置修改时）**

```bash
git add apps/runtime/src-tauri/tauri.conf.json apps/studio/src-tauri/tauri.conf.json
git commit -m "chore: 检查并修正构建配置"
```

---

## 执行顺序总结

```
Task 1:  Studio 后端过滤 + 错误中文化          [5 min]
Task 2:  Studio 前端校验增强                    [10 min]
Task 3:  SkillConfig 扩展 (Claude Code 兼容)     [10 min]
Task 4:  DB 迁移 + import_local_skill 命令       [15 min]
Task 5:  本地 Skill 前端 UI                      [15 min]
Task 6:  Markdown 代码高亮                       [5 min]
Task 7:  MCP 管理 UI Tab 化                      [10 min]
Task 8:  会话搜索与导出                          [15 min]
Task 9:  响应式布局                              [10 min]
Task 10: E2E 测试基础设施                        [10 min]
Task 11: E2E 测试用例                            [15 min]
Task 12: 构建配置检查                            [5 min]
```
