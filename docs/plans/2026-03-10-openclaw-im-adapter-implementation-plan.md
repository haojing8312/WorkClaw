# OpenClaw IM Adapter Boundary Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor WorkClaw's IM integration into a stable sidecar adapter boundary so Feishu becomes the first connector and future OpenClaw-supported IM channels can be added with minimal business-layer changes.

**Architecture:** Introduce a sidecar-owned `ChannelAdapter` kernel and normalized IM event model, wrap the existing Feishu implementation behind that boundary, then update Rust and frontend layers to consume channel-neutral interfaces while preserving compatibility aliases. Keep all OpenClaw-derived logic isolated to the sidecar boundary and continue reusing the existing OpenClaw routing subset through a channel-neutral route input.

**Tech Stack:** TypeScript sidecar (Hono, Node.js), Rust/Tauri runtime (`sqlx`, commands, event bridge), React/TypeScript frontend, existing Vitest and Rust test suites.

---

### Task 1: Freeze the sidecar adapter ABI

**Files:**
- Create: `apps/runtime/sidecar/src/adapters/types.ts`
- Create: `apps/runtime/sidecar/src/adapters/registry.ts`
- Create: `apps/runtime/sidecar/src/adapters/kernel.ts`
- Test: `apps/runtime/sidecar/test/adapters/kernel.test.ts`

**Step 1: Write the failing test**

Add tests that assert:

- adapters can be registered by name
- `start` returns an `instanceId`
- `health` returns adapter state by instance
- `drainEvents` delegates to the registered adapter
- unknown adapter name returns a controlled error

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar test -- kernel`

Expected: FAIL because adapter kernel files do not exist yet.

**Step 3: Write minimal implementation**

Define:

- `NormalizedImEvent`
- `RoutingContext`
- `SendMessageRequest`
- `SendMessageResult`
- `AdapterHealth`
- `ChannelAdapter`
- adapter registry helpers
- kernel methods for `start`, `stop`, `health`, `drainEvents`, `sendMessage`, `ack`

Keep the API small and channel-neutral.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar test -- kernel`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/adapters/types.ts apps/runtime/sidecar/src/adapters/registry.ts apps/runtime/sidecar/src/adapters/kernel.ts apps/runtime/sidecar/test/adapters/kernel.test.ts
git commit -m "feat(sidecar): add channel adapter kernel"
```

### Task 2: Wrap the existing Feishu sidecar implementation as an adapter

**Files:**
- Create: `apps/runtime/sidecar/src/adapters/feishu/index.ts`
- Create: `apps/runtime/sidecar/src/adapters/feishu/normalize.ts`
- Modify: `apps/runtime/sidecar/src/feishu.ts`
- Modify: `apps/runtime/sidecar/src/feishu_ws.ts`
- Test: `apps/runtime/sidecar/test/adapters/feishu-adapter.test.ts`

**Step 1: Write the failing test**

Add tests that assert:

- Feishu adapter implements `ChannelAdapter`
- incoming Feishu payloads normalize into `NormalizedImEvent`
- adapter `health` maps websocket/client state consistently
- outbound `sendMessage` uses the existing Feishu sender path

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar test -- feishu-adapter`

Expected: FAIL because no adapter wrapper exists.

**Step 3: Write minimal implementation**

Move Feishu-specific behavior behind adapter methods:

- adapter startup
- event queueing
- normalization
- outbound messaging
- stop/reconnect/health reporting

Do not remove old Feishu classes yet. Wrap them first.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar test -- feishu-adapter`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/adapters/feishu/index.ts apps/runtime/sidecar/src/adapters/feishu/normalize.ts apps/runtime/sidecar/src/feishu.ts apps/runtime/sidecar/src/feishu_ws.ts apps/runtime/sidecar/test/adapters/feishu-adapter.test.ts
git commit -m "refactor(sidecar): wrap feishu as channel adapter"
```

### Task 3: Route sidecar HTTP endpoints through the adapter kernel

**Files:**
- Modify: `apps/runtime/sidecar/src/index.ts`
- Test: `apps/runtime/sidecar/test/index.channel-endpoints.test.ts`

**Step 1: Write the failing test**

Add tests that assert:

