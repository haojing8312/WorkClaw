# Session Runtime State Persistence Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Preserve in-progress chat runtime UI when switching sessions, task tabs, or temporary main views.

**Architecture:** `App` will own a per-session runtime snapshot map keyed by `sessionId`. `ChatView` will hydrate from that snapshot and publish updates whenever streaming UI state changes so the active session can be restored without waiting for backend persistence.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Add failing runtime persistence tests

**Files:**
- Modify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`
- Modify: `apps/runtime/src/__tests__/App.chat-landing.test.tsx`

**Step 1: Write the failing test**

- Add a `ChatView` test that passes a saved runtime snapshot and expects the streaming bubble content to render immediately.
- Add an `App` test that simulates a session publishing runtime state, switches away, switches back, and expects the saved state to still be passed into `ChatView`.

**Step 2: Run test to verify it fails**

Run: `pnpm vitest apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx apps/runtime/src/__tests__/App.chat-landing.test.tsx`

Expected: new tests fail because runtime state is not persisted across session/view switches.

### Task 2: Add App-level runtime snapshot ownership

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/types.ts`

**Step 1: Write the minimal implementation**

- Define a reusable runtime snapshot type.
- Add `sessionId -> runtime snapshot` state in `App`.
- Pass the selected session snapshot into `ChatView`.
- Accept runtime snapshot updates from `ChatView` and store them per session.

**Step 2: Run targeted tests**

Run: `pnpm vitest apps/runtime/src/__tests__/App.chat-landing.test.tsx`

Expected: updated `App` test passes.

### Task 3: Hydrate and publish runtime state in ChatView

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

**Step 1: Write the minimal implementation**

- Accept an optional persisted runtime snapshot prop.
- Initialize/reset execution UI state from that snapshot on session switch.
- Publish runtime snapshot updates when visible execution state changes.

**Step 2: Run targeted tests**

Run: `pnpm vitest apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx`

Expected: runtime hydration test passes.

### Task 4: Verify the touched WorkClaw surface

**Files:**
- Modify: none unless fixes are needed

**Step 1: Run verification commands**

Run: `pnpm vitest apps/runtime/src/components/__tests__/ChatView.session-resilience.test.tsx apps/runtime/src/__tests__/App.chat-landing.test.tsx`

Run: `pnpm test:e2e:runtime` only if targeted tests reveal a broader regression risk that cannot be covered honestly by the touched-unit tests.

**Step 2: Inspect results**

- Confirm new regression coverage passes
- Report any still-unverified user-facing runtime areas explicitly
