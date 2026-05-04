# Full Access Path Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `full_access` allow WorkClaw file tools to access ordinary absolute paths outside the session directory while preserving workspace-only behavior for standard modes and blocking sensitive paths.

**Architecture:** Keep `work_dir` as the session cwd and relative-path base, and add a separate `PathAccessPolicy` to `ToolContext`. Runtime context builders derive this policy from `PermissionMode`, while every file-like tool continues to use `ctx.check_path` as the single path authorization gate.

**Tech Stack:** Rust Tauri runtime (`apps/runtime/src-tauri`), existing `PermissionMode`, Rust integration tests, React desktop settings copy.

---

## File Structure

- Modify `apps/runtime/src-tauri/src/agent/types.rs`
  - Add `PathAccessPolicy`.
  - Add `path_access` to `ToolContext`.
  - Update `check_path` to allow ordinary external paths only under full-access policy.
- Create `apps/runtime/src-tauri/src/agent/path_access.rs`
  - Own sensitive-path classification helpers so `types.rs` does not become a policy grab bag.
- Modify `apps/runtime/src-tauri/src/agent/mod.rs`
  - Register and re-export the new path policy type as needed.
- Modify `apps/runtime/src-tauri/src/agent/context.rs`
  - Add `build_tool_context_with_permission_mode`.
  - Keep `build_tool_context` as a compatibility wrapper for existing tests and any context without explicit permission mode.
- Modify `apps/runtime/src-tauri/src/agent/turn_executor.rs`
  - Build `ToolContext` from `permission_mode`.
  - Update tests that construct `ToolContext` literals.
- Modify `apps/runtime/src-tauri/src/agent/runtime/kernel/direct_dispatch.rs`
  - Pass `execution_context.permission_mode` when building tool context.
- Modify `apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs`
  - Pass `execution_context.permission_mode` when building tool context for direct vision dispatch.
- Modify `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
  - Pass `execution_context.permission_mode` when building tool context for user skill commands.
- Modify `apps/runtime/src-tauri/tests/test_write_file.rs`
  - Add full-access outside-workspace write coverage.
  - Update literal contexts with `path_access`.
- Modify `apps/runtime/src/components/settings/desktop/DesktopRuntimeSection.tsx`
  - Clarify full-access copy.

## Task 1: Add Path Policy Tests

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/types.rs`
- Modify: `apps/runtime/src-tauri/tests/test_write_file.rs`

- [ ] **Step 1: Add failing unit tests for `ToolContext::check_path`**

In `apps/runtime/src-tauri/src/agent/types.rs`, extend the existing `#[cfg(test)] mod tests` import:

```rust
use super::{
    AgentStateEvent, BackgroundProcessEvent, PathAccessPolicy, ToolCallEvent, ToolContext,
};
use serde_json::{json, Value};
use tempfile::tempdir;
```

Add these tests inside the same `tests` module:

```rust
#[test]
fn workspace_only_rejects_absolute_path_outside_work_dir() {
    let work_dir = tempdir().expect("create work dir");
    let outside_dir = tempdir().expect("create outside dir");
    let outside_file = outside_dir.path().join("outside.txt");
    let ctx = ToolContext {
        work_dir: Some(work_dir.path().to_path_buf()),
        path_access: PathAccessPolicy::WorkspaceOnly,
        ..Default::default()
    };

    let err = ctx
        .check_path(&outside_file.to_string_lossy())
        .expect_err("outside path should be rejected");

    assert!(err.to_string().contains("不在工作目录"));
}

#[test]
fn full_access_allows_ordinary_absolute_path_outside_work_dir() {
    let work_dir = tempdir().expect("create work dir");
    let outside_dir = tempdir().expect("create outside dir");
    let outside_file = outside_dir.path().join("outside.txt");
    let ctx = ToolContext {
        work_dir: Some(work_dir.path().to_path_buf()),
        path_access: PathAccessPolicy::FullAccessWithSensitiveGuards,
        ..Default::default()
    };

    let checked = ctx
        .check_path(&outside_file.to_string_lossy())
        .expect("ordinary outside path should be allowed");

    assert_eq!(checked, outside_file);
}

#[test]
fn full_access_rejects_sensitive_absolute_path_outside_work_dir() {
    let work_dir = tempdir().expect("create work dir");
    let outside_dir = tempdir().expect("create outside dir");
    let sensitive_file = outside_dir.path().join(".ssh").join("config");
    let ctx = ToolContext {
        work_dir: Some(work_dir.path().to_path_buf()),
        path_access: PathAccessPolicy::FullAccessWithSensitiveGuards,
        ..Default::default()
    };

    let err = ctx
        .check_path(&sensitive_file.to_string_lossy())
        .expect_err("sensitive outside path should be rejected");

    assert!(err.to_string().contains("敏感路径"));
}

#[test]
fn relative_paths_still_resolve_under_work_dir_in_full_access() {
    let work_dir = tempdir().expect("create work dir");
    let ctx = ToolContext {
        work_dir: Some(work_dir.path().to_path_buf()),
        path_access: PathAccessPolicy::FullAccessWithSensitiveGuards,
        ..Default::default()
    };

    let checked = ctx.check_path("nested/report.md").expect("relative path");

    assert_eq!(checked, work_dir.path().join("nested").join("report.md"));
}
```

