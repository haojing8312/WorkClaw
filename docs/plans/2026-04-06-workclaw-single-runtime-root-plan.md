# WorkClaw Single Runtime Root Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Move all desktop runtime data under one user-configurable root directory with startup-safe migration, rollback, and a simplified desktop settings UI.

**Architecture:** Add a bootstrap-owned runtime root model that resolves before diagnostics and database startup, then migrate legacy and user-selected roots through an early-startup transaction. Replace scattered `app_data_dir` / `app_cache_dir` / `app_log_dir` usage with a unified path layer, and simplify the settings UI to one root-directory control that schedules migration and restarts the app.

**Tech Stack:** Rust, Tauri 2, React, TypeScript, SQLite, filesystem migration helpers, Vitest, Rust unit tests, Rust integration tests.

---

### Task 1: Add failing tests for runtime-root bootstrap discovery

**Files:**
- Create: `apps/runtime/src-tauri/src/runtime_bootstrap.rs`
- Test: `apps/runtime/src-tauri/src/runtime_bootstrap.rs`

**Step 1: Write the failing tests**

Add tests for:

- default bootstrap creation when no file exists
- parsing an existing bootstrap file with `current_root`
- rejecting malformed bootstrap content and falling back safely
- upgrade discovery that prefers bootstrap over legacy directories

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime_lib runtime_bootstrap`

Expected: FAIL because the bootstrap module does not exist yet.

**Step 3: Write minimal implementation**

Create a bootstrap module that can:

- resolve the stable bootstrap file location
- read and write bootstrap JSON
- initialize default bootstrap state
- expose typed bootstrap records for pending migration and last migration results

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime_lib runtime_bootstrap`

Expected: PASS.

### Task 2: Add failing tests for unified runtime-path derivation

**Files:**
- Create: `apps/runtime/src-tauri/src/runtime_paths.rs`
- Test: `apps/runtime/src-tauri/src/runtime_paths.rs`

**Step 1: Write the failing tests**

Add tests proving:

- the default root resolves to `%USERPROFILE%\\.workclaw` on Windows-style environments
- all derived paths resolve under one root
- the default workspace is always `<root>/workspace`
- path-validation rejects nested migration targets

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime_lib runtime_paths`

Expected: FAIL because unified runtime path derivation is not implemented.

**Step 3: Write minimal implementation**

Create `RuntimePaths` and related helpers that expose:

- root
- database files
- diagnostics tree
- cache tree
- sessions tree
- plugin state directories
- workspace directory

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime_lib runtime_paths`

Expected: PASS.

### Task 3: Add failing tests for runtime-root migration scheduling

**Files:**
- Modify: `apps/runtime/src-tauri/src/runtime_bootstrap.rs`
- Create: `apps/runtime/src-tauri/src/runtime_root_migration.rs`
- Test: `apps/runtime/src-tauri/src/runtime_root_migration.rs`

**Step 1: Write the failing tests**

Add tests for:

- scheduling a migration from current root to a new root
- refusing an empty or non-writable target
- refusing parent/child nested target roots
- refusing to schedule a second migration while one is pending

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime_lib runtime_root_migration schedule`

Expected: FAIL because migration scheduling does not exist.

**Step 3: Write minimal implementation**

Add helpers that validate a target root and persist `pending_migration` into bootstrap.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime_lib runtime_root_migration schedule`

Expected: PASS.

### Task 4: Add failing tests for migration execution and rollback

**Files:**
- Modify: `apps/runtime/src-tauri/src/runtime_root_migration.rs`
- Test: `apps/runtime/src-tauri/src/runtime_root_migration.rs`

**Step 1: Write the failing tests**

Add tests proving:

- managed runtime files move or copy into the target root
- database files, diagnostics, cache, session journals, and plugin directories are all migrated
- interrupted or failed migration restores the old root in bootstrap
- successful migration records `previous_root` and completion metadata

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime_lib runtime_root_migration execute`

Expected: FAIL because execution, verification, and rollback are not implemented.

**Step 3: Write minimal implementation**

Implement migration execution that:

- marks pending migrations `in_progress`
- copies or moves managed paths
- validates the target root
- updates bootstrap on success
- restores bootstrap on failure

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime_lib runtime_root_migration execute`

Expected: PASS.

### Task 5: Add failing tests for startup adoption of the unified root

