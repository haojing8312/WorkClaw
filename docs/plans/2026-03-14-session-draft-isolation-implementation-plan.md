# Session Draft Isolation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Preserve unsent composer drafts per chat session so switching sessions feels like switching browser tabs.

**Architecture:** Keep draft persistence inside `ChatView` and store drafts in `localStorage` by `session_id`. Save on input changes, restore on `sessionId` changes, and clear when the composer empties after send or initial auto-send.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Lock session draft behavior with a failing test

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Write the failing test**

Add a test that types a draft in session A, switches to session B, types a different draft, then switches back and expects session A's draft to be restored.

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.session-resilience.test.tsx -t "keeps unsent drafts isolated per session when switching between conversations" --pool forks --poolOptions.forks.singleFork`

Expected: FAIL because `ChatView` currently keeps one shared input state across rerenders.

### Task 2: Implement session-scoped draft persistence

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`

**Step 1: Write minimal implementation**

Add small `localStorage` helpers keyed by `sessionId`, restore drafts when the active session changes, and persist/remove drafts when the composer input changes.

**Step 2: Run focused test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.session-resilience.test.tsx -t "keeps unsent drafts isolated per session when switching between conversations" --pool forks --poolOptions.forks.singleFork`

Expected: PASS

### Task 3: Verify related chat flows still behave correctly

**Files:**
- Verify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Run the full resilience suite**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.session-resilience.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: PASS

**Step 2: Run typecheck**

Run: `pnpm --dir apps/runtime exec tsc --noEmit`

Expected: PASS
