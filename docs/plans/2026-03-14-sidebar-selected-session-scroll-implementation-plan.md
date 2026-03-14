# Sidebar Selected Session Scroll Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Automatically scroll the selected sidebar session into view so multi-session navigation feels more like browser tabs.

**Architecture:** Keep this entirely inside `Sidebar.tsx` by storing per-session row refs and running a small effect keyed by `selectedSessionId`, `sessions`, and `collapsed`. Use `scrollIntoView({ block: "nearest", inline: "nearest" })` for minimal movement.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Lock the behavior with a failing test

**Files:**
- Create: `apps/runtime/src/components/__tests__/Sidebar.selected-session-scroll.test.tsx`

**Step 1: Write the failing test**

Add a test that renders the sidebar, changes `selectedSessionId` to a visible session, and expects that row's `scrollIntoView` method to be called.

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/Sidebar.selected-session-scroll.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: FAIL because `Sidebar` currently has no scroll effect.

### Task 2: Implement the scroll effect

**Files:**
- Modify: `apps/runtime/src/components/Sidebar.tsx`

**Step 1: Write minimal implementation**

Track session row elements in refs and scroll the selected row into view whenever the active session becomes available in the rendered list.

**Step 2: Run the focused test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/Sidebar.selected-session-scroll.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: PASS

### Task 3: Verify adjacent sidebar behavior remains stable

**Files:**
- Verify: `apps/runtime/src/components/__tests__/Sidebar.display-title.test.tsx`
- Verify: `apps/runtime/src/components/__tests__/Sidebar.theme.test.tsx`

**Step 1: Run sidebar tests**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/Sidebar.selected-session-scroll.test.tsx src/components/__tests__/Sidebar.display-title.test.tsx src/components/__tests__/Sidebar.theme.test.tsx --pool forks --poolOptions.forks.singleFork`

Expected: PASS

**Step 2: Run typecheck**

Run: `pnpm --dir apps/runtime exec tsc --noEmit`

Expected: PASS