- sidecar exposes channel-neutral adapter endpoints
- Feishu compatibility endpoints forward into the adapter kernel
- route resolution still accepts a channel-neutral payload

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar test -- channel-endpoints`

Expected: FAIL because the endpoints are still Feishu-specialized.

**Step 3: Write minimal implementation**

Add kernel-backed endpoints such as:

- `/api/channels/start`
- `/api/channels/stop`
- `/api/channels/health`
- `/api/channels/drain-events`
- `/api/channels/send-message`

Keep current Feishu endpoints working by translating them into kernel requests.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar test -- channel-endpoints`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/index.ts apps/runtime/sidecar/test/index.channel-endpoints.test.ts
git commit -m "feat(sidecar): expose channel-neutral adapter endpoints"
```

### Task 4: Generalize the Rust IM event model

**Files:**
- Modify: `apps/runtime/src-tauri/src/im/types.rs`
- Modify: `apps/runtime/src-tauri/src/commands/im_gateway.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_gateway.rs`
- Test: `apps/runtime/src-tauri/tests/test_openclaw_gateway.rs`
- Test: `apps/runtime/src-tauri/tests/test_openclaw_route_regression.rs`

**Step 1: Write the failing test**

Add tests that assert:

- normalized ingress supports non-Feishu channel metadata
- `handle_openclaw_event` accepts generic channel event payloads
- route payload assembly no longer assumes Feishu-only defaults except where compatibility alias applies

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_openclaw_gateway --test test_openclaw_route_regression -- --nocapture`

Expected: FAIL because current code remains Feishu-biased.

**Step 3: Write minimal implementation**

Refactor:

- `ImEvent` into a channel-neutral structure, or introduce `UnifiedImEvent`
- `process_im_event` to store dynamic channel source
- route assembly to consume generic `routing_context`
- `handle_feishu_callback` to remain as a forwarding compatibility alias

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_openclaw_gateway --test test_openclaw_route_regression -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/im/types.rs apps/runtime/src-tauri/src/commands/im_gateway.rs apps/runtime/src-tauri/src/commands/openclaw_gateway.rs apps/runtime/src-tauri/tests/test_openclaw_gateway.rs apps/runtime/src-tauri/tests/test_openclaw_route_regression.rs
git commit -m "refactor(runtime): generalize im ingress for channel adapters"
```

### Task 5: Generalize IM routing persistence and simulation

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/im_routing.rs`
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_routing.rs`

**Step 1: Write the failing test**

Add tests that assert:

- bindings persist channel-neutral routing fields
- route simulation supports connectors beyond Feishu
- existing Feishu rows remain readable

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test test_im_routing -- --nocapture`

Expected: FAIL because routing persistence still carries Feishu-era assumptions.

**Step 3: Write minimal implementation**

Update persistence to support:

- generalized channel identity fields
- extensible connector metadata
- compatibility reads for existing Feishu records

Prefer additive migration over destructive schema changes.

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test test_im_routing -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/im_routing.rs apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/tests/test_im_routing.rs
git commit -m "refactor(runtime): generalize im routing bindings"
```

### Task 6: Convert frontend Feishu config into connector-backed UI state

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/components/employees/FeishuRoutingWizard.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Create: `apps/runtime/src/components/connectors/ConnectorConfigPanel.tsx`
- Create: `apps/runtime/src/components/connectors/connectorSchemas.ts`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.thread-binding.test.tsx`

**Step 1: Write the failing test**

Add tests that assert:

- Feishu renders as connector `feishu`
- connector health/status renders from generic connector state
- existing Feishu user flows still work through the new connector abstraction

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- EmployeeHubView.feishu-connection-status`

Expected: FAIL because the UI is still Feishu-specific.

**Step 3: Write minimal implementation**

Introduce:

- connector schema registry
- connector config panel
- Feishu schema as the first connector
- forwarding from old Feishu-specific UI controls into generic connector actions

Keep visible product behavior stable for Feishu users.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime test -- EmployeeHubView.feishu-connection-status`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/FeishuRoutingWizard.tsx apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/connectors/ConnectorConfigPanel.tsx apps/runtime/src/components/connectors/connectorSchemas.ts apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.thread-binding.test.tsx
git commit -m "refactor(ui): add connector-backed im configuration"
```

### Task 7: Add adapter observability and diagnostics

**Files:**
- Modify: `apps/runtime/sidecar/src/adapters/kernel.ts`
- Modify: `apps/runtime/src-tauri/src/commands/desktop_lifecycle.rs`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Test: `apps/runtime/sidecar/test/adapters/kernel.test.ts`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx`

