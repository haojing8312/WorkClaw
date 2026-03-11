# Multi-Channel Platform Hardening P0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Harden the connector platform for Feishu and WeCom by adding unified connector metadata, diagnostics, and replay support without expanding into P1/P2 work.

**Architecture:** The sidecar kernel becomes the platform contract owner for connector catalog, diagnostics, acknowledgement, and replay. Rust relays that contract through generic commands. The desktop UI consumes the contract through a dedicated diagnostics panel and capability-aware connector surfaces.

**Tech Stack:** TypeScript, Hono, Vitest, Rust, Tauri commands, React, Playwright

---

### Task 1: Write the P0 sidecar contract tests

**Files:**
- Modify: `apps/runtime/sidecar/test/channel-endpoints.test.ts`
- Modify: `apps/runtime/sidecar/test/wecom-adapter.test.ts`

**Step 1: Write the failing tests**

Add tests for:

- `POST /api/channels/catalog` returns connector metadata and capability lists
- `POST /api/channels/diagnostics` returns normalized issue fields
- `POST /api/channels/ack` updates retained event ack state
- `POST /api/channels/replay-events` returns drained events for a connector instance

**Step 2: Run tests to verify they fail**

Run:

```bash
pnpm --dir apps/runtime/sidecar test -- channel-endpoints wecom-adapter
```

Expected: FAIL because the new endpoints and replay bookkeeping do not exist yet.

**Step 3: Commit**

```bash
git add apps/runtime/sidecar/test/channel-endpoints.test.ts apps/runtime/sidecar/test/wecom-adapter.test.ts
git commit -m "test(sidecar): define connector platform p0 contract"
```

### Task 2: Implement sidecar catalog, diagnostics, ack, and replay

**Files:**
- Modify: `apps/runtime/sidecar/src/adapters/types.ts`
- Modify: `apps/runtime/sidecar/src/adapters/kernel.ts`
- Modify: `apps/runtime/sidecar/src/index.ts`
- Modify: `apps/runtime/sidecar/src/adapters/feishu/index.ts`
- Modify: `apps/runtime/sidecar/src/adapters/wecom/index.ts`

**Step 1: Write the minimal implementation**

Implement:

- connector metadata and capability types
- structured connector issue type
- kernel-owned replay store and ack bookkeeping
- catalog, diagnostics, ack, and replay sidecar endpoints
- adapter metadata exposure for Feishu and WeCom

**Step 2: Run tests to verify they pass**

Run:

```bash
pnpm --dir apps/runtime/sidecar test -- channel-endpoints wecom-adapter
```

Expected: PASS

**Step 3: Commit**

```bash
git add apps/runtime/sidecar/src/adapters/types.ts apps/runtime/sidecar/src/adapters/kernel.ts apps/runtime/sidecar/src/index.ts apps/runtime/sidecar/src/adapters/feishu/index.ts apps/runtime/sidecar/src/adapters/wecom/index.ts apps/runtime/sidecar/test/channel-endpoints.test.ts apps/runtime/sidecar/test/wecom-adapter.test.ts
git commit -m "feat(sidecar): add connector platform diagnostics and replay"
```

### Task 3: Write runtime bridge tests for the new connector contract

**Files:**
- Create: `apps/runtime/src-tauri/tests/test_channel_connectors.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`

**Step 1: Write the failing tests**

Add tests for:

- listing connector catalog through a generic runtime command
- fetching diagnostics through a generic runtime command
- forwarding ack requests
- forwarding replay requests

**Step 2: Run tests to verify they fail**

Run:

```bash
cd apps/runtime/src-tauri
cargo test -j 1 --test test_channel_connectors -- --nocapture
```

Expected: FAIL because the command module does not exist yet.

**Step 3: Commit**

```bash
git add apps/runtime/src-tauri/tests/test_channel_connectors.rs apps/runtime/src-tauri/src/commands/mod.rs
git commit -m "test(runtime): define generic connector diagnostics contract"
```

