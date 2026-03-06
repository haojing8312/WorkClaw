# First-Use Dev Entry Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a development-only entry in runtime settings so developers can reset first-use onboarding state and reopen the quick model setup dialog for manual testing.

**Architecture:** Keep first-use state ownership inside `App.tsx`, where onboarding visibility is already computed. Extend `SettingsView` with optional development-only callbacks rendered only when `import.meta.env.DEV` is true, so production behavior and persisted runtime data models stay unchanged.

**Tech Stack:** React 18, TypeScript, Vitest, Testing Library, Vite env flags

---

### Task 1: Define the development-only settings hooks

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/App.tsx`

**Step 1: Write the failing test**

Add an app-level regression test that opens settings in development mode and expects:
- a dev tools section to appear
- a "reset first-use onboarding" action
- an "open quick model setup" action

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.model-setup-hint.test.tsx`

Expected: FAIL because the new controls do not exist yet.

**Step 3: Write minimal implementation**

- Extend `SettingsView` props with optional callbacks:
  - `showDevModelSetupTools?: boolean`
  - `onDevResetFirstUseOnboarding?: () => void`
  - `onDevOpenQuickModelSetup?: () => void`
- In `App.tsx`, pass those props only in development mode.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.model-setup-hint.test.tsx`

Expected: PASS for the new dev entry assertions.

### Task 2: Implement the onboarding reset/open actions

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Test: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`

**Step 1: Write the failing test**

Add targeted assertions for:
- resetting local storage onboarding flags from the dev entry
- reopening the blocking first-use gate after reset when there are still no models
- opening the quick setup dialog directly from the dev entry without touching persisted model configs

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.model-setup-hint.test.tsx`

Expected: FAIL because the callbacks are not wired.

**Step 3: Write minimal implementation**

In `App.tsx`:
- add a small helper to clear `INITIAL_MODEL_SETUP_COMPLETED_KEY` and `MODEL_SETUP_HINT_DISMISSED_KEY`
- update component state to reflect the reset immediately
- reuse existing `openQuickModelSetup()` for the direct-open action

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.model-setup-hint.test.tsx`

Expected: PASS, including the new dev-only flows.

### Task 3: Render the development tools section in settings

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Test: `apps/runtime/src/components/__tests__/SettingsView.theme.test.tsx`

**Step 1: Write the failing test**

Add a settings-level test that renders `SettingsView` with dev props and asserts:
- the dev section appears when `showDevModelSetupTools` is true
- it stays hidden by default

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/SettingsView.theme.test.tsx`

Expected: FAIL because the section is not rendered yet.

**Step 3: Write minimal implementation**

Render a compact panel in the `models` tab with:
- a small explanatory label
- a button to reset first-use onboarding
- a button to open quick model setup

Keep styling aligned with the existing semantic settings surface.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/SettingsView.theme.test.tsx`

Expected: PASS with no production-path regressions.

### Task 4: Verify the full feature

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Test: `apps/runtime/src/__tests__/App.model-setup-hint.test.tsx`
- Test: `apps/runtime/src/components/__tests__/SettingsView.theme.test.tsx`

**Step 1: Run targeted tests**

Run:
- `pnpm --dir apps/runtime exec vitest run src/__tests__/App.model-setup-hint.test.tsx`
- `pnpm --dir apps/runtime exec vitest run src/components/__tests__/SettingsView.theme.test.tsx`

Expected: PASS.

**Step 2: Run build verification**

Run: `pnpm --dir apps/runtime build`

Expected: successful TypeScript and Vite build.
