# Quick Model Test Result Visibility Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the quick model setup connection result visible immediately after clicking `测试连接` by moving the status message next to the action buttons.

**Architecture:** Keep the existing `quickModelTestResult` state and async command flow in `App.tsx`, but render the status banner in the footer action section instead of the scrollable form body. This keeps state management unchanged while improving visibility at the interaction point.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, Tailwind utility classes

---

### Task 1: Add a failing UI test for footer-level feedback

**Files:**
- Modify: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`
- Test: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`

**Step 1: Write the failing test**

Add an assertion that the quick setup test result is rendered inside the footer action area test container after clicking `测试连接`.

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- App.model-setup-hint.test.tsx`
Expected: FAIL because the status banner is still rendered in the scrollable form area, not the footer action area.

**Step 3: Write minimal implementation**

Move the `quickModelTestResult` banner in `apps/runtime/src/App.tsx` so it renders inside the footer section near `quick-model-setup-actions`.

**Step 4: Run test to verify it passes**

Run: `pnpm --filter runtime test -- App.model-setup-hint.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/__tests__/App.model-setup-hint.test.tsx docs/plans/2026-03-14-quick-model-test-result-visibility-*.md
git commit -m "fix(runtime): surface quick setup test result near actions"
```

### Task 2: Verify the updated interaction stays stable

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Test: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`

**Step 1: Run targeted verification**

Run: `pnpm --filter runtime test -- App.model-setup-hint.test.tsx`
Expected: PASS with no new failures in the quick setup suite.

**Step 2: Inspect for layout regressions**

Confirm the footer still shows helper text, buttons, and status banner in the expected order for both success and failure states.

**Step 3: Commit**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/__tests__/App.model-setup-hint.test.tsx docs/plans/2026-03-14-quick-model-test-result-visibility-*.md
git commit -m "test(runtime): cover quick setup connection feedback visibility"
```
