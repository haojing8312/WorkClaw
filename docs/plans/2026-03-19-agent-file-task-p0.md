# Agent File Task P0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a lightweight Tauri-agent strategy layer that preflights file reads, exposes task-scoped temp state, and marks skills as executable vs instruction-only so the agent stops entering uncontrolled tool retries.

**Architecture:** Extend `ToolContext` with lightweight execution metadata, inject that metadata from the executor, and update a few high-leverage tools (`read_file`, `skill_invoke`, `bash`, `write_file`) to consume it. Keep the implementation local to the runtime Tauri agent layer and avoid sidecar or frontend work.

**Tech Stack:** Rust, Tauri runtime agent tools, serde_json, tempfile/path utilities, existing WorkClaw tool/result patterns.

---

### Task 1: Extend ToolContext For P0 Metadata

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/types.rs`
- Test: `apps/runtime/src-tauri/src/agent/executor.rs`

**Step 1: Write the failing test**

Add or update a small unit test around tool-context construction to assert the new metadata fields are present when the executor builds a context.

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime-app tool_context`
Expected: FAIL because the new fields/types do not exist yet.

**Step 3: Write minimal implementation**

Add the new `ToolContext` fields and small supporting structs:

- `session_id: Option<String>`
- `task_temp_dir: Option<PathBuf>`
- `execution_caps: Option<ExecutionCaps>`
- `file_task_caps: Option<FileTaskCaps>`

Keep defaults compatible.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime-app tool_context`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/types.rs apps/runtime/src-tauri/src/agent/executor.rs
git commit -m "feat: extend agent tool context with execution metadata"
```

### Task 2: Add File Task Preflight Helper

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/file_task_preflight.rs`
- Modify: `apps/runtime/src-tauri/src/agent/mod.rs` or relevant module export file
- Test: `apps/runtime/src-tauri/src/agent/file_task_preflight.rs`

**Step 1: Write the failing test**

Add tests for:

- `.txt` => `text_direct`
- `.docx` => `binary_or_office`
- missing file => `missing`

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime-app file_task_preflight`
Expected: FAIL because the helper module does not exist.

**Step 3: Write minimal implementation**

Implement a small helper that:

- resolves path through existing `ToolContext::check_path`-compatible logic
- infers read mode from extension
- returns a small `FileTaskCaps`

Do not add MIME sniffing or complex binary detection in this P0.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime-app file_task_preflight`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/file_task_preflight.rs apps/runtime/src-tauri/src/agent/mod.rs
git commit -m "feat: add agent file-task preflight helper"
```

### Task 3: Inject Temp Directory And Execution Caps From Executor

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Create: `apps/runtime/src-tauri/src/agent/execution_caps.rs`
- Test: `apps/runtime/src-tauri/src/agent/executor.rs`

**Step 1: Write the failing test**

Add a targeted test asserting executor-built context contains:

- a WorkClaw-prefixed temp directory
- basic execution caps for the current platform

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime-app executor`
Expected: FAIL because no such metadata is injected today.

**Step 3: Write minimal implementation**

Implement:

- a small helper to compute `ExecutionCaps`
- temp-dir creation during context setup
- propagation into `ToolContext`

Keep capability detection cheap and static for P0.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime-app executor`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/executor.rs apps/runtime/src-tauri/src/agent/execution_caps.rs
git commit -m "feat: inject task temp dir and execution caps into tool context"
```

### Task 4: Make read_file Fail Cleanly On Office/Binary Files

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/read_file.rs`
- Test: `apps/runtime/src-tauri/src/agent/tools/read_file.rs` or adjacent test module

**Step 1: Write the failing test**

Add tests for:

- reading a normal UTF-8 text file succeeds
- reading a `.docx` file path returns structured failure with an explicit unsupported raw-read reason

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime-app read_file`
Expected: FAIL because `.docx` currently tries `read_to_string`.

**Step 3: Write minimal implementation**

Use the preflight result before `read_to_string`:

- `text_direct` => existing success path
- `binary_or_office` => `tool_result::failure(...)`

Do not add parsing; only improve classification and failure mode.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime-app read_file`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/read_file.rs
git commit -m "fix: classify office files before raw text reads"
```

### Task 5: Mark Skill Executability Explicitly

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs`
- Test: `apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs`

**Step 1: Write the failing test**

Add tests for:

- skill with no declared tools => `instruction_only`
- skill with declared tools and narrowed tools => `executable`
- skill with declared tools but none available => `blocked`

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime-app skill_invoke`
Expected: FAIL because no structured executability status exists.

**Step 3: Write minimal implementation**

Add a helper inside `skill_invoke.rs` that computes status and reason, then include that status in the returned content summary.

Preserve current human-readable output while adding explicit status lines.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime-app skill_invoke`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs
git commit -m "feat: expose skill executability in skill invoke results"
```

### Task 6: Surface Temp/Capability Context In Shell Tool

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/bash.rs`
- Test: `apps/runtime/src-tauri/src/agent/tools/bash.rs`

**Step 1: Write the failing test**

Add a test that checks returned `details` include basic execution metadata when a command is executed in context.

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime-app bash`
Expected: FAIL because the metadata is not present.

**Step 3: Write minimal implementation**

Include non-invasive metadata in the result payload:

- platform shell
- current work dir
- task temp dir if present

Do not redesign `bash` input schema yet.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime-app bash`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/bash.rs
git commit -m "chore: expose execution context metadata in bash tool results"
```

### Task 7: Verify End-To-End P0 Behavior

**Files:**
- Modify: only as needed from previous tasks
- Test: relevant Rust test targets

**Step 1: Run focused tests**

Run:

```bash
cargo test -p runtime-app file_task_preflight
cargo test -p runtime-app read_file
cargo test -p runtime-app skill_invoke
cargo test -p runtime-app bash
```

Expected: PASS

**Step 2: Run broader agent tool tests**

Run:

```bash
cargo test -p runtime-app agent
```

Expected: PASS or known unrelated failures documented.

**Step 3: Check for formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS

**Step 4: Commit final integration**

```bash
git add apps/runtime/src-tauri/src/agent docs/plans
git commit -m "feat: add p0 guardrails for file-task agent execution"
```

