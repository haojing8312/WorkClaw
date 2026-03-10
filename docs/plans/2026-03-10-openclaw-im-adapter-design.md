# OpenClaw IM Adapter Boundary Design

## Goal

Build a stable IM adapter boundary in WorkClaw so the project can adopt additional IM channels supported by OpenClaw with minimal business-layer changes.

The target is not to embed all of OpenClaw. The target is:

- OpenClaw provides channel protocol and routing-related capabilities
- WorkClaw owns employee model, session ownership, long-term memory, team runtime, UI, and local product behavior
- Upstream change is absorbed inside a sidecar adapter boundary instead of leaking into Rust business code and frontend code

## Current State

WorkClaw currently vendors only an OpenClaw routing subset in sidecar:

- `apps/runtime/sidecar/vendor/openclaw-core/`
- `apps/runtime/sidecar/src/openclaw-bridge/route-engine.ts`

Current IM integration is Feishu-centric:

- Sidecar Feishu implementations:
  - `apps/runtime/sidecar/src/feishu.ts`
  - `apps/runtime/sidecar/src/feishu_ws.ts`
- Rust ingress and routing bridge:
  - `apps/runtime/src-tauri/src/commands/im_gateway.rs`
  - `apps/runtime/src-tauri/src/commands/openclaw_gateway.rs`
  - `apps/runtime/src-tauri/src/commands/im_routing.rs`

This works for Feishu, but the boundary is not yet stable enough for elegant multi-channel upgrades.

## Non-Goals

- Do not run a full external OpenClaw gateway process
- Do not rewrite employee/team/memory runtime around OpenClaw internals
- Do not let business logic depend directly on upstream OpenClaw types
- Do not add multiple new IM channels in the first refactor phase

## Chosen Approach

Use a sidecar plugin-style adapter architecture.

WorkClaw defines a stable channel adapter ABI. Each channel implementation plugs into the sidecar through that ABI. OpenClaw-derived code is vendored or wrapped only inside the sidecar adapter layer.

This gives:

- stable WorkClaw business interfaces
- isolated upstream volatility
- channel-by-channel rollout
- lower upgrade cost when OpenClaw adds new IM support

## Architecture

The target architecture has four layers.

### 1. Vendor/Core Layer

Contains vendored or wrapped OpenClaw channel logic that WorkClaw wants to reuse.

Rules:

- only sidecar can import from this layer
- Rust and frontend must never import upstream-specific types
- each vendor subtree must have explicit sync script, upstream commit record, and local patch log

Expected structure:

- `apps/runtime/sidecar/vendor/openclaw-core/`
- `apps/runtime/sidecar/vendor/openclaw-im-core/` later if needed

### 2. Channel Adapter Layer

Each channel is implemented as an adapter.

Expected structure:

- `apps/runtime/sidecar/src/adapters/types.ts`
- `apps/runtime/sidecar/src/adapters/registry.ts`
- `apps/runtime/sidecar/src/adapters/kernel.ts`
- `apps/runtime/sidecar/src/adapters/feishu/*`
- `apps/runtime/sidecar/src/adapters/slack/*`
- `apps/runtime/sidecar/src/adapters/discord/*`

Responsibilities:

- auth/bootstrap
- websocket or webhook lifecycle
- reconnect and health tracking
- raw payload normalization
- send message
- ack/retry
- channel-specific identity mapping

### 3. WorkClaw Channel Kernel

This is the stable sidecar-facing runtime owned by WorkClaw.

Responsibilities:

- adapter registration and lifecycle
- adapter config loading
- health state aggregation
- event buffering and draining
- uniform send-message pipeline
- route-resolution pre-processing
- observability and diagnostics

The kernel is the only thing Rust commands talk to.

### 4. Business Layer

Business modules remain WorkClaw-owned:

- employee identity and employee groups
- IM routing persistence
- session ownership
- long-term memory isolation
- group runtime and event stream
- desktop UI and settings UX

These modules consume normalized channel events and normalized route inputs only.

## Stable Data Model

### NormalizedImEvent

WorkClaw should standardize all channel ingress into a single event shape.

Required fields:

- `channel`
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

Notes:

- `raw_payload` is retained for diagnostics and future compatibility
- `thread_id` is WorkClaw's canonical conversation container id, regardless of source channel
- `mentions` must support both user mentions and role mentions

### RoutingContext

Route resolution should depend on a channel-neutral structure:

- `peer.kind`
- `peer.id`
- `parent_peer`
- `guild_id`
- `team_id`
- `member_role_ids`
- `identity_links`

This structure is intentionally close to the current OpenClaw route engine inputs so that existing route reuse remains cheap.

### SendMessageRequest

Outbound messaging should also be normalized:

