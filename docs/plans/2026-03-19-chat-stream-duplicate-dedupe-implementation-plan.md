# Chat Stream Duplicate Dedupe Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Stop duplicated assistant sentences from appearing during streaming, in persisted assistant history, and in session export recovery output.

**Architecture:** Tighten the chat pipeline at two boundaries. First, make the runtime UI treat repeated or cumulative `stream-token` payloads as idempotent updates instead of blindly appending them. Second, normalize persisted assistant content so tool-call-backed assistant messages do not store the same text twice, which also improves export-time dedupe.

**Tech Stack:** React, TypeScript, Vitest, Rust, Tokio tests

---

### Task 1: Frontend streaming dedupe

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.thinking-block.test.tsx`
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: Write the failing test**

Add a test that emits the same `stream-token` text twice for one session and asserts the visible streaming text still appears once.

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- ChatView.thinking-block.test.tsx`

Expected: FAIL because duplicated stream payloads currently render repeated assistant text.

**Step 3: Write minimal implementation**

Add a small helper that compares the incoming token to the current trailing streamed text and only appends the non-overlapping suffix. Keep existing pure-delta behavior unchanged.

**Step 4: Run test to verify it passes**

Run: `pnpm --filter runtime test -- ChatView.thinking-block.test.tsx`

Expected: PASS

### Task 2: Backend assistant-content dedupe

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_runtime_io.rs`

**Step 1: Write the failing test**

Add a unit test for `build_assistant_content_from_final_messages` proving that an assistant message with both `content` text and `tool_calls` does not produce duplicated text items.

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml build_assistant_content_from_final_messages -- --nocapture`

Expected: FAIL because the current implementation pushes the same text into ordered items twice when `tool_calls` are present.

**Step 3: Write minimal implementation**

Only insert assistant text into ordered items once per assistant message while preserving tool-call ordering and final text extraction.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml build_assistant_content_from_final_messages -- --nocapture`

Expected: PASS

### Task 3: Focused verification

**Files:**
- Verify only

**Step 1: Run frontend coverage for touched runtime flow**

Run: `pnpm --filter runtime test -- ChatView.thinking-block.test.tsx`

**Step 2: Run Rust coverage for touched assistant content builder**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml build_assistant_content -- --nocapture`

**Step 3: Optional follow-up if export path still looks risky**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_session_export_recovery -- --nocapture`

**Step 4: Report verification honestly**

Document which duplicate paths are covered directly and whether export recovery remains indirectly covered or explicitly tested.
