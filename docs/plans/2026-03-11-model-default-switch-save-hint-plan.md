# Model Default Switch Save Hint Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Show an explicit success hint in the model settings form when creating a new model automatically switches the default model.

**Architecture:** Keep the feedback local to `SettingsView` by adding a small model-form save state and message string. The save handler decides between a generic success message and a more explicit “switched to default” message based on whether the save happened in create mode and triggered `set_default_model`.

**Tech Stack:** React 18, TypeScript, Vitest, Testing Library

---

### Task 1: Add failing tests for model save success messaging

**Files:**
- Modify: `apps/runtime/src/components/__tests__/SettingsView.model-providers.test.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`

**Step 1: Write the failing tests**

- Add a test that creates a second model and expects the form area to show `已保存，并切换为默认模型`.
- Add a test that edits an existing model and expects the form area to show `已保存`.

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --filter runtime test -- SettingsView.model-providers.test.tsx
```

Expected:
- Tests fail because the model form does not expose a save-success hint yet.

**Step 3: Write minimal implementation**

- Add local model-form success state and message fields.
- Clear stale success state before saves and form resets.
- Set the explicit “switched to default” message only for create-mode saves that invoke `set_default_model`.
- Render the success hint near the model form action area using the existing green badge style.

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --filter runtime test -- SettingsView.model-providers.test.tsx
```

Expected:
- The new success-hint tests pass along with existing model-provider tests.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/__tests__/SettingsView.model-providers.test.tsx
git commit -m "feat(settings): explain default switch after model creation"
```

### Task 2: Document the behavior for future maintenance

**Files:**
- Modify: `docs/user-manual/06-settings.md`

**Step 1: Update the user manual**

- Add one concise sentence that when a new model is added, the settings page will show a success hint if that new model becomes the default.

**Step 2: Commit**

```bash
git add docs/user-manual/06-settings.md
git commit -m "docs(settings): note save hint for default model switching"
```