### Task 4: Implement runtime connector commands

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/channel_connectors.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`

**Step 1: Write the minimal implementation**

Implement generic commands backed by `call_sidecar_json`:

- `list_channel_connectors`
- `get_channel_connector_diagnostics`
- `ack_channel_events`
- `replay_channel_events`

Register them in the Tauri command list.

**Step 2: Run tests to verify they pass**

Run:

```bash
cd apps/runtime/src-tauri
cargo test -j 1 --test test_channel_connectors -- --nocapture
```

Expected: PASS

**Step 3: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/channel_connectors.rs apps/runtime/src-tauri/src/commands/feishu_gateway.rs apps/runtime/src-tauri/src/commands/mod.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_channel_connectors.rs
git commit -m "feat(runtime): expose connector diagnostics and replay commands"
```

### Task 5: Write frontend diagnostics tests

**Files:**
- Create: `apps/runtime/src/components/connectors/__tests__/ConnectorDiagnosticsPanel.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.wecom-connector.test.tsx`
- Modify: `apps/runtime/src/types.ts`

**Step 1: Write the failing tests**

Add tests for:

- a dedicated diagnostics panel rendering normalized status
- capability chips rendering from connector metadata
- user message vs technical message disclosure behavior

**Step 2: Run tests to verify they fail**

Run:

```bash
pnpm --dir apps/runtime test -- ConnectorDiagnosticsPanel SettingsView.wecom-connector
```

Expected: FAIL because the standalone panel does not exist yet.

**Step 3: Commit**

```bash
git add apps/runtime/src/components/connectors/__tests__/ConnectorDiagnosticsPanel.test.tsx apps/runtime/src/components/__tests__/SettingsView.wecom-connector.test.tsx apps/runtime/src/types.ts
git commit -m "test(ui): define connector diagnostics panel behavior"
```

### Task 6: Implement the diagnostics panel and connector metadata flow

**Files:**
- Create: `apps/runtime/src/components/connectors/ConnectorDiagnosticsPanel.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/components/connectors/ConnectorConfigPanel.tsx`
- Modify: `apps/runtime/src/components/connectors/connectorSchemas.ts`
- Modify: `apps/runtime/src/types.ts`

**Step 1: Write the minimal implementation**

Implement:

- frontend connector metadata and diagnostics types
- dedicated diagnostics panel component
- capability chip rendering
- normalized status rendering
- technical details disclosure

Wire the panel into the settings experience without changing existing rule editing behavior.

**Step 2: Run tests to verify they pass**

Run:

```bash
pnpm --dir apps/runtime test -- ConnectorDiagnosticsPanel SettingsView.wecom-connector SettingsView.feishu SettingsView.feishu-routing-wizard
pnpm --dir apps/runtime exec playwright test e2e/im-connectors.feishu.spec.ts e2e/im-connectors.wecom.spec.ts
```

Expected: PASS

**Step 3: Commit**

```bash
git add apps/runtime/src/components/connectors/ConnectorDiagnosticsPanel.tsx apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/connectors/ConnectorConfigPanel.tsx apps/runtime/src/components/connectors/connectorSchemas.ts apps/runtime/src/types.ts apps/runtime/src/components/connectors/__tests__/ConnectorDiagnosticsPanel.test.tsx apps/runtime/src/components/__tests__/SettingsView.wecom-connector.test.tsx
git commit -m "feat(ui): add connector diagnostics panel"
```

### Task 7: Run end-to-end verification

**Files:**
- No new files

**Step 1: Run the targeted verification suite**

Run:

```bash
pnpm --dir apps/runtime/sidecar test -- channel-endpoints wecom-adapter
pnpm --dir apps/runtime test -- ConnectorDiagnosticsPanel SettingsView.wecom-connector SettingsView.feishu SettingsView.feishu-routing-wizard
pnpm --dir apps/runtime exec playwright test e2e/im-connectors.feishu.spec.ts e2e/im-connectors.wecom.spec.ts
cd apps/runtime/src-tauri
cargo test -j 1 --test test_channel_connectors -- --nocapture
```

Expected: PASS

**Step 2: Commit**

```bash
git add .
git commit -m "test: verify connector platform hardening p0"
```
