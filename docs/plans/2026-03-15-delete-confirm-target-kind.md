# Delete Confirm Target Kind Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the `file_delete` high-risk confirmation describe files, folders, and recursive folder deletes accurately.

**Architecture:** Keep the fix in the Rust backend where confirmation payloads are generated. Resolve the target path using the current work directory, inspect filesystem metadata, then build title/summary/impact text from the detected target kind. Cover the behavior with focused unit tests in the same module.

**Tech Stack:** Rust, Tauri backend, serde_json, std::fs

---

### Task 1: Add failing tests for confirmation wording

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Test: `apps/runtime/src-tauri/src/agent/executor.rs`

**Step 1: Write the failing test**

Add unit tests that call `critical_action_summary` for:
- an existing file target
- an existing empty directory target
- an existing directory target with `recursive: true`
- a missing target

**Step 2: Run test to verify it fails**

Run: `cargo test file_delete_confirmation --package runtime`
Expected: FAIL because the current code always returns `删除文件`.

**Step 3: Write minimal implementation**

Add a small helper that:
- resolves `path` against `work_dir` when relative
- detects `file | directory | unknown`
- chooses accurate user-facing text for the confirmation payload

**Step 4: Run test to verify it passes**

Run: `cargo test file_delete_confirmation --package runtime`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/executor.rs docs/plans/2026-03-15-delete-confirm-target-kind-design.md docs/plans/2026-03-15-delete-confirm-target-kind.md
git commit -m "fix(runtime): clarify delete confirmation target kind"
```

### Task 2: Verify no regression in existing confirmation flow

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Test: `apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx`

**Step 1: Review existing front-end confirmation assumptions**

Confirm the React test only depends on dialog rendering, not the hardcoded `删除文件` title from backend generation.

**Step 2: Run targeted verification**

Run: `pnpm test -- ChatView.risk-flow.test.tsx`
Expected: PASS

**Step 3: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/executor.rs
git commit -m "test(runtime): cover delete confirmation target kinds"
```
