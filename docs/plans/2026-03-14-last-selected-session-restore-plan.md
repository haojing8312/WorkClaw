# Last Selected Session Restore Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore the last selected chat session on reload or restart by persisting `session_id`, without treating skills as session groups.

**Architecture:** Keep this entirely in the runtime frontend. `App.tsx` will persist the selected session id to `localStorage`, restore it after sessions load, and fall back safely when the session no longer exists. No backend schema changes and no skill-scoped recovery keys.

**Tech Stack:** React, Tauri frontend, Vitest, Testing Library, TypeScript

---

### Task 1: Lock the recovery behavior with a failing test

**Files:**
- Modify: `apps/runtime/src/__tests__/App.chat-landing.test.tsx`

**Step 1: Write the failing test**

Add a test that seeds `localStorage` with a previously selected `session_id`, renders `App`, and expects the matching session to open directly instead of showing the landing state.

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.chat-landing.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: FAIL because the app currently does not restore the saved session.

### Task 2: Implement minimal restore and persistence logic

**Files:**
- Modify: `apps/runtime/src/App.tsx`

**Step 1: Write minimal implementation**

Add a dedicated `localStorage` key for the last selected session id. Persist the id whenever the selected session changes, clear it when the selected session becomes invalid, and restore it only after the latest session list has loaded and the stored id is present in that list.

**Step 2: Run the focused test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.chat-landing.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: PASS

### Task 3: Verify no regression in adjacent session flows

**Files:**
- Verify: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`

**Step 1: Run adjacent tests**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.chat-landing.test.tsx src/__tests__/App.session-create-flow.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: PASS

**Step 2: Run typecheck**

Run: `pnpm --dir apps/runtime exec tsc --noEmit`

Expected: PASS
