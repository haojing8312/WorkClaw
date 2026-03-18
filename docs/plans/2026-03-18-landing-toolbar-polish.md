# Landing Toolbar Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Polish the homepage task composer so its attachment button matches chat styling and its workdir button shows the default directory name with full-path hover.

**Architecture:** Keep the change inside the runtime frontend. Reuse the already loaded `defaultWorkDir` in `App` to seed the landing composer display, and only adjust `NewSessionLanding` presentation/state initialization without changing session creation or backend contracts.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Add failing landing tests for toolbar polish

**Files:**
- Modify: `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`

**Step 1: Write the failing test**

Add coverage for:
- attachment button renders with icon affordance hook
- default workdir is shown on initial render
- long workdir keeps full path in `title` while displaying the leaf directory name

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test -- --run src/components/__tests__/NewSessionLanding.test.tsx`
Expected: FAIL because landing composer does not receive or display a default workdir and the attachment button is still plain text.

### Task 2: Thread the default workdir into the landing component

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/components/NewSessionLanding.tsx`

**Step 1: Add a landing prop for default workdir**

Pass the already loaded `defaultWorkDir` from `App` into `NewSessionLanding`.

**Step 2: Seed landing display state**

Initialize landing workdir display from the default workdir, but allow explicit user picks to override it.

### Task 3: Implement the toolbar polish

**Files:**
- Modify: `apps/runtime/src/components/NewSessionLanding.tsx`

**Step 1: Add the attachment icon**

Update the landing attachment trigger to visually match the chat composer with a lightweight icon + label treatment.

**Step 2: Refine workdir display**

Display:
- the leaf directory name when a workdir exists
- full path in `title`
- `选择工作目录` only when no default or picked workdir exists

### Task 4: Verify the changed runtime surface

**Files:**
- Modify: none

**Step 1: Run focused tests**

Run:
- `pnpm --filter runtime test -- --run src/components/__tests__/NewSessionLanding.test.tsx`
- `pnpm --filter runtime test -- --run src/__tests__/App.session-create-flow.test.tsx`

**Step 2: Report remaining risk honestly**

Call out that this verifies landing UI and session handoff, but not full desktop E2E.
