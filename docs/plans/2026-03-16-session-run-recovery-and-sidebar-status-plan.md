# Session Run Recovery And Sidebar Status Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore in-progress assistant output when reopening a running session, and surface a compact runtime status icon for each session in the sidebar.

**Architecture:** Keep `session_runs` as the single source of truth for active execution state. The backend will aggregate a lightweight `runtime_status` onto `list_sessions`, while the frontend `ChatView` will synthesize a temporary recovery assistant message from the latest active run when no final assistant message exists yet.

**Tech Stack:** React 18, TypeScript, Vitest, Tauri 2, Rust, SQLx, SQLite

---

### Task 1: Add backend session runtime status aggregation

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_session_io.rs`
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src-tauri/src/commands/chat_session_io.rs`

**Step 1: Write the failing backend test**

Add a test near the existing `list_sessions_with_pool` tests that creates sessions plus `session_runs` rows and expects `list_sessions_with_pool(...)` to return:

- `running` for `thinking`
- `waiting_approval` for `waiting_approval`
- `completed` for `completed`
- `failed` for `failed` and `cancelled`

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- --runInBand apps/runtime/src-tauri/src/commands/chat_session_io.rs`

Expected: FAIL because `runtime_status` is missing or incorrect.

**Step 3: Write minimal implementation**

Update `list_sessions_with_pool` to join or subquery the latest relevant `session_runs` row per session and project a normalized `runtime_status`.

Add `runtime_status` to the serialized session JSON and to the frontend `SessionInfo` type.

**Step 4: Run test to verify it passes**

Run: `cargo test -p runtime chat_session_io::tests::list_sessions_with_pool_ -- --nocapture`

Expected: PASS for the new aggregation cases.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_session_io.rs apps/runtime/src/types.ts
git commit -m "feat: project session runtime status"
```

### Task 2: Recover active buffered assistant output in ChatView

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Write the failing frontend test**

Add a test that mounts `ChatView` with:

- one existing user message
- one active run from `list_session_runs`
- no assistant message yet
- `buffered_text` containing partial assistant output

Expect the partial assistant output to render immediately after mount.

Add another test that rerenders or refreshes with a formal assistant message present and expects the temporary recovery content to disappear.

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

Expected: FAIL because `ChatView` only renders `messages`.

**Step 3: Write minimal implementation**

In `ChatView.tsx`:

- derive the latest active run from `sessionRuns`
- detect runs with no `assistant_message_id` and non-empty `buffered_text`
- synthesize a temporary assistant message for rendering only
- append it to the rendered message list without mutating persisted `messages`
- suppress the synthetic item once the real assistant message exists

Keep existing failure-card behavior intact.

**Step 4: Run test to verify it passes**

Run: `pnpm --filter runtime test -- apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

Expected: PASS for the recovery scenario and no regression in existing resilience tests.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx
git commit -m "fix: recover active session output on reopen"
```

### Task 3: Render runtime status icons in the sidebar

**Files:**
- Modify: `apps/runtime/src/components/Sidebar.tsx`
- Add or Modify: `apps/runtime/src/components/__tests__/Sidebar.runtime-status.test.tsx`

**Step 1: Write the failing sidebar test**

Add tests that render `Sidebar` with sessions carrying:

- `runtime_status: "running"`
- `runtime_status: "waiting_approval"`
- `runtime_status: "completed"`
- `runtime_status: "failed"`
- no `runtime_status`

Expect the corresponding icon label or tooltip to appear only for the first four cases.

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- apps/runtime/src/components/__tests__/Sidebar.runtime-status.test.tsx`

Expected: FAIL because the sidebar currently has no status icon rendering.

**Step 3: Write minimal implementation**

In `Sidebar.tsx`:

- add a small helper to map `runtime_status` to icon, color class, and label
- render the icon to the left of each session title
- preserve source badge, export, and delete affordances

**Step 4: Run test to verify it passes**

Run: `pnpm --filter runtime test -- apps/runtime/src/components/__tests__/Sidebar.runtime-status.test.tsx`

Expected: PASS for all status/icon combinations.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/Sidebar.tsx apps/runtime/src/components/__tests__/Sidebar.runtime-status.test.tsx
git commit -m "feat: show sidebar runtime status icons"
```

### Task 4: Verify integration end to end

**Files:**
- Modify if needed: `apps/runtime/src/__tests__/App.sidebar-navigation-selected-session.test.tsx`
- Modify if needed: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Write the failing integration test**

If coverage is still missing, add one integration-oriented UI test that:

- starts on one session
- switches to another
- switches back
- verifies the original session still shows its recovered in-progress output

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- App.sidebar-navigation-selected-session ChatView.session-resilience`

Expected: FAIL until the recovery flow is wired through.

**Step 3: Write minimal implementation**

Only if the earlier tasks did not already satisfy the integration path, add the smallest required plumbing.

**Step 4: Run targeted tests**

Run: `pnpm --filter runtime test -- apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx apps/runtime/src/components/__tests__/Sidebar.runtime-status.test.tsx apps/runtime/src/__tests__/App.sidebar-navigation-selected-session.test.tsx`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src apps/runtime/src-tauri/src
git commit -m "test: cover session recovery and sidebar status flow"
```

### Task 5: Final verification

**Files:**
- No new files expected

**Step 1: Run frontend test suite for touched areas**

Run: `pnpm --filter runtime test -- apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx apps/runtime/src/components/__tests__/Sidebar.runtime-status.test.tsx`

Expected: PASS with no new failures in touched tests.

**Step 2: Run backend tests for touched areas**

Run: `cargo test -p runtime chat_session_io::tests::list_sessions_with_pool_`

Expected: PASS for runtime status aggregation tests.

**Step 3: Manual smoke verification**

Run the desktop app and verify:

1. Start a long-running task in session A
2. Switch to home and create session B
3. Switch back to session A
4. Confirm buffered assistant output is still visible
5. Confirm sidebar shows the correct runtime icon for session A

Suggested command: `pnpm app`

**Step 4: Commit final polish if needed**

```bash
git add apps/runtime/src apps/runtime/src-tauri/src docs/plans
git commit -m "fix: preserve running session output across navigation"
```
