# Permission Mode Simplification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current three-tier permission model with a two-mode personal-edition flow that defaults to low-friction execution and only confirms truly critical actions.

**Architecture:** Persist a desktop-level permission preference, remove session-scoped mode selection from the sidebar, and route confirmation decisions through a risk classifier instead of tool-name checks. Frontend settings UI becomes the only place to change modes, while the runtime evaluates file, command, and browser actions as either normal or critical.

**Tech Stack:** React 18, Tauri 2, TypeScript, Rust, Vitest, Rust tests

---

### Task 1: Persist the new desktop permission mode

**Files:**
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src-tauri/src/commands/runtime_preferences.rs`
- Test: `apps/runtime/src/components/__tests__/SettingsView.risk-flow.test.tsx`
- Test: `apps/runtime/src-tauri/tests/test_runtime_preferences.rs`

**Step 1: Write the failing frontend and backend tests**

- Extend the settings tests to assert that `设置 > 桌面` renders exactly two options: `标准模式（推荐）` and `全自动模式`.
- Add a runtime preference test that verifies a new `operation_permission_mode` field defaults to `standard` and can be persisted as `full_access`.

**Step 2: Run the targeted tests to verify failure**

Run:

```bash
pnpm --filter runtime test -- SettingsView.risk-flow.test.tsx
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_runtime_preferences
```

Expected:
- The frontend test fails because the new settings control does not exist yet.
- The Rust test fails because `operation_permission_mode` is not part of runtime preferences yet.

**Step 3: Implement minimal preference persistence**

- Add a frontend type for desktop permission mode with only `standard | full_access`.
- Add `operation_permission_mode` to runtime preference payloads and defaults.
- Update Tauri runtime preference read/write logic to normalize invalid values back to `standard`.
- Add a small desktop settings section in `SettingsView` that loads and saves the new preference.

**Step 4: Run the targeted tests to verify they pass**

Run:

```bash
pnpm --filter runtime test -- SettingsView.risk-flow.test.tsx
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_runtime_preferences
```

Expected:
- Both test groups pass.

**Step 5: Commit**

```bash
git add apps/runtime/src/types.ts apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/__tests__/SettingsView.risk-flow.test.tsx apps/runtime/src-tauri/src/commands/runtime_preferences.rs apps/runtime/src-tauri/tests/test_runtime_preferences.rs
git commit -m "feat: persist desktop permission mode"
```

### Task 2: Remove sidebar permission controls and session-scoped mode selection

**Files:**
- Modify: `apps/runtime/src/App.tsx`
- Modify: `apps/runtime/src/components/Sidebar.tsx`
- Test: `apps/runtime/src/components/__tests__/Sidebar.risk-flow.test.tsx`
- Test: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`

**Step 1: Write the failing tests**

- Replace the sidebar risk-flow test with assertions that the sidebar no longer shows the permission combobox.
- Add or update an app-level session creation test to assert new sessions use the persisted desktop mode rather than a sidebar-selected mode.

**Step 2: Run the targeted tests to verify failure**

Run:

```bash
pnpm --filter runtime test -- Sidebar.risk-flow.test.tsx App.session-create-flow.test.tsx
```

Expected:
- Tests fail because sidebar still renders the mode selector and app state still owns `newSessionPermissionMode`.

**Step 3: Implement the UI simplification**

- Remove the sidebar props and state related to permission mode selection.
- Remove the unrestricted-mode confirmation dialog from the sidebar.
- Update app session creation flow to source the permission mode from runtime preferences loaded at app level.

**Step 4: Run the targeted tests to verify they pass**

Run:

```bash
pnpm --filter runtime test -- Sidebar.risk-flow.test.tsx App.session-create-flow.test.tsx
```

Expected:
- The sidebar no longer exposes permission controls.
- Session creation uses the new desktop preference.

**Step 5: Commit**

```bash
git add apps/runtime/src/App.tsx apps/runtime/src/components/Sidebar.tsx apps/runtime/src/components/__tests__/Sidebar.risk-flow.test.tsx apps/runtime/src/__tests__/App.session-create-flow.test.tsx
git commit -m "refactor: move permission control out of sidebar"
```

### Task 3: Add settings-level full-access confirmation and visibility

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/components/RiskConfirmDialog.tsx`
- Test: `apps/runtime/src/components/__tests__/SettingsView.risk-flow.test.tsx`

**Step 1: Write the failing tests**

- Add a settings test that switching from `标准模式` to `全自动模式` opens a confirmation dialog.
- Add a test that canceling the dialog keeps the saved mode unchanged.
- Add a test that confirming persists `full_access`.

**Step 2: Run the targeted tests to verify failure**

Run:

```bash
pnpm --filter runtime test -- SettingsView.risk-flow.test.tsx
```

Expected:
- Tests fail because the settings page does not yet gate `full_access` with a confirmation dialog.

**Step 3: Implement the settings flow**

- Add local pending-state handling in `SettingsView`.
- Reuse `RiskConfirmDialog` with the new product copy for `全自动模式`.
- Ensure switching back to `标准模式` is immediate and silent.

**Step 4: Run the targeted tests to verify they pass**

Run:

```bash
pnpm --filter runtime test -- SettingsView.risk-flow.test.tsx
```

Expected:
- All full-access settings interaction tests pass.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/RiskConfirmDialog.tsx apps/runtime/src/components/__tests__/SettingsView.risk-flow.test.tsx
git commit -m "feat: confirm full access mode in desktop settings"
```

