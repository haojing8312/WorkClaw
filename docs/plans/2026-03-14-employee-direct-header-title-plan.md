# Employee Direct Header Title Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ensure employee-direct chat sessions show the agent employee name in the chat header instead of the underlying skill name.

**Architecture:** Keep the existing `ChatView` header priority order, but make `App` provide a more reliable `sessionEmployeeName` fallback for employee-direct sessions. Add a regression test that reproduces the missing-`employee_id` case and verify the header still resolves to the employee name.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Lock the regression with a failing test

**Files:**
- Modify: `apps/runtime/src/__tests__/App.employee-chat-entry.test.tsx`

**Step 1: Write the failing test**

Add an assertion that an employee-direct session still passes `sessionEmployeeName="销售主管"` to `ChatView` even when the selected session payload does not include `employee_id`.

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- App.employee-chat-entry.test.tsx`

Expected: FAIL because `sessionEmployeeName` is empty.

### Task 2: Implement the minimal fallback

**Files:**
- Modify: `apps/runtime/src/App.tsx`

**Step 1: Write minimal implementation**

Extend `selectedSessionEmployeeName` so employee-direct sessions can fall back to the currently selected employee when the session payload is missing `employee_id`.

**Step 2: Run test to verify it passes**

Run: `pnpm --filter runtime test -- App.employee-chat-entry.test.tsx`

Expected: PASS.

### Task 3: Verify no regression in header behavior

**Files:**
- Verify: `apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx`

**Step 1: Run focused regression coverage**

Run: `pnpm --filter runtime test -- App.employee-chat-entry.test.tsx ChatView.im-routing-panel.test.tsx`

Expected: PASS.
