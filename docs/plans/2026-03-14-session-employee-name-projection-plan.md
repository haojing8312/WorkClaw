# Session Employee Name Projection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `employee_name` to session list payloads so employee-direct sessions naturally expose the bound employee identity to the frontend.

**Architecture:** Reuse the existing employee lookup already built inside `chat_session_io::list_sessions_with_pool`, and include the matched employee name in each returned session JSON object. Keep `display_title` derivation unchanged and verify behavior with a focused Rust regression test.

**Tech Stack:** Rust, sqlx, serde_json, Tokio tests

---

### Task 1: Lock the regression with a failing backend test

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_session_io.rs`

**Step 1: Write the failing test**

Add an assertion in the existing session-list projection test that an employee-direct session returns `"employee_name": "张三"`.

**Step 2: Run test to verify it fails**

Run: `cargo test list_sessions_with_pool_derives_display_title_for_general_sessions`

Expected: FAIL because `employee_name` is missing from the returned JSON.

### Task 2: Implement the minimal payload change

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_session_io.rs`

**Step 1: Write minimal implementation**

Use the already-built `employee_name_by_code` map to resolve the session employee name and emit it as `employee_name` in the session JSON payload.

**Step 2: Run test to verify it passes**

Run: `cargo test list_sessions_with_pool_derives_display_title_for_general_sessions`

Expected: PASS.

### Task 3: Verify related session projection coverage

**Files:**
- Verify: `apps/runtime/src-tauri/src/commands/chat_session_io.rs`

**Step 1: Run focused verification**

Run: `cargo test list_sessions_with_pool`

Expected: PASS for all `list_sessions_with_pool` tests.
