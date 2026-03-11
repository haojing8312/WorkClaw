# OpenClaw WeCom Adapter Design

## Goal

Add Enterprise WeChat (`wecom`) as the second IM connector in WorkClaw through the existing OpenClaw-compatible adapter boundary.

The target is not to hand-build a WorkClaw-specific WeCom protocol stack. The target is:

- reuse OpenClaw-supported WeCom channel capability inside the sidecar boundary
- keep WorkClaw runtime, employee routing, session ownership, memory, and UI channel-neutral
- prove that future OpenClaw-supported IM channels can be added with small boundary-only changes

## Scope

First phase only delivers the minimum closed loop:

- receive text messages from WeCom
- normalize them into `NormalizedImEvent`
- resolve employee route through the existing OpenClaw route engine
- create or reuse WorkClaw sessions
- send text replies back through the adapter

## Non-Goals

- no WeCom-specific contact management
- no media/file/card messages
- no bespoke WorkClaw-only WeCom runtime
- no direct dependency from Rust or frontend onto upstream OpenClaw WeCom types

## Current State

WorkClaw already has:

- a stable sidecar `ChannelAdapter` kernel
- Feishu wrapped as the first connector
- channel-neutral HTTP endpoints under `/api/channels/*`
- channel-neutral IM ingress and routing state in runtime
- vendor lane discipline for OpenClaw-derived code

Relevant areas:

- `apps/runtime/sidecar/src/adapters/*`
- `apps/runtime/sidecar/src/openclaw-bridge/*`
- `apps/runtime/src-tauri/src/commands/im_gateway.rs`
- `apps/runtime/src-tauri/src/commands/openclaw_gateway.rs`
- `apps/runtime/src-tauri/src/commands/im_routing.rs`
- `apps/runtime/src/components/connectors/*`

## Chosen Approach

Use an OpenClaw-driven WeCom adapter shim inside the sidecar.

WorkClaw will:

- vendor or wrap the upstream WeCom integration only inside `apps/runtime/sidecar/vendor/openclaw-im-core/` or a sibling vendor lane
- expose it through WorkClaw's stable `ChannelAdapter` ABI
- keep runtime and frontend consuming only `channel = "wecom"` plus normalized connector metadata

This is preferred over a custom WeCom implementation because it preserves the upstream-first upgrade path you asked for.

## Architecture

### 1. Vendor Lane

Add a WeCom-focused vendor subtree and sync metadata:

- `apps/runtime/sidecar/vendor/openclaw-im-core/`
- `apps/runtime/sidecar/vendor/openclaw-im-core/wecom/`
- `scripts/sync-openclaw-im-core.mjs`

Rules:

- only sidecar code can import vendor code
- every patch must be logged in `PATCHES.md`
- upstream revision must be pinned in `UPSTREAM_COMMIT`

### 2. Sidecar Adapter Shim

Add:

- `apps/runtime/sidecar/src/adapters/wecom/index.ts`
- `apps/runtime/sidecar/src/adapters/wecom/normalize.ts`
- `apps/runtime/sidecar/src/adapters/wecom/config.ts`

Responsibilities:

- map OpenClaw WeCom startup config into WorkClaw adapter config
- convert upstream WeCom events into `NormalizedImEvent`
- implement `start`, `stop`, `health`, `drainEvents`, `sendMessage`, `ack`
- keep WeCom-specific retry or transport details inside the shim

### 3. Runtime Integration

Rust runtime should only see:

- `channel = "wecom"`
- normalized `account_id`, `workspace_id`, `thread_id`
- generic `routing_context`

Business modules remain unchanged in shape:

- employee matching
- route binding persistence
- session creation
- memory isolation
- response orchestration

### 4. Frontend Connector UX

Frontend should render WeCom as another connector entry, not a new standalone product area.

Add:

- WeCom connector schema
- WeCom connector card in settings
- WeCom-compatible route simulation inputs
- connector diagnostics for WeCom instance health and last error

## Data Model

### Normalized Event

WeCom events normalize into:

- `channel: "wecom"`
- `workspace_id`
- `account_id`
- `thread_id`
- `message_id`
- `sender_id`
- `sender_name`
- `text`
- `mentions`
- `raw_event_type`
- `occurred_at`
- `reply_target`
- `routing_context`
- `raw_payload`

### Routing Context

WeCom-specific raw fields should collapse into generic routing fields:

- `peer.kind`: `direct` or `group`
- `peer.id`
- `parent_peer`
- `org_id`
- `department_ids`
- `identity_links`

### Connector Config

WeCom connector config should fit the existing connector container:

- `connector_id`
- `channel`
- `display_name`
- `enabled`
- `auth_config`
- `runtime_config`
- `capabilities`
- `health_state`

Only WeCom-specific secrets stay inside `auth_config`.

## Runtime Flow

1. User configures a WeCom connector in settings.
2. Runtime persists connector config and asks sidecar to start adapter instance.
3. Sidecar WeCom shim starts the upstream adapter and buffers normalized events.
4. Runtime drains events and resolves route through existing OpenClaw route engine.
5. Runtime creates or reuses a session and runs the assigned employee.
6. Outbound text is sent back through `sendMessage` on the same connector instance.

## Error Handling

### Adapter Errors

- auth failure
- upstream transport failure
- send-message failure
- retry exhaustion

These update connector health and diagnostics, but must not corrupt runtime state.

### Routing Errors

- no route matched
- multiple routes matched unexpectedly
- missing required identity fields

These should be surfaced as routing diagnostics, not adapter crashes.

### Runtime Errors

- employee execution failure
- tool failure
- session persistence failure

These should follow existing WorkClaw runtime rules and still leave the connector healthy unless the transport itself failed.

## Testing Strategy

### Sidecar

- WeCom adapter lifecycle tests
- WeCom event normalization tests
- channel endpoint compatibility tests
- health and diagnostics tests

### Runtime

- `channel = "wecom"` ingress parsing tests
- route binding and simulation tests
- session creation and reply dispatch tests

### Frontend

- connector schema rendering tests
- WeCom connector settings flow tests
- route simulation form tests

### End-to-End

- minimum WeCom connector smoke test with mocked sidecar or mocked upstream adapter

## Rollout Plan

### Phase 1

- add vendor lane metadata for WeCom
- add sidecar WeCom adapter shim
- add sidecar unit tests

### Phase 2

- add runtime WeCom ingress and send path
- add route simulation coverage
- keep all Feishu regressions green

### Phase 3

- add frontend WeCom connector configuration
- add diagnostics
- add end-to-end connector coverage

## Risks

### Risk 1: Upstream WeCom API surface is unstable

Mitigation:

- keep all upstream imports behind the sidecar shim
- pin upstream commit and patch set explicitly

### Risk 2: WeCom needs fields not present in current normalized routing context

Mitigation:

- extend `routing_context` additively
- never special-case WeCom in business-layer APIs if a generic field can represent the same thing

### Risk 3: Frontend drifts back to channel-specific UI

Mitigation:

- add WeCom through connector schema registry only
- avoid creating `WecomSettingsPage`-style standalone screens

## Recommendation

Implement WeCom as a strict second connector through the existing adapter boundary, keep the first release to text-message closed loop only, and treat every WeCom-specific concern as sidecar-boundary detail unless it is truly a cross-channel concept.
