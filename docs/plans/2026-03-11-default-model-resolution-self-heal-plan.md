# Default Model Resolution Self-Heal Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Unify default-model resolution across frontend and backend so every entrypoint prefers `is_default`, falls back safely, and self-heals missing-default data.

**Architecture:** Add a tiny frontend helper for reading the default model from loaded configs, and add backend pool-level helpers that resolve or repair the default model before returning it. Replace scattered `models[0]` and duplicated SQL fallback blocks with these shared helpers.

**Tech Stack:** React 18, TypeScript, Tauri 2, Rust, Vitest, sqlx

---

### Task 1: Add failing frontend tests for default-model selection

**Files:**
- Modify: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.theme.test.tsx`

**Step 1: Write the failing tests**

- Update the app session-create test so `list_model_configs` returns two models where the first is not default and the second is default, then assert `create_session` uses the second id.
- Add a chat-view test that passes the same two-model array and asserts the visible model badge shows the default model name, not the first array element.

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --filter runtime test -- App.session-create-flow.test.tsx ChatView.theme.test.tsx
```

Expected:
- Tests fail because the app and chat view still read `models[0]`.

**Step 3: Write minimal implementation**

- Add `apps/runtime/src/lib/default-model.ts` with `getDefaultModel()` and `getDefaultModelId()`.
- Replace all `models[0]?.id` reads in `App.tsx`.
- Replace the `ChatView` current-model selection with `getDefaultModel(models)`.

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --filter runtime test -- App.session-create-flow.test.tsx ChatView.theme.test.tsx
```

Expected:
- Session creation and chat header both honor `is_default`.

**Step 5: Commit**

```bash
git add apps/runtime/src/lib/default-model.ts apps/runtime/src/App.tsx apps/runtime/src/components/ChatView.tsx apps/runtime/src/__tests__/App.session-create-flow.test.tsx apps/runtime/src/components/__tests__/ChatView.theme.test.tsx
git commit -m "refactor(runtime): unify frontend default model selection"
```

### Task 2: Add failing backend tests for default-model self-healing

**Files:**
- Modify: `apps/runtime/src-tauri/tests/test_models.rs`
- Modify: `apps/runtime/src-tauri/src/commands/models.rs`

**Step 1: Write the failing tests**

- Add a test for `resolve_default_model_id_with_pool` where one default exists and assert that id is returned unchanged.
- Add a second test with two non-search models and no default, assert the first id is returned and one row becomes default afterward.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_models
```

Expected:
- Tests fail because the helper does not exist yet.

**Step 3: Write minimal implementation**

- Add shared pool-level default-model resolver helpers to `commands/models.rs`.
- Keep search configs excluded.
- If the fallback path is used, call `set_default_model_with_pool` before returning.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_models
```

Expected:
- Default-model helper tests pass and prove self-healing.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/models.rs apps/runtime/src-tauri/tests/test_models.rs
git commit -m "feat(models): self-heal missing default model state"
```

### Task 3: Replace duplicated backend fallback logic

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/clawhub.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/tests/test_models.rs`

**Step 1: Write or extend failing tests if needed**

- Add a focused test that the shared resolver can be used for API-key-required selection and still self-heals.

**Step 2: Run test to verify failure**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_models
```

Expected:
- Tests fail until the API-key-aware resolver path exists.

**Step 3: Write minimal implementation**

- Update `clawhub.rs` to use the shared config resolver instead of separate `default + fallback` SQL.
- Update `employee_agents.rs` to use the shared id resolver instead of manually querying `is_default` then first row.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_models
```

Expected:
- Shared resolver handles all covered call sites cleanly.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/clawhub.rs apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/tests/test_models.rs
git commit -m "refactor(models): reuse shared default resolver in runtime commands"
```