- [ ] **Step 2: Add failing write-file integration test**

In `apps/runtime/src-tauri/tests/test_write_file.rs`, update the import:

```rust
use runtime_lib::agent::{PathAccessPolicy, Tool, ToolContext, WriteFileTool};
```

Add this test:

```rust
#[test]
fn test_write_file_allows_absolute_path_outside_work_dir_in_full_access() {
    let tool = WriteFileTool;
    let work_dir = setup_work_dir("full_access_work_dir");
    let outside_dir = setup_work_dir("full_access_outside_dir");
    let target = outside_dir.join("artifact.md");
    let ctx = ToolContext {
        work_dir: Some(work_dir.clone()),
        path_access: PathAccessPolicy::FullAccessWithSensitiveGuards,
        allowed_tools: None,
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };

    let input = json!({
        "path": target.to_str().unwrap(),
        "content": "# outside"
    });

    let result = tool.execute(input, &ctx).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["details"]["bytes_written"], 9);
    assert_eq!(fs::read_to_string(&target).unwrap(), "# outside");

    fs::remove_dir_all(&work_dir).unwrap();
    fs::remove_dir_all(&outside_dir).unwrap();
}
```

- [ ] **Step 3: Run tests to verify they fail before implementation**

Run:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml workspace_only_rejects_absolute_path_outside_work_dir -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_write_file_allows_absolute_path_outside_work_dir_in_full_access -- --nocapture
```

Expected: compile failure because `PathAccessPolicy` and `ToolContext.path_access` do not exist yet.

## Task 2: Implement Path Access Policy

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/path_access.rs`
- Modify: `apps/runtime/src-tauri/src/agent/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/types.rs`
- Modify: `apps/runtime/src-tauri/tests/test_write_file.rs`

- [ ] **Step 1: Create sensitive-path helper**

Create `apps/runtime/src-tauri/src/agent/path_access.rs`:

```rust
use std::path::{Component, Path};

const SENSITIVE_DIR_NAMES: &[&str] = &[
    ".ssh",
    ".aws",
    ".azure",
    ".kube",
    ".gnupg",
    ".git",
];

const SENSITIVE_FILE_NAMES: &[&str] = &[
    ".bashrc",
    ".bash_profile",
    ".zshrc",
    ".profile",
    "credentials",
    "config",
    "known_hosts",
    "authorized_keys",
];

const SENSITIVE_EXTENSIONS: &[&str] = &[
    "pem",
    "key",
    "p12",
    "pfx",
];

pub(crate) fn is_sensitive_path(path: &Path) -> bool {
    let mut inside_sensitive_dir = false;
    for component in path.components() {
        let Component::Normal(value) = component else {
            continue;
        };
        let segment = value.to_string_lossy().to_ascii_lowercase();
        if SENSITIVE_DIR_NAMES.iter().any(|name| segment == *name) {
            inside_sensitive_dir = true;
            break;
        }
    }
    if inside_sensitive_dir {
        return true;
    }

    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    let file_name = file_name.to_ascii_lowercase();
    if file_name == ".env" || file_name.starts_with(".env.") {
        return true;
    }
    if SENSITIVE_FILE_NAMES.iter().any(|name| file_name == *name) {
        return true;
    }

    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            SENSITIVE_EXTENSIONS.iter().any(|sensitive| ext == *sensitive)
        })
        .unwrap_or(false)
}
```

- [ ] **Step 2: Register and export the new module**

In `apps/runtime/src-tauri/src/agent/mod.rs`, add:

```rust
pub mod path_access;
```

Update the public type exports:

```rust
pub use types::{
    AgentState, AgentStateEvent, BackgroundProcessEvent, LLMResponse, PathAccessPolicy, Tool,
    ToolCall, ToolCallEvent, ToolContext, ToolResult,
};
```

