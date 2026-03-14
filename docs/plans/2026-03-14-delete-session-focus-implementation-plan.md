# Delete Session Focus Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Keep chat focus on a neighboring session when the user deletes the currently selected session.

**Architecture:** Compute the adjacent session id from the existing `sessions` array in `App.tsx`, update local selection immediately after a successful delete, and then refresh the canonical session list from the backend. The focus rule is next session first, previous second, landing last.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Lock delete-focus behavior with failing tests

**Files:**
- Modify: `apps/runtime/src/__tests__/App.chat-landing.test.tsx`

**Step 1: Write the failing tests**

Add tests that:
- select the first session, delete it, and expect focus to move to the next session
- select the last session, delete it, and expect focus to move to the previous session

**Step 2: Run tests to verify they fail**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.chat-landing.test.tsx -t "deleting the selected" --pool forks --poolOptions.forks.singleFork`

Expected: FAIL because `handleDeleteSession` currently clears `selectedSessionId`.

### Task 2: Implement adjacent focus logic

**Files:**
- Modify: `apps/runtime/src/App.tsx`

**Step 1: Write minimal implementation**

Add a small helper to compute the neighboring session id from the current `sessions` order and use it in `handleDeleteSession` when deleting the active session.

**Step 2: Run focused tests to verify they pass**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.chat-landing.test.tsx -t "deleting the selected" --pool forks --poolOptions.forks.singleFork`

Expected: PASS

### Task 3: Verify adjacent chat flows remain stable

**Files:**
- Verify: `apps/runtime/src/__tests__/App.chat-landing.test.tsx`

**Step 1: Run the full landing suite**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.chat-landing.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: PASS

**Step 2: Run typecheck**

Run: `pnpm --dir apps/runtime exec tsc --noEmit`

Expected: PASS
