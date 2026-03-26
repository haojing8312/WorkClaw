# Network Retry And Session Resilience Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make network errors respect the existing route retry setting while guaranteeing a minimum of 5 retries, and keep prior session content visible when runtime reloads fail.

**Architecture:** Adjust the shared runtime failover retry budget so only network-classified failures gain the higher retry floor. Preserve frontend in-memory chat/session-run state on reload failures, and keep the existing "continue" path as the recovery mechanism after connectivity is restored.

**Tech Stack:** Rust, Tauri, React, TypeScript, Vitest

---

### Task 1: Lock Retry Floor Behavior

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/failover.rs`

**Step 1: Write the failing test**

Add a test proving a network failure gets retried 5 times even when `per_candidate_retry_count` is `0`.

**Step 2: Run test to verify it fails**

Run: `cargo test -p runtime-tauri failover::tests::execute_candidates_guarantees_five_network_retries`

Expected: FAIL because the retry budget is still below 5.

**Step 3: Write minimal implementation**

Raise the runtime network retry floor from `1` to `5` without changing other error classes.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime-tauri failover::tests::execute_candidates_guarantees_five_network_retries`

Expected: PASS

### Task 2: Keep Session State On Reload Errors

**Files:**
- Modify: `apps/runtime/src/scenes/chat/useChatSessionController.ts`
- Modify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Write the failing test**

Add a chat view test that loads initial messages and session runs, then simulates a failed reload after sending, and asserts the existing content stays visible.

**Step 2: Run test to verify it fails**

Run: `pnpm vitest apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx -t "keeps prior session content visible when reload after send fails"`

Expected: FAIL because the controller currently clears messages/session runs in the catch path.

**Step 3: Write minimal implementation**

Keep the previous `messages` and `sessionRuns` state when reloads fail, while preserving the intentional reset for brand-new initial-message sessions.

**Step 4: Run test to verify it passes**

Run: `pnpm vitest apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx -t "keeps prior session content visible when reload after send fails"`

Expected: PASS

### Task 3: Clarify Recovery Copy

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: Write the failing test**

Extend an existing network failure rendering test to assert the card mentions that previous output is retained and the user can type `继续` after connectivity recovers.

**Step 2: Run test to verify it fails**

Run: `pnpm vitest apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx -t "shows network failure recovery guidance"`

Expected: FAIL because the current message only reports the network problem.

**Step 3: Write minimal implementation**

Append recovery guidance only for network failures, reusing the current continue semantics.

**Step 4: Run test to verify it passes**

Run: `pnpm vitest apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx -t "shows network failure recovery guidance"`

Expected: PASS