- [ ] **Step 3: Add `PathAccessPolicy` and update `ToolContext`**

In `apps/runtime/src-tauri/src/agent/types.rs`, add this import:

```rust
use super::path_access::is_sensitive_path;
```

Add the enum above `ToolContext`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathAccessPolicy {
    WorkspaceOnly,
    FullAccessWithSensitiveGuards,
}

impl Default for PathAccessPolicy {
    fn default() -> Self {
        Self::WorkspaceOnly
    }
}
```

Update `ToolContext`:

```rust
pub struct ToolContext {
    /// 工作目录路径；相对文件路径会以此为基准解析
    pub work_dir: Option<PathBuf>,
    /// 文件工具路径访问策略
    pub path_access: PathAccessPolicy,
    /// 当前回合允许调用的工具集合（已规范化工具名）
    pub allowed_tools: Option<Vec<String>>,
    /// 当前会话标识，便于工具层记录和诊断
    pub session_id: Option<String>,
    /// 任务级临时目录，用于中间产物和受控退路
    pub task_temp_dir: Option<PathBuf>,
    /// 运行时探测到的执行能力
    pub execution_caps: Option<ExecutionCaps>,
    /// 文件任务预检结果
    pub file_task_caps: Option<FileTaskCaps>,
}
```

- [ ] **Step 4: Update `check_path` policy logic**

Replace the current `check_path` body in `apps/runtime/src-tauri/src/agent/types.rs` with:

```rust
pub fn check_path(&self, path: &str) -> anyhow::Result<PathBuf> {
    let target = std::path::Path::new(path);
    let canonical = if target.is_absolute() {
        target.to_path_buf()
    } else if let Some(ref wd) = self.work_dir {
        wd.join(target)
    } else {
        std::env::current_dir()?.join(target)
    };

    let check_path = Self::normalize_for_scope_check(&canonical)?;

    if let Some(ref wd) = self.work_dir {
        let wd_canonical = Self::normalize_for_scope_check(wd)?;
        if !check_path.starts_with(&wd_canonical) {
            match self.path_access {
                PathAccessPolicy::WorkspaceOnly => {
                    anyhow::bail!(
                        "路径 {} 不在工作目录 {} 范围内；切换到 full_access 后可访问普通外部路径",
                        path,
                        wd.display()
                    );
                }
                PathAccessPolicy::FullAccessWithSensitiveGuards => {
                    if is_sensitive_path(&check_path) {
                        anyhow::bail!("full_access 仍会保护敏感路径，拒绝访问该位置: {}", path);
                    }
                }
            }
        }
    } else if matches!(
        self.path_access,
        PathAccessPolicy::FullAccessWithSensitiveGuards
    ) && is_sensitive_path(&check_path)
    {
        anyhow::bail!("full_access 仍会保护敏感路径，拒绝访问该位置: {}", path);
    }

    Ok(canonical)
}
```

- [ ] **Step 5: Update existing literal contexts**

In `apps/runtime/src-tauri/tests/test_write_file.rs`, update the existing `test_write_file_allows_absolute_nested_path_within_work_dir` context:

```rust
let ctx = ToolContext {
    work_dir: Some(work_dir.clone()),
    path_access: PathAccessPolicy::WorkspaceOnly,
    allowed_tools: None,
    session_id: None,
    task_temp_dir: None,
    execution_caps: None,
    file_task_caps: None,
};
```

Any other literal `ToolContext { ... }` that does not use `..Default::default()` must set `path_access: PathAccessPolicy::WorkspaceOnly` or use `..Default::default()`.

- [ ] **Step 6: Run policy tests**

Run:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml workspace_only_rejects_absolute_path_outside_work_dir -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml full_access_allows_ordinary_absolute_path_outside_work_dir -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml full_access_rejects_sensitive_absolute_path_outside_work_dir -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml relative_paths_still_resolve_under_work_dir_in_full_access -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_write_file_allows_absolute_path_outside_work_dir_in_full_access -- --nocapture
```

Expected: all listed tests pass.

