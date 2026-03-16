# Provider Default Model Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refresh built-in provider preset defaults so provider selection autofills current agent-oriented recommended models in both Settings and quick setup.

**Architecture:** The shared provider catalog already feeds both onboarding and settings. We only need to update its default model entries and suggestion ordering, then lock the behavior with focused frontend tests.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Lock the desired defaults with tests

**Files:**
- Modify: `apps/runtime/src/__tests__/model-provider-catalog.test.ts`
- Modify: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`

**Step 1: Write the failing test**

Add assertions for refreshed provider defaults, starting with Zhipu and Qwen, plus any quick-setup expectations that still reference `glm-4-flash`.

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/__tests__/model-provider-catalog.test.ts --config vitest.config.ts`

Expected: FAIL because the catalog still returns stale defaults such as `glm-4-flash`.

**Step 3: Write minimal implementation**

Update only the catalog entries and test fixtures needed to satisfy the new expectations.

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/__tests__/model-provider-catalog.test.ts src/__tests__/App.model-setup-hint.test.tsx --config vitest.config.ts`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/model-provider-catalog.ts apps/runtime/src/__tests__/model-provider-catalog.test.ts apps/runtime/src/__tests__/App.model-setup-hint.test.tsx
git commit -m "feat(runtime): refresh provider default model presets"
```

### Task 2: Refresh the shared provider catalog

**Files:**
- Modify: `apps/runtime/src/model-provider-catalog.ts`

**Step 1: Update default models**

Change the official provider `defaultModel` values to the refreshed agent-oriented defaults and reorder each `models` list to place the default first.

**Step 2: Keep compatibility**

Do not change provider IDs, `providerKey`, API format, or base URL behavior.

**Step 3: Run targeted tests**

Run: `npx vitest run src/__tests__/model-provider-catalog.test.ts src/__tests__/App.model-setup-hint.test.tsx --config vitest.config.ts`

Expected: PASS.

**Step 4: Sanity-check optional placeholders**

Update any provider-related placeholder text that still calls out obviously stale model examples.

### Task 3: Record the design decision

**Files:**
- Create: `docs/plans/2026-03-16-provider-default-model-refresh-design.md`
- Create: `docs/plans/2026-03-16-provider-default-model-refresh-plan.md`

**Step 1: Save rationale**

Document why the app keeps a static provider catalog and why defaults now bias toward agent-capable models.

**Step 2: Final verification**

Run:

```bash
npx vitest run src/__tests__/model-provider-catalog.test.ts src/__tests__/App.model-setup-hint.test.tsx --config vitest.config.ts
git diff --stat
```

Expected: tests green and only intended files changed.
