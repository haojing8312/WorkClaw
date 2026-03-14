# Feishu Association Atomic Save Hotfix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Feishu employee reception saves atomic and keep employee Feishu status accurate for legacy binding-only data.

**Architecture:** Add one backend command in `employee_agents.rs` that updates employee scopes and Feishu routing bindings inside a single transaction, then simplify the React save path to call it. Update the employee list status logic to recognize either `enabled_scopes` or existing Feishu bindings.

**Tech Stack:** React, TypeScript, Tauri, Rust, sqlx, Vitest, Tokio integration tests

---

### Task 1: Cover legacy-binding status in the React test suite

**Files:**
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`

**Step 1: Write the failing test**

Add a test that renders an employee with:

- `enabled_scopes: []`
- one Feishu binding returned from `list_im_routing_bindings`

Assert that the employee list dot is not gray and the UI does not present the employee as "未关联飞书接待".

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime test -- EmployeeHubView.feishu-connection-status.test.tsx
```

Expected: FAIL because `resolveFeishuStatus` only checks `enabled_scopes`.

**Step 3: Write minimal implementation**

Update `EmployeeHubView.tsx` so `resolveFeishuStatus(employee)` also treats the employee as Feishu-enabled when `routingBindings` contains at least one Feishu binding for that employee agent id.

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --dir apps/runtime test -- EmployeeHubView.feishu-connection-status.test.tsx
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx
git commit -m "test(feishu): cover legacy binding status"
```

### Task 2: Add a backend test for atomic Feishu association saving

**Files:**
- Modify: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`

**Step 1: Write the failing test**

Add an integration test that:

- creates two employees
- seeds an existing Feishu default or scoped binding
- calls the new helper with intentionally invalid follow-up input that should fail during the same save
- verifies the employee scopes and original routing bindings remain unchanged after failure

Also add one success-path test that verifies:

- the previous default or conflicting scoped binding is replaced
- the employee scopes are updated together with the new binding

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test test_im_employee_agents save_feishu -- --nocapture
```

Expected: FAIL because the helper does not exist yet.

**Step 3: Write minimal implementation**

In `employee_agents.rs`:

- add a serializable input struct for Feishu association save
- extract or add a transaction-safe helper that can persist the employee row without opening a second transaction
- add `save_feishu_employee_association_with_pool(...)`
- add the tauri command wrapper `save_feishu_employee_association(...)`
- register the new command in `src-tauri/src/lib.rs`

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test test_im_employee_agents save_feishu -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_im_employee_agents.rs
git commit -m "fix(feishu): save employee association atomically"
```

### Task 3: Switch the React save path to the backend command

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx`

**Step 1: Write the failing test**

Add or update a React test that clicks "保存飞书接待" and asserts:

- the frontend invokes `save_feishu_employee_association`
- it no longer invokes `delete_im_routing_binding` / `upsert_im_routing_binding` directly for the save path

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime test -- EmployeeHubView.feishu-connection-status.test.tsx
```

Expected: FAIL because the old multi-call save flow is still in place.

**Step 3: Write minimal implementation**

Update `EmployeeHubView.tsx` to:

- build one payload for the new backend command
- invoke it once
- reload employees/bindings state needed by the UI

Add a shared TS type in `apps/runtime/src/types.ts` if that keeps the payload shape explicit.

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --dir apps/runtime test -- EmployeeHubView.feishu-connection-status.test.tsx
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/types.ts apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx
git commit -m "refactor(feishu): use atomic association save command"
```

### Task 4: Run focused verification for the hotfix

**Files:**
- No code changes required unless failures appear

**Step 1: Run frontend verification**

```bash
pnpm --dir apps/runtime test -- SettingsView.feishu.test.tsx SettingsView.feishu-routing-wizard.test.tsx EmployeeHubView.feishu-connection-status.test.tsx EmployeeHubView.thread-binding.test.tsx
```

Expected: PASS with all targeted Feishu employee tests green.

**Step 2: Run backend verification**

```bash
cargo test --test test_im_employee_agents save_feishu -- --nocapture
```

Expected: PASS with the new atomic-save coverage green.

**Step 3: Commit any follow-up fixes**

```bash
git add -A
git commit -m "test(feishu): verify atomic association hotfix"
```

Only create this commit if verification required code fixes.
