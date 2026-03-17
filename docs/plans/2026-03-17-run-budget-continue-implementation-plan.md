# Run Budget Continue Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Raise default run budgets to 100 turns and allow users to continue a `max_turns` stopped run by granting another 100 turns each time they continue.

**Architecture:** Update the shared Rust run-budget defaults, thread an optional `maxIterations` override through the existing `send_message` path, and teach the React chat view to detect `max_turns` continuation intents and surface a continue action on stopped run cards.

**Tech Stack:** Rust (Tauri, serde), React + TypeScript, Vitest, Cargo unit tests

---

### Task 1: Raise Shared Budget Defaults

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/run_guard.rs`
- Test: `apps/runtime/src-tauri/src/agent/run_guard.rs`

**Step 1: Write the failing test**

Update budget-default tests to assert `100` for the current scopes.

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml run_budget_policy_defaults_general_chat_to_12_turns -- --nocapture`

Expected: FAIL after renaming or updating the assertion to `100`.

**Step 3: Write minimal implementation**

Change the current default `max_turns` values in `RunBudgetPolicy::for_scope`.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml run_budget_policy -- --nocapture`

Expected: PASS

### Task 2: Thread Per-Run Max Iteration Overrides Through send_message

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs`
- Test: `apps/runtime/src-tauri/src/commands/chat.rs`

**Step 1: Write the failing test**

Add a focused request-shape test for `SendMessageRequest` with optional `maxIterations`.

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml send_message_request -- --nocapture`

Expected: FAIL because the request has no `maxIterations` field yet.

**Step 3: Write minimal implementation**

Add optional `max_iterations` to `SendMessageRequest`, plumb it into `PrepareSendMessageParams`, and have `prepare_send_message_context` prefer the request override when present.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml send_message_request -- --nocapture`

Expected: PASS

### Task 3: Add Frontend Continuation Detection And Continue Action

**Files:**
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.run-guardrails.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Write the failing test**

Add tests that verify:

- the stopped `max_turns` card shows `继续执行`
- clicking it sends `send_message` with `maxIterations: 100`
- typing `继续` after a latest `max_turns` stop also sends `maxIterations: 100`

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- --run src/components/__tests__/ChatView.run-guardrails.test.tsx src/components/__tests__/ChatView.session-resilience.test.tsx`

Expected: FAIL because no continuation override exists yet.

**Step 3: Write minimal implementation**

Extend `SendMessageRequest` in TypeScript, detect whether the latest run ended with `max_turns`, and apply `maxIterations: 100` for continuation sends. Render a continue button on the failure card for `max_turns`.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime test -- --run src/components/__tests__/ChatView.run-guardrails.test.tsx src/components/__tests__/ChatView.session-resilience.test.tsx`

Expected: PASS

### Task 4: Verify End-To-End Budget Handling

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_policy.rs`
- Test: `apps/runtime/src-tauri/src/commands/chat_policy.rs`

**Step 1: Write the failing test**

Update any hard-coded structured stop-reason fixtures that still say `12`.

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml chat_policy -- --nocapture`

Expected: FAIL if stale literals remain.

**Step 3: Write minimal implementation**

Update fixtures and any user-facing `max_turns` details that are asserted in tests.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml chat_policy -- --nocapture`

Expected: PASS