### Task 4: Replace tool-name approvals with action-risk classification

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/permissions.rs`
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Test: `apps/runtime/src-tauri/tests/test_permissions.rs`
- Test: `apps/runtime/src-tauri/tests/test_bash.rs`
- Test: `apps/runtime/src-tauri/tests/test_file_delete.rs`
- Test: `apps/runtime/src-tauri/tests/test_browser_tools.rs`

**Step 1: Write the failing tests**

- Replace the current permission tests with risk-based cases:
  - standard mode does not confirm ordinary workspace edits
  - standard mode confirms delete actions
  - standard mode confirms out-of-workspace writes
  - standard mode does not confirm harmless commands
  - standard mode confirms dangerous commands
  - full access confirms nothing
- Add or update specific tool tests to pass representative tool inputs into the classifier.

**Step 2: Run the targeted Rust tests to verify failure**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_permissions
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_file_delete
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_browser_tools
```

Expected:
- Tests fail because permission logic is still keyed off tool names.

**Step 3: Implement minimal risk classification**

- Replace the three-value permission enum with `Standard` and `FullAccess`.
- Introduce an internal `ActionRisk` classifier that inspects:
  - file-operation intent and target path
  - command strings for destructive patterns
  - browser actions for submit/send/publish/confirm semantics
- Update executor confirmation checks to use `ActionRisk::Critical`.

**Step 4: Run the targeted Rust tests to verify they pass**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_permissions
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_file_delete
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_browser_tools
```

Expected:
- Risk-classification tests pass and tool confirmation is only triggered for critical actions in standard mode.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/permissions.rs apps/runtime/src-tauri/src/agent/executor.rs apps/runtime/src-tauri/tests/test_permissions.rs apps/runtime/src-tauri/tests/test_bash.rs apps/runtime/src-tauri/tests/test_file_delete.rs apps/runtime/src-tauri/tests/test_browser_tools.rs
git commit -m "feat: gate confirmations with action risk classification"
```

### Task 5: Improve confirmation payloads and user-facing copy

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/executor.rs`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/RiskConfirmDialog.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx`

**Step 1: Write the failing tests**

- Add or update chat risk-flow tests so the confirmation UI expects structured content:
  - action title
  - target summary
  - impact text
- Add a test case for a delete confirmation and a browser-submit confirmation payload.

**Step 2: Run the targeted tests to verify failure**

Run:

```bash
pnpm --filter runtime test -- ChatView.risk-flow.test.tsx
```

Expected:
- Tests fail because current tool-confirm payloads only expose raw tool name/input.

**Step 3: Implement structured confirmation metadata**

- Build a small formatter on the Rust side that converts critical actions into:
  - title
  - summary
  - impact
  - irreversible
- Update chat-side confirmation handling to render the normalized metadata instead of generic tool details.

**Step 4: Run the targeted tests to verify they pass**

Run:

```bash
pnpm --filter runtime test -- ChatView.risk-flow.test.tsx
```

Expected:
- Confirmation dialogs show user-readable impact details for critical actions.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/executor.rs apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/RiskConfirmDialog.tsx apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx
git commit -m "feat: show user-readable critical action confirmations"
```

### Task 6: Add visible full-access state and update documentation

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `docs/user-manual/08-security.md`
- Modify: `README.md`
- Modify: `README.en.md`
- Test: `apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx`

**Step 1: Write the failing tests**

- Add a UI test that shows a `全自动模式` badge when the persisted preference is `full_access`.
- Add a UI test that no badge appears in `标准模式`.

**Step 2: Run the targeted tests to verify failure**

Run:

```bash
pnpm --filter runtime test -- ChatView.risk-flow.test.tsx
```

Expected:
- Tests fail because no full-access state indicator exists yet.

**Step 3: Implement the minimal visibility and docs updates**

- Add a lightweight badge in the chat view or input area when full access is active.
- Link or hint back to desktop settings.
- Update the user manual and README copy to reflect the two-mode personal edition model.

**Step 4: Run the targeted tests to verify they pass**

Run:

```bash
pnpm --filter runtime test -- ChatView.risk-flow.test.tsx
```

Expected:
- Badge visibility tests pass.
- Docs reflect the new model.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/SettingsView.tsx docs/user-manual/08-security.md README.md README.en.md apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx
git commit -m "docs: align UI and docs with two-mode permission model"
```

### Task 7: Run final verification

**Files:**
- No code changes expected

**Step 1: Run focused frontend tests**

Run:

```bash
pnpm --filter runtime test -- SettingsView.risk-flow.test.tsx Sidebar.risk-flow.test.tsx ChatView.risk-flow.test.tsx App.session-create-flow.test.tsx
```

Expected:
- All targeted frontend tests pass.

**Step 2: Run focused Rust tests**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_permissions
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_runtime_preferences
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_file_delete
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_browser_tools
```

Expected:
- All focused Rust tests pass.

**Step 3: Run one broader smoke suite**

Run:

```bash
pnpm --filter runtime test
```

Expected:
- No regression in adjacent settings/chat/sidebar flows.

**Step 4: Summarize residual risks**

- Document any remaining classifier blind spots, especially around:
  - key-file overwrite detection
  - long or obfuscated shell commands
  - browser actions whose labels are unavailable

**Step 5: Commit verification notes if needed**

```bash
git status --short
```
