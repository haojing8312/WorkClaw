# Quick Model Footer Feedback Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ensure quick model setup shows immediate connection feedback in the footer action area without requiring any scrolling.

**Architecture:** Reuse the existing quick setup test state and result state in `App.tsx`, but render the result banner in the footer action section instead of anywhere in the scrollable body. Clear stale test results when key connection fields change so the footer status always reflects the current input.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, Tailwind utility classes

---

### Task 1: Cover footer visibility with a failing test

**Files:**
- Modify: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`
- Test: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`

**Step 1: Write the failing test**

Add assertions that:
- the connection result banner is rendered inside the footer action area
- the banner text matches the latest success/failure state

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- App.model-setup-hint.test.tsx`
Expected: FAIL if the banner is still rendered in the scrollable content or not rendered at all in the footer.

**Step 3: Write minimal implementation**

Move the quick setup connection result UI into the footer action area in `apps/runtime/src/App.tsx`.

**Step 4: Run test to verify it passes**

Run: `pnpm --filter runtime test -- App.model-setup-hint.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/__tests__/App.model-setup-hint.test.tsx
git commit -m "fix(runtime): show quick setup connection result in footer"
```

### Task 2: Clear stale success state after input changes

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`
- Test: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`

**Step 1: Write the failing test**

Add a test that:
- performs a successful connection test
- changes a key field such as `API Key`
- expects the previous footer success banner to disappear

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- App.model-setup-hint.test.tsx`
Expected: FAIL because the previous result still remains after editing.

**Step 3: Write minimal implementation**

Reset the quick setup test result when key connection inputs change.

**Step 4: Run test to verify it passes**

Run: `pnpm --filter runtime test -- App.model-setup-hint.test.tsx`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/__tests__/App.model-setup-hint.test.tsx
git commit -m "fix(runtime): clear stale quick setup connection result"
```

### Task 3: Verify the targeted flow end-to-end

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Test: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`

**Step 1: Run targeted verification**

Run: `pnpm --filter runtime test -- App.model-setup-hint.test.tsx`
Expected: PASS with all quick setup tests green.

**Step 2: Review UI behavior manually**

Confirm:
- the banner appears above the footer buttons
- the loading state still works
- the footer layout remains readable in the dialog

**Step 3: Commit**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/__tests__/App.model-setup-hint.test.tsx docs/plans/2026-03-14-quick-model-footer-feedback-*.md
git commit -m "docs(plans): capture quick model footer feedback plan"
```