## Task 3: Derive Path Policy From Permission Mode

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/context.rs`
- Modify: `apps/runtime/src-tauri/src/agent/turn_executor.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/direct_dispatch.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`

- [ ] **Step 1: Add permission-aware context builder**

In `apps/runtime/src-tauri/src/agent/context.rs`, add the imports:

```rust
use super::permissions::PermissionMode;
use super::types::{PathAccessPolicy, ToolContext};
```

Replace the existing `use super::types::ToolContext;` import with the two imports above.

Add this helper above `build_tool_context`:

```rust
fn path_access_for_permission_mode(permission_mode: PermissionMode) -> PathAccessPolicy {
    match permission_mode {
        PermissionMode::Unrestricted => PathAccessPolicy::FullAccessWithSensitiveGuards,
        PermissionMode::AcceptEdits | PermissionMode::Default => PathAccessPolicy::WorkspaceOnly,
    }
}
```

Add this new builder:

```rust
pub(crate) fn build_tool_context_with_permission_mode(
    session_id: Option<&str>,
    work_dir: Option<PathBuf>,
    allowed_tools: Option<&[String]>,
    permission_mode: PermissionMode,
) -> Result<ToolContext> {
    let task_temp_dir = match session_id {
        Some(session_id) => Some(build_task_temp_dir(session_id)?),
        None => None,
    };
    Ok(ToolContext {
        work_dir,
        path_access: path_access_for_permission_mode(permission_mode),
        allowed_tools: allowed_tools.map(|tools| tools.to_vec()),
        session_id: session_id.map(str::to_string),
        task_temp_dir,
        execution_caps: Some(detect_execution_caps()),
        file_task_caps: None,
    })
}
```

Change `build_tool_context` into a compatibility wrapper:

```rust
pub(crate) fn build_tool_context(
    session_id: Option<&str>,
    work_dir: Option<PathBuf>,
    allowed_tools: Option<&[String]>,
) -> Result<ToolContext> {
    build_tool_context_with_permission_mode(
        session_id,
        work_dir,
        allowed_tools,
        PermissionMode::Default,
    )
}
```

- [ ] **Step 2: Add context-builder tests**

In `apps/runtime/src-tauri/src/agent/context.rs`, update the test imports:

```rust
use super::{build_tool_context, build_tool_context_with_permission_mode};
use crate::agent::permissions::PermissionMode;
use crate::agent::types::PathAccessPolicy;
```

Add:

```rust
#[test]
fn full_access_context_uses_sensitive_guard_policy() {
    let ctx = build_tool_context_with_permission_mode(
        Some("session-full"),
        None,
        None,
        PermissionMode::Unrestricted,
    )
    .expect("build context");

    assert_eq!(
        ctx.path_access,
        PathAccessPolicy::FullAccessWithSensitiveGuards
    );
}

#[test]
fn standard_context_uses_workspace_only_policy() {
    let ctx = build_tool_context_with_permission_mode(
        Some("session-standard"),
        None,
        None,
        PermissionMode::Default,
    )
    .expect("build context");

    assert_eq!(ctx.path_access, PathAccessPolicy::WorkspaceOnly);
}
```

- [ ] **Step 3: Update runtime call sites**

In `apps/runtime/src-tauri/src/agent/turn_executor.rs`, change the import:

```rust
use super::context::build_tool_context_with_permission_mode;
```

Replace the main context construction:

```rust
let tool_ctx = build_tool_context_with_permission_mode(
    session_id,
    work_dir.map(PathBuf::from),
    allowed_tools,
    permission_mode,
)
.map_err(|error| AgentTurnExecutionError::from_error(error, compaction_outcome.clone()))?;
```

Inside the same file's tests, either call `super::build_tool_context_with_permission_mode(..., PermissionMode::Default)` or import and keep using the wrapper if the test is specifically checking default context construction.

In `apps/runtime/src-tauri/src/agent/runtime/kernel/direct_dispatch.rs`, change the import and call:

```rust
use crate::agent::context::build_tool_context_with_permission_mode;
```

```rust
let tool_ctx = build_tool_context_with_permission_mode(
    Some(session_id),
    execution_context
        .executor_work_dir
        .as_ref()
        .map(std::path::PathBuf::from),
    setup.skill_allowed_tools.as_deref(),
    execution_context.permission_mode,
)
.map_err(|err| err.to_string())?;
```

In `apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs`, change the import and call in `maybe_direct_dispatch_workspace_vision`:

```rust
use crate::agent::context::build_tool_context_with_permission_mode;
```

```rust
let tool_ctx = build_tool_context_with_permission_mode(
    Some(params.session_id),
    params
        .execution_context
        .executor_work_dir
        .as_ref()
        .map(PathBuf::from),
    params.execution_context.allowed_tools(),
    params.execution_context.permission_mode,
)
.map_err(|error| error.to_string())?;
```

In `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`, change the import and call:

```rust
use crate::agent::context::build_tool_context_with_permission_mode;
```

```rust
let tool_ctx = build_tool_context_with_permission_mode(
    Some(session_id),
    execution_context
        .executor_work_dir
        .as_ref()
        .map(PathBuf::from),
    execution_context.allowed_tools(),
    execution_context.permission_mode,
)
.map_err(|err| SkillCommandDispatchError {
    error: err.to_string(),
    skill_id: spec.skill_id.clone(),
})?;
```

- [ ] **Step 4: Fix remaining compiler errors from context literals**

Run:

```powershell
rg -n "ToolContext \\{" apps/runtime/src-tauri/src apps/runtime/src-tauri/tests -g "*.rs"
```

For each literal without `..Default::default()`, add:

```rust
path_access: PathAccessPolicy::WorkspaceOnly,
```

or convert to:

```rust
ToolContext {
    work_dir: Some(path),
    ..Default::default()
}
```

- [ ] **Step 5: Run focused builder and write-file tests**

Run:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml full_access_context_uses_sensitive_guard_policy -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml standard_context_uses_workspace_only_policy -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_write_file -- --nocapture
```