**Step 1: Write the failing test**

Add tests that assert:

- adapter health includes last success/error and reconnect attempts
- UI can surface connector diagnostics without channel-specific branching

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar test -- kernel`

Expected: FAIL because health payload is too narrow.

**Step 3: Write minimal implementation**

Extend health reporting and expose it to Tauri/UI so connector status becomes consistent and reusable for future channels.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar test -- kernel`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/adapters/kernel.ts apps/runtime/src-tauri/src/commands/desktop_lifecycle.rs apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/sidecar/test/adapters/kernel.test.ts apps/runtime/src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx
git commit -m "feat(connectors): add adapter diagnostics"
```

### Task 8: Lock down Feishu compatibility and regression coverage

**Files:**
- Modify: `apps/runtime/e2e/skill-library-cache.spec.ts`
- Create: `apps/runtime/e2e/im-connectors.feishu.spec.ts`
- Modify: `docs/integrations/feishu-routing.md`
- Modify: `docs/browser-automation-integration.md`

**Step 1: Write the failing test**

Add end-to-end or integration coverage that asserts:

- existing Feishu onboarding still works
- route simulation still returns expected routing fields
- connector state survives restart/reconcile paths

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test:e2e`

Expected: FAIL or missing coverage before compatibility assertions are added.

**Step 3: Write minimal implementation**

Patch compatibility gaps discovered by the new tests. Keep changes focused on forwarding and serialization, not new product features.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime test:e2e`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/e2e/skill-library-cache.spec.ts apps/runtime/e2e/im-connectors.feishu.spec.ts docs/integrations/feishu-routing.md docs/browser-automation-integration.md
git commit -m "test(connectors): lock feishu compatibility through adapter boundary"
```

### Task 9: Prepare the second-channel vendor lane without enabling a channel

**Files:**
- Create: `scripts/sync-openclaw-im-core.mjs`
- Create: `apps/runtime/sidecar/vendor/openclaw-im-core/README.md`
- Create: `apps/runtime/sidecar/vendor/openclaw-im-core/PATCHES.md`
- Create: `apps/runtime/sidecar/vendor/openclaw-im-core/UPSTREAM_COMMIT`
- Modify: `docs/maintainers/openclaw-upgrade.md`

**Step 1: Write the failing test**

Add a lightweight script test or maintainers check that asserts the new vendor lane has:

- sync script
- upstream pin file
- patch log

**Step 2: Run test to verify it fails**

Run: `node --test scripts/check-openclaw-vendor-lane.test.mjs`

Expected: FAIL because the vendor lane does not exist.

**Step 3: Write minimal implementation**

Create the vendor maintenance lane and update the maintainer runbook so future Slack/Discord-style adoption follows the same discipline as the current routing subset.

**Step 4: Run test to verify it passes**

Run: `node --test scripts/check-openclaw-vendor-lane.test.mjs`

Expected: PASS.

**Step 5: Commit**

```bash
git add scripts/sync-openclaw-im-core.mjs apps/runtime/sidecar/vendor/openclaw-im-core/README.md apps/runtime/sidecar/vendor/openclaw-im-core/PATCHES.md apps/runtime/sidecar/vendor/openclaw-im-core/UPSTREAM_COMMIT docs/maintainers/openclaw-upgrade.md
git commit -m "chore(sidecar): add vendor lane for openclaw im adapters"
```

### Task 10: Full regression verification

**Files:**
- No new files required

**Step 1: Run sidecar unit tests**

Run: `pnpm --dir apps/runtime/sidecar test`

Expected: PASS.

**Step 2: Run runtime frontend tests**

Run: `pnpm --dir apps/runtime test`

Expected: PASS.

**Step 3: Run Rust route and runtime tests**

Run: `cd apps/runtime/src-tauri && cargo test -- --nocapture`

Expected: PASS.

**Step 4: Run targeted browser automation regression**

Run: `pnpm test:browser-automation`

Expected: PASS.

**Step 5: Commit verification-only state if needed**

```bash
git status
```

Expected: clean worktree or only intentional documentation updates.