**Files:**
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/src/diagnostics.rs`
- Test: `apps/runtime/src-tauri/tests/test_runtime_root_startup.rs`

**Step 1: Write the failing tests**

Add integration-style tests proving:

- startup initializes diagnostics and database under the unified root
- legacy layouts are discovered and adopted when bootstrap is absent
- startup can complete a pending migration before database initialization

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime_lib test_runtime_root_startup`

Expected: FAIL because startup still uses system directories directly.

**Step 3: Write minimal implementation**

Adjust startup to:

- load bootstrap
- complete or recover migrations
- construct `RuntimePaths`
- initialize diagnostics, database, and runtime state from those paths

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime_lib test_runtime_root_startup`

Expected: PASS.

### Task 6: Add failing tests for runtime subsystems that still read direct system paths

**Files:**
- Modify: `apps/runtime/src-tauri/src/session_journal.rs`
- Modify: `apps/runtime/src-tauri/src/commands/desktop_lifecycle.rs`
- Modify: `apps/runtime/src-tauri/src/commands/desktop_lifecycle/filesystem.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins/setup_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins/plugin_host_service.rs`
- Modify: any additional runtime module still using `app_data_dir`, `app_cache_dir`, or `app_log_dir`

**Step 1: Write the failing tests**

Add or extend tests proving:

- session journals now live below the unified root
- plugin state and shim state live below the unified root
- desktop lifecycle APIs report paths derived from the unified root
- cache and diagnostics cleanup acts on unified-root paths only

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime_lib desktop_lifecycle`

Expected: FAIL where modules still derive paths directly from Tauri system directories.

**Step 3: Write minimal implementation**

Replace direct system path calls with `RuntimePaths` in all touched runtime modules.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime_lib desktop_lifecycle`

Expected: PASS.

### Task 7: Add failing frontend tests for the simplified desktop settings surface

**Files:**
- Modify: `apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.desktop-system-tab.test.tsx`
- Create or modify: `apps/runtime/src/components/__tests__/SettingsView.runtime-root-migration.test.tsx`

**Step 1: Write the failing tests**

Add tests proving:

- the desktop settings page only shows one runtime-root directory card
- legacy path sections are gone
- selecting a new root shows a pending migration confirmation state
- success and failure status banners render correctly from backend state

**Step 2: Run test to verify it fails**

Run: `pnpm vitest apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx apps/runtime/src/components/__tests__/SettingsView.desktop-system-tab.test.tsx`

Expected: FAIL because the current UI still shows multiple directory sections.

**Step 3: Write minimal implementation**

Update the desktop settings UI and service layer to:

- load unified runtime-root status
- select a directory
- schedule migration
- trigger restart
- display success / failure migration feedback

**Step 4: Run test to verify it passes**

Run: `pnpm vitest apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx apps/runtime/src/components/__tests__/SettingsView.desktop-system-tab.test.tsx`

Expected: PASS.

### Task 8: Add Tauri commands for runtime-root status, migration scheduling, and restart

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/desktop_lifecycle.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Modify: `apps/runtime/src/components/settings/desktop/desktopSettingsService.ts`

**Step 1: Write the failing tests**

Add backend and frontend-facing tests covering:

- get active runtime root and last migration result
- schedule migration to a new root
- reject invalid root targets
- request app restart after scheduling

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime_lib runtime_root_migration_command`

Expected: FAIL because these commands do not exist.

**Step 3: Write minimal implementation**

Add commands that:

- return active runtime-root status
- persist a pending migration
- expose a restart command or restart hook for the frontend flow

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime_lib runtime_root_migration_command`

Expected: PASS.

### Task 9: Run focused verification for the changed frontend and Rust surfaces

**Files:**
- Verify touched runtime-root files and desktop settings tests above

**Step 1: Run targeted frontend tests**

Run: `pnpm vitest apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx apps/runtime/src/components/__tests__/SettingsView.desktop-system-tab.test.tsx apps/runtime/src/components/__tests__/SettingsView.runtime-root-migration.test.tsx`

Expected: PASS.

**Step 2: Run Rust fast path**

Run: `pnpm test:rust-fast`

Expected: PASS.

**Step 3: Record covered areas**

Document that verification covered:

- bootstrap discovery
- unified runtime path derivation
- migration scheduling
- migration execution and rollback
- startup adoption
- desktop settings UI simplification

**Step 4: Note remaining gaps**

Document any still-unverified area, especially real Windows restart behavior and long-running migration timing with large user datasets.
