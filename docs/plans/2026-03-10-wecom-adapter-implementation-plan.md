# OpenClaw WeCom Adapter Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Enterprise WeChat (`wecom`) as the second connector in WorkClaw using the OpenClaw-compatible channel adapter boundary, with a first release limited to text receive/route/reply.

**Architecture:** Reuse the existing sidecar `ChannelAdapter` kernel and channel-neutral runtime pipeline. Implement WeCom as a new sidecar adapter shim plus minimal runtime and frontend connector support, while keeping all upstream OpenClaw-derived details confined to the vendor lane and sidecar boundary.

**Tech Stack:** TypeScript sidecar, Rust/Tauri runtime, React/TypeScript frontend, Vitest, Node test runner, Playwright, Rust integration tests.

---

### Task 1: Add WeCom adapter contract tests in sidecar

**Files:**
- Create: `apps/runtime/sidecar/test/wecom-adapter.test.ts`
- Test: `apps/runtime/sidecar/test/wecom-adapter.test.ts`

**Step 1: Write the failing test**

Add tests that assert:

- a `wecom` adapter can be registered in the channel kernel
- WeCom raw events normalize into `channel = "wecom"` events
- `sendMessage` on the adapter delegates to a WeCom transport shim

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar test -- wecom-adapter`

Expected: FAIL because the WeCom adapter files do not exist yet.

**Step 3: Write minimal implementation**

Create a minimal test seam for WeCom adapter startup and normalization.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar test -- wecom-adapter`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/test/wecom-adapter.test.ts
git commit -m "test(sidecar): add wecom adapter contract coverage"
```

### Task 2: Add WeCom adapter shim in sidecar

**Files:**
- Create: `apps/runtime/sidecar/src/adapters/wecom/index.ts`
- Create: `apps/runtime/sidecar/src/adapters/wecom/normalize.ts`
- Create: `apps/runtime/sidecar/src/adapters/wecom/config.ts`
- Modify: `apps/runtime/sidecar/src/adapters/registry.ts`
- Modify: `apps/runtime/sidecar/src/adapters/types.ts`

**Step 1: Write the failing test**

Use the test from Task 1 and extend it to cover:

- start/stop/health
- event drain shape
- outbound text send

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar test -- wecom-adapter`

Expected: FAIL because there is still no actual adapter implementation.

**Step 3: Write minimal implementation**

Implement the smallest working WeCom adapter shim that satisfies the `ChannelAdapter` contract and can be wired to upstream/vendor code later.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar test -- wecom-adapter`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/adapters/wecom/index.ts apps/runtime/sidecar/src/adapters/wecom/normalize.ts apps/runtime/sidecar/src/adapters/wecom/config.ts apps/runtime/sidecar/src/adapters/registry.ts apps/runtime/sidecar/src/adapters/types.ts
git commit -m "feat(sidecar): add wecom channel adapter shim"
```

### Task 3: Wire WeCom through channel endpoints

**Files:**
- Modify: `apps/runtime/sidecar/src/index.ts`
- Modify: `apps/runtime/sidecar/test/channel-endpoints.test.ts`

**Step 1: Write the failing test**

Add tests that assert:

- `/api/channels/start` can start a `wecom` instance
- `/api/channels/health` returns WeCom adapter state
- `/api/channels/send-message` accepts a WeCom request

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar test -- channel-endpoints`

Expected: FAIL because WeCom is not yet wired into the registry or endpoint paths.

**Step 3: Write minimal implementation**

Register the WeCom adapter and expose it through the existing channel-neutral endpoints.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar test -- channel-endpoints`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/index.ts apps/runtime/sidecar/test/channel-endpoints.test.ts
git commit -m "feat(sidecar): expose wecom through channel endpoints"
```

### Task 4: Add runtime ingress and outbound tests for WeCom

**Files:**
- Create: `apps/runtime/src-tauri/tests/test_wecom_gateway.rs`
- Modify: `apps/runtime/src-tauri/tests/test_openclaw_gateway.rs`

**Step 1: Write the failing test**

Add tests that assert:

- runtime accepts `channel = "wecom"` ingress payloads
- route assembly preserves WeCom channel metadata
- outbound send requests target the correct connector channel

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_wecom_gateway --test test_openclaw_gateway -- --nocapture`

Expected: FAIL because runtime does not yet cover WeCom-specific channel flow.

**Step 3: Write minimal implementation**

Add only the runtime glue required to parse, store, and forward WeCom events and replies through the existing channel-neutral pipeline.

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_wecom_gateway --test test_openclaw_gateway -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/tests/test_wecom_gateway.rs apps/runtime/src-tauri/tests/test_openclaw_gateway.rs
git commit -m "test(runtime): add wecom ingress coverage"
```

### Task 5: Implement runtime WeCom connector flow

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/im_gateway.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_gateway.rs`
- Modify: `apps/runtime/src-tauri/src/commands/im_routing.rs`
- Modify: `apps/runtime/src-tauri/src/im/types.rs`

**Step 1: Write the failing test**

