# WorkClaw Single-App Merge Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Merge Studio + Runtime into one user-facing WorkClaw app with zero data loss, no feature regression, and non-technical UX naming.

**Architecture:** Keep one Tauri app (`apps/runtime`) as the only client, migrate Studio packaging UI/commands into it, and preserve storage compatibility via mapping layers. Use phased delivery: capability merge, UX renaming, migration hardening, cleanup.

**Tech Stack:** Tauri 2 (Rust), React 18 + TypeScript, SQLite (sqlx), `packages/skillpack-rs`, pnpm workspace

---

### Task 1: Add Packaging Commands Into Runtime Backend

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/packaging.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_packaging_commands.rs`

**Step 1: Write the failing test**

```rust
use runtime_lib::commands::packaging::read_skill_dir;

#[tokio::test]
async fn read_skill_dir_requires_skill_md() {
    let tmp = tempfile::tempdir().unwrap();
    let err = read_skill_dir(tmp.path().to_string_lossy().to_string()).await.unwrap_err();
    assert!(err.contains("SKILL.md"));
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_packaging_commands -- --nocapture`  
Expected: FAIL because module/command is missing.

**Step 3: Write minimal implementation**

Implement `read_skill_dir` and `pack_skill` in `commands/packaging.rs` by porting `apps/studio/src-tauri/src/commands.rs` logic and returning `Result<_, String>`.

**Step 4: Register commands**

- Export module in `commands/mod.rs`
- Add `commands::packaging::read_skill_dir` and `commands::packaging::pack_skill` in `lib.rs` `invoke_handler`.

**Step 5: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test test_packaging_commands -- --nocapture`  
Expected: PASS.

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/packaging.rs apps/runtime/src-tauri/src/commands/mod.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_packaging_commands.rs
git commit -m "feat(app): add packaging commands into single runtime backend"
```

### Task 2: Add Packaging Page Into Runtime Frontend

**Files:**
- Create: `apps/runtime/src/components/packaging/PackagingView.tsx`
- Create: `apps/runtime/src/components/packaging/PackForm.tsx`
- Create: `apps/runtime/src/components/packaging/FileTree.tsx`
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/components/Sidebar.tsx`

**Step 1: Write the failing UI contract test (or smoke assertion)**

If no UI test framework exists, add a build-time smoke criterion in plan execution: the new components compile and route render.

**Step 2: Run frontend build to verify baseline**

Run: `cd apps/runtime && pnpm run build`  
Expected: PASS before changes.

**Step 3: Port Studio packaging UI**

- Port `FileTree` and `PackForm` from `apps/studio/src/components/`.
- Create `PackagingView.tsx` to orchestrate directory select + command invoke.
- In `App.tsx`, add top-level mode/state to render Packaging page.
- In `Sidebar.tsx`, add entry button/tab labeled `打包`.

**Step 4: Run frontend build to verify it passes**

Run: `cd apps/runtime && pnpm run build`  
Expected: PASS with new Packaging page.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/packaging/PackagingView.tsx apps/runtime/src/components/packaging/PackForm.tsx apps/runtime/src/components/packaging/FileTree.tsx apps/runtime/src/App.tsx apps/runtime/src/components/Sidebar.tsx
git commit -m "feat(app-ui): add integrated packaging view to single app"
```

### Task 3: Rename User-Facing Terms For Non-Technical UX

**Files:**
- Modify: `apps/runtime/src/components/Sidebar.tsx`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`

**Step 1: Write failing unit tests for permission mode mapping**

Add tests in `chat.rs` module for display mapping:
- `accept_edits` -> `推荐模式`
- `default` -> `谨慎模式`
- `unrestricted` -> `全自动模式（高风险）`

**Step 2: Run tests to verify fail**

Run: `cd apps/runtime/src-tauri && cargo test permission_mode -- --nocapture`  
Expected: FAIL until mapping helpers are added.

**Step 3: Implement mapping layer**

- Keep storage enum values unchanged.
- Add display label helper functions for frontend payload (or UI-only map in TS).
- Replace user-visible strings: `新会话权限模式` -> `操作确认级别`; replace option labels with non-technical text.

**Step 4: Verify**

Run:
- `cd apps/runtime/src-tauri && cargo test permission_mode -- --nocapture`
- `cd apps/runtime && pnpm run build`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/Sidebar.tsx apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/SettingsView.tsx apps/runtime/src/types.ts apps/runtime/src-tauri/src/commands/chat.rs
git commit -m "feat(ux): replace technical permission wording with user-friendly labels"
```

### Task 4: Guarantee No Data Loss With Compatibility Tests

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Create: `apps/runtime/src-tauri/tests/test_single_app_migration.rs`
- Modify: `apps/runtime/src-tauri/tests/test_permissions.rs`

**Step 1: Write failing migration tests**

Test scenarios:
- Existing `sessions.permission_mode='accept_edits'` still parse correctly.
- Missing new optional tables does not break startup.
- New migrations are idempotent.

**Step 2: Run tests to verify fail**

Run: `cd apps/runtime/src-tauri && cargo test single_app_migration -- --nocapture`  
Expected: FAIL before migration guards.

**Step 3: Add migration hardening**

- Add idempotent SQL for any new tables.
- Keep legacy columns/values untouched.
- Ensure startup tolerates partial schema state.

**Step 4: Run tests to verify pass**

Run: `cd apps/runtime/src-tauri && cargo test single_app_migration -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/tests/test_single_app_migration.rs apps/runtime/src-tauri/tests/test_permissions.rs
git commit -m "test(db): harden single-app migration and legacy compatibility"
```

### Task 5: Deprecate Studio From Workspace Without Breaking Build

**Files:**
- Modify: `package.json`
- Modify: `pnpm-workspace.yaml`
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/diagrams/technical-architecture.svg` (or source `.excalidraw` then regenerate)
- Modify: `docs/diagrams/business-architecture.svg` (or source `.excalidraw` then regenerate)

**Step 1: Update scripts/docs first (non-destructive)**

- Remove Studio run/build scripts.
- Update docs text from dual-app to single-app.

**Step 2: Verify repo build commands**

Run:
- `pnpm install`
- `pnpm --filter runtime build`

Expected: PASS without Studio scripts.

**Step 3: Commit docs/config cleanup**

```bash
git add package.json pnpm-workspace.yaml README.md README.zh-CN.md docs/diagrams/technical-architecture.svg docs/diagrams/business-architecture.svg
git commit -m "chore(docs): switch architecture and scripts to single WorkClaw app"
```

### Task 6: Remove Studio App After All Tests Pass

**Files:**
- Delete: `apps/studio/` (all files)

**Step 1: Ensure no references remain**

Run: `rg -n "apps/studio|\\bstudio\\b" . -S`  
Expected: only historical docs allowed (or zero if fully cleaned).

**Step 2: Delete directory**

Run: `rmdir /s /q apps\\studio` (Windows)

**Step 3: Verify workspace/build/tests**

Run:
- `pnpm install`
- `cd apps/runtime && pnpm run build`
- `cd apps/runtime/src-tauri && cargo test`

Expected: all PASS.

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor(app): remove studio and finalize single-app architecture"
```

### Task 7: End-to-End Regression and Release Checklist

**Files:**
- Create: `docs/plans/2026-02-28-single-app-merge-release-checklist.md`
- Modify: `docs/plans/2026-02-28-single-app-merge-design.md`
- Modify: `docs/plans/2026-02-28-single-app-merge.md`

**Step 1: Create explicit checklist**

Include:
- Existing user data visible after upgrade
- Install skill flow works
- Package skill flow works
- Package -> install -> chat loop works
- User-visible wording has no forbidden technical terms

**Step 2: Manual smoke run**

Run app with real local data and execute checklist.

**Step 3: Final verification**

Run:
- `cd apps/runtime && pnpm run build`
- `cd apps/runtime/src-tauri && cargo test`

Expected: PASS.

**Step 4: Commit**

```bash
git add docs/plans/2026-02-28-single-app-merge-release-checklist.md docs/plans/2026-02-28-single-app-merge-design.md docs/plans/2026-02-28-single-app-merge.md
git commit -m "docs(release): add single-app merge verification checklist"
```