- `channel`
- `thread_id`
- `reply_target`
- `text`
- `format`
- `attachments`
- `mentions`
- `idempotency_key`

### AdapterHealth

- `adapter_name`
- `instance_id`
- `state`
- `last_ok_at`
- `last_error`
- `reconnect_attempts`
- `queue_depth`

## Stable Sidecar ABI

The sidecar kernel should expose a stable `ChannelAdapter` interface:

```ts
export interface ChannelAdapter {
  start(config: AdapterConfig): Promise<AdapterHandle>;
  stop(instanceId: string): Promise<void>;
  health(instanceId: string): Promise<AdapterHealth>;
  drainEvents(instanceId: string, limit: number): Promise<NormalizedImEvent[]>;
  sendMessage(instanceId: string, req: SendMessageRequest): Promise<SendMessageResult>;
  ack(instanceId: string, req: AckRequest): Promise<void>;
}
```

Rules:

- Rust does not call adapter implementations directly
- Rust only calls kernel endpoints backed by this ABI
- adapter-specific config is validated before startup and stored as opaque typed config

## Rust Boundary Changes

Rust should stop treating Feishu as the built-in special case.

### Current pain points

- `process_im_event` stores source as `"feishu"`
- `openclaw_gateway.rs` builds Feishu-biased route payload
- employee settings and commands are channel-specific in naming and behavior

### Target shape

- `ImEvent` becomes channel-neutral or is replaced by `UnifiedImEvent`
- `im_routing_bindings` remains the business persistence model, but accepts generalized channel routing fields
- `handle_openclaw_event` becomes a generic normalized ingress path
- compatibility aliases remain for legacy Feishu commands, but internally forward into the new path

### Compatibility rule

Keep these compatibility paths during migration:

- `handle_feishu_callback`
- existing Feishu sidecar endpoints
- existing frontend Feishu setup flow

Internally, all of them should forward to the new channel kernel and normalized ingress pipeline.

## Frontend Boundary Changes

The frontend should move from "Feishu config" to "channel connectors".

### Target UI model

- settings page shows a connector list
- employee page binds employees to one or more channel connectors
- Feishu remains the first connector
- each connector renders its own config schema-driven form

### Why

This avoids repeating page architecture each time a new IM channel is added.

## Upgrade Strategy

Use a four-phase migration.

### Phase 1. Freeze interfaces without changing behavior

- add `NormalizedImEvent`
- add adapter registry and kernel
- wrap current Feishu implementation behind `ChannelAdapter`
- keep public behavior unchanged

### Phase 2. Make Rust consume normalized channel models

- remove Feishu assumptions from ingress and routing bridge
- generalize persistence and command interfaces
- keep compatibility commands as forwarding aliases

### Phase 3. Add vendor discipline for OpenClaw channel adapters

For each adopted upstream channel adapter:

- maintain sync script
- maintain `UPSTREAM_COMMIT`
- maintain `PATCHES.md`
- keep local patches minimal
- require regression tests per adapter

### Phase 4. Make frontend connector-driven

- replace fixed Feishu page sections with connector abstractions
- keep Feishu card as adapter `feishu`
- add connector health and diagnostics UI

## Testing Strategy

### Sidecar tests

- adapter lifecycle tests
- reconnection behavior tests
- event normalization tests
- send-message contract tests
- route-input contract tests

### Rust tests

- normalized ingress parsing tests
- route binding persistence tests
- compatibility alias tests
- multi-channel session ownership tests
- employee memory isolation tests

### End-to-end tests

- Feishu remains green through the adapter boundary
- route simulation still returns `matched_by`, `session_key`, `agent_id`
- channel disconnect and reconnect state becomes visible in UI

## Operational Rules

- no business module may import vendored OpenClaw types directly
- all OpenClaw-derived code must stay inside sidecar adapter boundary
- each adapter upgrade must be reviewable via explicit upstream commit pinning
- avoid local patches unless required for ABI fit or dependency trimming

## Risks

### Risk 1. Feishu-specific assumptions remain in Rust

Impact:

- later channel additions still require core refactors

Mitigation:

- normalize ingress model before adding the second channel

### Risk 2. Adapter ABI grows unstable

Impact:

- each new channel causes kernel churn

Mitigation:

- keep ABI small and focused on lifecycle, health, event drain, send, ack

### Risk 3. Over-coupling to upstream internals

Impact:

- OpenClaw upgrades become expensive

Mitigation:

- never expose upstream types outside the sidecar boundary

## Recommendation

Proceed with:

1. interface freeze
2. Feishu adapter wrapping
3. Rust normalization
4. connector-driven frontend
5. only then add the second channel

This is the lowest-risk route to achieving elegant downstream support for OpenClaw-supported IM channels while preserving WorkClaw's own runtime ownership.
