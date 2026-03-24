# Rust Employee Manage Tool Split Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Shrink `apps/runtime/src-tauri/src/agent/tools/employee_manage.rs` into a thin `Tool` shell by extracting parsing, lookup, and action orchestration into child modules while preserving current behavior.

**Architecture:** Keep the synchronous `Tool` interface in the root file, but move reusable parsing and employee matching into `support.rs` and the action bodies into `actions.rs`. Add `schema.rs` so the root file stops carrying the full schema block inline.

**Tech Stack:** Rust, sqlx, SQLite, WorkClaw runtime tests

---

### Task 1: Create the support module

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/employee_manage/support.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/employee_manage.rs`

**Step 1: Move the parsing and normalization helpers**

- move `parse_string_array`
- move `parse_profile_answers`
- move `parse_optional_string`
- move `parse_optional_bool`
- move `dedupe_skill_ids`
- move `resolve_employee`
- move `normalize_employee_id`
- move `default_employee_work_dir`

**Step 2: Keep the root tool shell compiling**

- re-export the helper functions only as needed by the root file and action module

**Step 3: Verify a focused existing test**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --nocapture
```

Expected: PASS

### Task 2: Create the action module

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/employee_manage/actions.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/employee_manage.rs`

**Step 1: Move the action bodies**

- move `list_skills`
- move `list_employees`
- move `create_employee`
- move `update_employee`
- move `apply_profile`

**Step 2: Keep user-visible behavior stable**

- preserve `DEFAULT_PRIMARY_SKILL_ID`
- preserve `enabled_scopes` defaults
- preserve profile auto-apply behavior
- preserve employee resolution behavior

**Step 3: Verify the employee tool tests**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --nocapture
```

Expected: PASS

### Task 3: Extract the schema block

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/employee_manage/schema.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/employee_manage.rs`

**Step 1: Move the `input_schema` implementation**

- keep the same action enum and field descriptions
- no contract changes

**Step 2: Keep the root file as a shell**

- root should mostly wire `Tool` methods to child modules

### Task 4: Optional test move if root remains crowded

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/employee_manage/tests.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/employee_manage.rs`

**Step 1: Move the test module only if needed**

- move the existing tests unchanged
- keep them focused on behavior rather than module shape

### Task 5: Verify the full split

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/employee_manage.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/employee_manage/support.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/employee_manage/actions.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/employee_manage/schema.rs`

**Step 1: Run the employee tool tests**

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --nocapture
```

**Step 2: Run Rust fast verification**

```bash
pnpm test:rust-fast
```

**Step 3: Check remaining size**

```bash
(Get-Content apps/runtime/src-tauri/src/agent/tools/employee_manage.rs | Measure-Object -Line).Lines
```