Reuse the tests from Task 4 and add one routing simulation case for `channel = "wecom"`.

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test wecom -- --nocapture`

Expected: FAIL because runtime still lacks WeCom connector support.

**Step 3: Write minimal implementation**

Add additive support for:

- WeCom channel identifiers
- normalized route inputs
- outbound connector dispatch

Do not introduce channel-specific business branches unless unavoidable.

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test wecom -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/im_gateway.rs apps/runtime/src-tauri/src/commands/openclaw_gateway.rs apps/runtime/src-tauri/src/commands/im_routing.rs apps/runtime/src-tauri/src/im/types.rs
git commit -m "feat(runtime): add wecom connector flow"
```

### Task 6: Add frontend WeCom connector schema tests

**Files:**
- Create: `apps/runtime/src/components/connectors/__tests__/connectorSchemas.wecom.test.ts`
- Modify: `apps/runtime/src/components/connectors/connectorSchemas.ts`

**Step 1: Write the failing test**

Add tests that assert:

- connector schema registry contains `wecom`
- WeCom schema exposes the minimum fields for first-phase setup
- schema renders as connector-backed config, not a bespoke page

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- connectorSchemas.wecom`

Expected: FAIL because no WeCom schema exists.

**Step 3: Write minimal implementation**

Add a minimal WeCom connector schema definition.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime test -- connectorSchemas.wecom`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/connectors/__tests__/connectorSchemas.wecom.test.ts apps/runtime/src/components/connectors/connectorSchemas.ts
git commit -m "test(ui): add wecom connector schema coverage"
```

### Task 7: Add frontend WeCom connector UI

**Files:**
- Modify: `apps/runtime/src/components/connectors/ConnectorConfigPanel.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`

**Step 1: Write the failing test**

Add tests that assert:

- settings can render a WeCom connector card
- WeCom connector health/diagnostics render through generic connector UI
- routing/employee views can select a WeCom connector

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- wecom connector`

Expected: FAIL because the UI does not yet render WeCom.

**Step 3: Write minimal implementation**

Expose WeCom through the existing connector-backed panels and reuse generic rendering paths.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime test -- wecom connector`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/connectors/ConnectorConfigPanel.tsx apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/employees/EmployeeHubView.tsx
git commit -m "feat(ui): add wecom connector configuration"
```

### Task 8: Add vendor lane metadata for WeCom

**Files:**
- Modify: `scripts/sync-openclaw-im-core.mjs`
- Modify: `apps/runtime/sidecar/vendor/openclaw-im-core/README.md`
- Modify: `apps/runtime/sidecar/vendor/openclaw-im-core/PATCHES.md`
- Modify: `apps/runtime/sidecar/vendor/openclaw-im-core/UPSTREAM_COMMIT`
- Create: `scripts/check-openclaw-wecom-vendor-lane.test.mjs`

**Step 1: Write the failing test**

Add checks that assert:

- WeCom vendor metadata is present
- sync script knows about WeCom lane
- patch log mentions any local shim patches

**Step 2: Run test to verify it fails**

Run: `node --test scripts/check-openclaw-wecom-vendor-lane.test.mjs`

Expected: FAIL because WeCom vendor metadata does not exist yet.

**Step 3: Write minimal implementation**

Update vendor lane metadata and sync script so future upstream WeCom refreshes are reviewable.

**Step 4: Run test to verify it passes**

Run: `node --test scripts/check-openclaw-wecom-vendor-lane.test.mjs`

Expected: PASS.

**Step 5: Commit**

```bash
git add scripts/sync-openclaw-im-core.mjs apps/runtime/sidecar/vendor/openclaw-im-core/README.md apps/runtime/sidecar/vendor/openclaw-im-core/PATCHES.md apps/runtime/sidecar/vendor/openclaw-im-core/UPSTREAM_COMMIT scripts/check-openclaw-wecom-vendor-lane.test.mjs
git commit -m "chore(sidecar): prepare wecom vendor lane metadata"
```

### Task 9: Add end-to-end regression for WeCom connector shell

**Files:**
- Create: `apps/runtime/e2e/im-connectors.wecom.spec.ts`
- Modify: `apps/runtime/e2e/im-connectors.feishu.spec.ts`

**Step 1: Write the failing test**

Add end-to-end coverage that asserts:

- WeCom connector appears in settings
- generic connector flows remain stable with multiple channels present

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test:e2e`

Expected: FAIL or missing coverage before the new spec is added.

**Step 3: Write minimal implementation**

Patch any UI serialization issues required for the new E2E coverage.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime test:e2e`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/e2e/im-connectors.wecom.spec.ts apps/runtime/e2e/im-connectors.feishu.spec.ts
git commit -m "test(e2e): add wecom connector smoke coverage"
```

### Task 10: Full regression verification

**Files:**
- No new files required

**Step 1: Run sidecar tests**

Run: `pnpm --dir apps/runtime/sidecar test`

Expected: PASS.

**Step 2: Run frontend tests**

Run: `pnpm --dir apps/runtime test`

Expected: PASS.

**Step 3: Run runtime tests**

Run: `cd apps/runtime/src-tauri && cargo test -j 1 -- --nocapture`

Expected: PASS.

**Step 4: Run connector E2E**

Run: `pnpm --dir apps/runtime test:e2e`

Expected: PASS.

**Step 5: Verify clean worktree**

Run: `git status`

Expected: clean worktree or only intentional release notes/docs.
