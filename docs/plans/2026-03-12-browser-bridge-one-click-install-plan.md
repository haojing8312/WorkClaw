# Browser Bridge One-Click Install Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a one-click browser bridge installer in the existing settings tab that prepares the Chrome bridge environment and guides the user through the final Chrome enablement step.

**Architecture:** Keep orchestration in Tauri commands and the settings UI. Reuse the existing native-host installation logic and browser bridge pieces, but add a product-facing install status model and a lightweight extension handshake to detect successful enablement.

**Tech Stack:** Tauri 2, Rust, React 18, TypeScript, Vitest, Node helper scripts

---

### Task 1: Add Browser Bridge Install Types

**Files:**
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx`

**Step 1: Write the failing test**

Add a frontend test expectation for an install status payload shaped like:

```ts
{
  state: "not_installed",
  chrome_found: true,
  native_host_installed: false,
  extension_dir_ready: false,
  bridge_connected: false,
  last_error: null,
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime test src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx
```

Expected: FAIL because the type/usage is missing.

**Step 3: Write minimal implementation**

Add interfaces in `apps/runtime/src/types.ts`:

- `BrowserBridgeInstallState`
- `BrowserBridgeInstallStatus`

**Step 4: Run test to verify it passes**

Run the same test command.

**Step 5: Commit**

```bash
git add apps/runtime/src/types.ts apps/runtime/src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx
git commit -m "feat(browser-bridge): add install status types"
```

### Task 2: Add Browser Bridge Install Card UI

**Files:**
- Create: `apps/runtime/src/components/employees/BrowserBridgeInstallCard.tsx`
- Create: `apps/runtime/src/components/employees/__tests__/BrowserBridgeInstallCard.test.tsx`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`

**Step 1: Write the failing test**

Add tests that render the card in:

- `not_installed`
- `installing`
- `waiting_for_enable`
- `connected`
- `failed`

For example:

```tsx
expect(screen.getByText("浏览器桥接安装")).toBeInTheDocument();
expect(screen.getByRole("button", { name: "安装浏览器桥接" })).toBeInTheDocument();
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime test src/components/employees/__tests__/BrowserBridgeInstallCard.test.tsx
```

Expected: FAIL because the component does not exist.

**Step 3: Write minimal implementation**

Create a small presentational card that accepts:

- `status`
- `installing`
- `onInstall`
- `onOpenExtensionPage`
- `onOpenExtensionDir`
- `onStartFeishuSetup`

Wire it into the existing settings tab in `EmployeeHubView.tsx`.

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --dir apps/runtime test src/components/employees/__tests__/BrowserBridgeInstallCard.test.tsx src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx
```

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/BrowserBridgeInstallCard.tsx apps/runtime/src/components/employees/__tests__/BrowserBridgeInstallCard.test.tsx apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx
git commit -m "feat(browser-bridge): add install card to settings"
```

### Task 3: Add Tauri Install Command Surface

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/browser_bridge_install.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_browser_bridge_install.rs`

**Step 1: Write the failing test**

Add Rust tests for:

- default status when nothing is installed
- error when Chrome path cannot be resolved

Example:

```rust
assert_eq!(status.state, "not_installed");
assert!(!status.native_host_installed);
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_browser_bridge_install -- --nocapture
```

Expected: FAIL because command module and types do not exist.

**Step 3: Write minimal implementation**

Create:

- `BrowserBridgeInstallStatus`
- `get_browser_bridge_install_status`
- stub `install_browser_bridge`
- `open_browser_bridge_extension_page`
- `open_browser_bridge_extension_dir`

Register commands in `lib.rs`.

**Step 4: Run test to verify it passes**

Run the same Rust test command.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/browser_bridge_install.rs apps/runtime/src-tauri/src/commands/mod.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_browser_bridge_install.rs
git commit -m "feat(browser-bridge): add install command surface"
```

### Task 4: Implement Windows Path Detection and Host Install

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/browser_bridge_install.rs`
- Reference: `scripts/install-chrome-native-host.mjs`
- Reference: `scripts/workclaw-chrome-native-host.mjs`
- Test: `apps/runtime/src-tauri/tests/test_browser_bridge_install.rs`

**Step 1: Write the failing test**

Add test coverage for:

- launcher path generation
- manifest path generation
- status changing to `waiting_for_enable` after install

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_browser_bridge_install -- --nocapture
```

Expected: FAIL because install logic is still stubbed.

**Step 3: Write minimal implementation**

Implement Windows-only installation logic:

- resolve Chrome user data dir
- resolve Node path
- write launcher file
- write native host manifest
- ensure extension directory exists / is exposed

Return a status like:

```rust
state: "waiting_for_enable"
```

**Step 4: Run test to verify it passes**

Run the same Rust test command.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/browser_bridge_install.rs apps/runtime/src-tauri/tests/test_browser_bridge_install.rs
git commit -m "feat(browser-bridge): install native host on windows"
```

### Task 5: Wire Frontend Install Flow

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx`

**Step 1: Write the failing test**

Add a test that:

- clicks `安装浏览器桥接`
- calls `install_browser_bridge`
- enters `等待启用`

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime test src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx
```

Expected: FAIL because the install command is not wired.

**Step 3: Write minimal implementation**

In `EmployeeHubView.tsx`:

- load install status on mount
- invoke install command on click
- poll status while `installing` or `waiting_for_enable`

**Step 4: Run test to verify it passes**

Run the same test command.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx
git commit -m "feat(browser-bridge): wire install flow in settings"
```

### Task 6: Add Extension Handshake Detection

**Files:**
- Modify: `apps/runtime/src/browser-bridge/chrome-extension/background.ts`
- Modify: `apps/runtime/src/browser-bridge/chrome-extension/content.ts`
- Modify: `apps/runtime/sidecar/src/index.ts` or relevant local callback path if needed
- Modify: `apps/runtime/src-tauri/src/commands/browser_bridge_install.rs`
- Test: `apps/runtime/src/browser-bridge/chrome-extension/__tests__/background.test.ts`
- Test: `apps/runtime/src/browser-bridge/chrome-extension/__tests__/content.test.ts`
- Test: `apps/runtime/src-tauri/tests/test_browser_bridge_install.rs`

**Step 1: Write the failing test**

Add tests that expect:

- extension emits a lightweight `hello`
- desktop install status becomes `connected`

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime test src/browser-bridge/chrome-extension/__tests__/background.test.ts src/browser-bridge/chrome-extension/__tests__/content.test.ts
```

Expected: FAIL because no handshake exists.

**Step 3: Write minimal implementation**

Implement the smallest viable handshake:

- content/background sends local hello after load
- desktop stores last seen timestamp
- status uses recent heartbeat to mark `connected`

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --dir apps/runtime test src/browser-bridge/chrome-extension/__tests__/background.test.ts src/browser-bridge/chrome-extension/__tests__/content.test.ts
```

Also run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_browser_bridge_install -- --nocapture
```

**Step 5: Commit**

```bash
git add apps/runtime/src/browser-bridge/chrome-extension/background.ts apps/runtime/src/browser-bridge/chrome-extension/content.ts apps/runtime/sidecar/src/index.ts apps/runtime/src-tauri/src/commands/browser_bridge_install.rs apps/runtime/src/browser-bridge/chrome-extension/__tests__/background.test.ts apps/runtime/src/browser-bridge/chrome-extension/__tests__/content.test.ts apps/runtime/src-tauri/tests/test_browser_bridge_install.rs
git commit -m "feat(browser-bridge): detect extension handshake"
```

### Task 7: Add Docs and Operator Guidance

**Files:**
- Modify: `docs/integrations/feishu-browser-setup.md`
- Modify: `README.md`
- Modify: `README.en.md`

**Step 1: Write the failing doc check**

Manually verify the docs are missing:

- one-click install description
- final Chrome confirmation wording
- troubleshooting steps

**Step 2: Run doc update**

Update docs to include:

- settings-page install flow
- last-step Chrome confirmation
- known limitations

**Step 3: Verify docs**

Open the changed markdown files and confirm links/paths are correct.

**Step 4: Commit**

```bash
git add docs/integrations/feishu-browser-setup.md README.md README.en.md
git commit -m "docs(browser-bridge): document one-click install flow"
```

### Task 8: Final Verification

**Files:**
- Verify all touched files

**Step 1: Run frontend verification**

```bash
pnpm --dir apps/runtime test src/browser-bridge/chrome-extension/__tests__/background.test.ts src/browser-bridge/chrome-extension/__tests__/content.test.ts src/browser-bridge/chrome-extension/__tests__/feishu-detector.test.ts src/browser-bridge/native-host/__tests__/native-host.test.ts src/components/employees/__tests__/BrowserBridgeInstallCard.test.tsx src/components/employees/__tests__/FeishuBrowserSetupView.test.tsx src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx
```

Expected: PASS

**Step 2: Run sidecar verification**

```bash
cd apps/runtime/sidecar
npx tsx --test test/browser-bridge-endpoints.test.ts
```

Expected: PASS

**Step 3: Run Rust verification**

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_browser_bridge_install --test test_browser_bridge_callback --test test_feishu_browser_setup --test test_feishu_browser_setup_binding -- --nocapture
```

Expected: PASS

**Step 4: Commit any final merge/fixups**

```bash
git status
```

If clean, no additional code commit required.