Expected: all listed tests pass.

## Task 4: Update Copy And Verify

**Files:**
- Modify: `apps/runtime/src/components/settings/desktop/DesktopRuntimeSection.tsx`
- Verify: `apps/runtime/src-tauri/src/agent/**/*.rs`
- Verify: `apps/runtime/src-tauri/tests/test_write_file.rs`

- [ ] **Step 1: Update full-access UI copy**

In `apps/runtime/src/components/settings/desktop/DesktopRuntimeSection.tsx`, replace:

```tsx
<span className="mt-1 block text-gray-500">所有操作自动执行，适合可信任务与熟悉环境。</span>
```

with:

```tsx
<span className="mt-1 block text-gray-500">
  所有操作自动执行，文件工具可访问会话目录外的普通路径；敏感路径仍会被保护。
</span>
```

Replace the confirm dialog copy:

```tsx
summary="全自动模式会允许智能体自动执行所有本地操作。"
impact="这会显著降低人工确认频率，适合可信任务与受控环境。"
```

with:

```tsx
summary="全自动模式会允许智能体自动执行本地操作，并让文件工具访问会话目录外的普通路径。"
impact="这会显著降低人工确认频率；敏感路径仍会被保护，但请只在可信任务与受控环境中使用。"
```

- [ ] **Step 2: Format changed Rust code**

Run:

```powershell
cargo fmt --manifest-path apps/runtime/src-tauri/Cargo.toml
```

Expected: exits 0 and only formats touched Rust files.

- [ ] **Step 3: Run focused Rust tests**

Run:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml workspace_only_rejects_absolute_path_outside_work_dir -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml full_access_allows_ordinary_absolute_path_outside_work_dir -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml full_access_rejects_sensitive_absolute_path_outside_work_dir -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml relative_paths_still_resolve_under_work_dir_in_full_access -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_write_file -- --nocapture
```

Expected: all listed tests pass.

- [ ] **Step 4: Run WorkClaw Rust fast path**

Run:

```powershell
pnpm test:rust-fast
```

Expected: exits 0. This covers the shared Tauri runtime surface touched by `ToolContext`, context builders, and file tools.

- [ ] **Step 5: Run a focused frontend test if existing settings tests cover this component**

First inspect available settings tests:

```powershell
rg -n "DesktopRuntimeSection|全自动模式|operation_permission_mode" apps/runtime/src/components apps/runtime/src/__tests__ -g "*.test.tsx" -g "*.test.ts"
```

If an existing test directly renders the settings component or checks the full-access copy, run the smallest matching Vitest command. If no test covers this static copy, record that the UI copy was reviewed but not separately executed.

- [ ] **Step 6: Review final diff**

Run:

```powershell
git diff -- apps/runtime/src-tauri/src/agent apps/runtime/src-tauri/tests/test_write_file.rs apps/runtime/src/components/settings/desktop/DesktopRuntimeSection.tsx
```

Check that:

- `standard` and `accept_edits` still map to workspace-only.
- Only `full_access` / `unrestricted` maps to `FullAccessWithSensitiveGuards`.
- Relative paths still resolve under `work_dir`.
- Sensitive-path denial is centralized and file tools still call `ctx.check_path`.
- No unrelated user changes are reverted.

- [ ] **Step 7: Prepare completion summary**

Use the `$workclaw-change-verification` output shape:

```md
## Verification Summary
- Changed surface:
- Commands run:
- Results:
- Covered areas:
- Still unverified:
- Verification verdict:
```

Mention any frontend static-copy test gap if no focused test exists.
