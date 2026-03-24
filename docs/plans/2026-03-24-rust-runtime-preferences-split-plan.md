# Rust Runtime Preferences Split Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Split `apps/runtime/src-tauri/src/commands/runtime_preferences.rs` into thin command wrappers plus focused `types`, `repo`, `service`, and `autostart` child modules without changing runtime preference behavior.

**Architecture:** Move constants and DTOs into `types.rs`, database access into `repo.rs`, normalization and preference orchestration into `service.rs`, and platform autostart synchronization into `autostart.rs`. Keep the root module as a stable Tauri entrypoint that delegates to these child modules.

**Tech Stack:** Rust, Tauri commands, sqlx, SQLite, WorkClaw runtime tests

---

### Task 1: Create the type and repo module skeletons

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/runtime_preferences/types.rs`
- Create: `apps/runtime/src-tauri/src/commands/runtime_preferences/repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/runtime_preferences.rs`

**Step 1: Move preference DTOs and constants**

- Move `RuntimePreferences`, `RuntimePreferencesInput`, preference key constants, defaults, and the autostart app name into `types.rs`
- Re-export them from the root module if needed

**Step 2: Move app-setting SQL helpers**

- Move `get_app_setting` and `set_app_setting` into `repo.rs`
- Keep the query strings and semantics unchanged

**Step 3: Rewire the root module**

- Import the moved items back into `runtime_preferences.rs`
- Keep the public command signatures unchanged

### Task 2: Create the service module

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/runtime_preferences/service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/runtime_preferences.rs`

**Step 1: Move normalization and preference orchestration**

- Move path normalization helpers
- Move `get_runtime_preferences_with_pool`
- Move `set_runtime_preferences_with_pool`
- Move `resolve_default_work_dir_with_pool`

**Step 2: Keep semantics stable**

- Preserve all default values and fallback behavior
- Preserve partial update semantics
- Preserve empty `default_work_dir` rejection

**Step 3: Verify focused runtime preference tests**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_runtime_preferences -- --nocapture
```

Expected: PASS

### Task 3: Create the autostart module

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/runtime_preferences/autostart.rs`
- Modify: `apps/runtime/src-tauri/src/commands/runtime_preferences.rs`

**Step 1: Move autostart logic**

- Move `sync_launch_at_login`
- Move platform-specific helpers such as `format_windows_command_failure` and `resolve_home_dir`

**Step 2: Keep command behavior unchanged**

- `set_runtime_preferences` should still call autostart sync after the database write completes

**Step 3: Verify the runtime preference tests again**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_runtime_preferences -- --nocapture
```

Expected: PASS

### Task 4: Thin the root command module

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/runtime_preferences.rs`

**Step 1: Keep only command wrappers and minimal compatibility glue**

- `get_runtime_preferences`
- `set_runtime_preferences`
- `resolve_default_work_dir`

**Step 2: Remove duplicated logic from root**

- Do not keep normalization, app-setting SQL, or autostart internals in the root file

**Step 3: Re-run the Rust fast path**

Run:

```bash
pnpm test:rust-fast
```

Expected: PASS
