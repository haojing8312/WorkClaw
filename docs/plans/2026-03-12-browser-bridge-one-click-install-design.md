# Browser Bridge One-Click Install Design

**Goal:** Add a beginner-friendly one-click browser bridge installer in the existing settings tab so users can prepare the Chrome bridge environment without manual CLI steps.

## Context

WorkClaw already has the core Feishu browser bridge pieces:

- Chrome extension source under `apps/runtime/src/browser-bridge/chrome-extension/`
- Native host install script at `scripts/install-chrome-native-host.mjs`
- Native host runner at `scripts/workclaw-chrome-native-host.mjs`
- Feishu browser setup entry in `apps/runtime/src/components/employees/EmployeeHubView.tsx`

What is missing is the productized install flow for ordinary users. Today, users still need to understand Chrome extension loading, launcher paths, and Native Messaging registration.

## Product Definition

The feature should be presented as:

`Install Browser Bridge (final Chrome confirmation required)`

It is intentionally not a silent extension installer. The app will automatically prepare the local bridge environment and guide the user through the final Chrome enablement step.

## User Experience

The feature lives in the existing `settings` tab inside `EmployeeHubView`.

Add a new card above the existing Feishu browser setup card:

- Title: `浏览器桥接安装`
- Subtitle: `自动安装本地桥接，并引导你在 Chrome 中完成最后一步启用`

### States

1. `NotInstalled`
- Primary button: `安装浏览器桥接`

2. `Installing`
- Show step text:
  - `正在检测 Chrome`
  - `正在安装本地桥接`
  - `正在准备扩展目录`
  - `正在打开 Chrome`

3. `WaitingForEnable`
- Explain the last user action in one sentence:
  - `请在 Chrome 扩展页开启开发者模式，并加载已为你准备好的 WorkClaw 扩展目录`
- Secondary actions:
  - `重新打开 Chrome 扩展页`
  - `打开扩展目录`

4. `Connected`
- Show success:
  - `浏览器桥接已启用，可以开始飞书配置`
- Primary button:
  - `启动飞书浏览器配置`

5. `Failed`
- Show concrete reason
- Action:
  - `重试安装`

## Scope

### In Scope

- Automatic Native Messaging host installation on Windows
- Automatic launcher generation
- Automatic bridge extension directory preparation
- Automatic opening of Chrome extension page and extension directory
- Desktop UI state tracking and error messaging
- Extension connection detection through a lightweight handshake

### Out of Scope

- Silent Chrome extension installation
- Enterprise policy installation
- Non-Chrome browsers
- Automatic full Feishu backend configuration after bridge installation

## Technical Approach

Use a Tauri-orchestrated installation flow. The app should own path detection, file generation, and user guidance. Existing Node scripts are reused as building blocks, but orchestration and install status should live in desktop commands instead of being hidden in a script-only flow.

The final user-facing flow is:

1. User clicks install
2. Tauri command resolves Chrome and Node paths
3. Tauri command writes launcher + Native Messaging manifest
4. Tauri command prepares extension folder path
5. Tauri opens Chrome extension UI and extension directory
6. Frontend switches to `WaitingForEnable`
7. Extension handshake changes status to `Connected`

## Architecture

### Frontend

Modify `apps/runtime/src/components/employees/EmployeeHubView.tsx`:

- Add browser bridge install card state
- Poll install status while in `Installing` or `WaitingForEnable`
- Reuse existing `open_external_url` patterns

Potential new UI helper:

- `apps/runtime/src/components/employees/BrowserBridgeInstallCard.tsx`

### Tauri Commands

Add a new command module:

- `apps/runtime/src-tauri/src/commands/browser_bridge_install.rs`

Commands:

- `get_browser_bridge_install_status`
- `install_browser_bridge`
- `open_browser_bridge_extension_page`
- `open_browser_bridge_extension_dir`

### Native Host Preparation

Reuse logic conceptually from:

- `scripts/install-chrome-native-host.mjs`
- `scripts/workclaw-chrome-native-host.mjs`

The product flow should not require the frontend to manually assemble script arguments.

### Bridge Connectivity Detection

Use a minimal handshake:

- Extension/background sends a local `hello` or heartbeat after load
- Desktop stores last-seen bridge heartbeat
- Install status treats recent heartbeat as `Connected`

This keeps the first version simple and avoids over-designing a separate control protocol.

## Error Handling

Need explicit user-facing messages for:

- Chrome user data directory not found
- Node executable not found
- Native host manifest write failed
- Launcher write failed
- Extension directory missing or unreadable
- Chrome page open failed
- Bridge not connected after user enablement

## Testing Strategy

### Frontend

- Install card state rendering
- Install start action
- Waiting-for-enable polling
- Connected state transition

### Tauri

- Path resolution
- Install status aggregation
- Launcher / manifest write flow
- Failure branches

### Scripts / Integration

- Keep manifest / launcher generation tests
- Add install status integration tests where practical

### Manual

Run a fresh-machine Windows install flow:

1. Click install
2. Confirm Chrome page opens
3. Enable extension in Chrome
4. Confirm app auto-detects `Connected`

## Recommendation

This is the correct first version for ordinary users. It is not truly silent installation, but it provides the closest trustworthy experience within Chrome's security model:

- automatic local setup
- minimal manual Chrome action
- app-owned success detection

That is materially better than documentation-driven setup and good enough to ship before any enterprise silent-install path exists.
