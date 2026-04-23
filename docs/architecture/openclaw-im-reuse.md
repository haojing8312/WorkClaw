# OpenClaw IM Reuse Architecture Contract

This document defines the target IM boundary for WorkClaw's OpenClaw-first rearchitecture.

## Core Mapping

- `employee` maps to OpenClaw `agent`
- `employee_id` remains a WorkClaw-facing alias during transition
- New runtime/session code should prefer `agent_id`
- UI and compatibility surfaces may still display `employee`, but they should not define IM semantics

## Ownership Boundaries

### Sidecar Adapter

The sidecar adapter layer is channel I/O only.

It is responsible for:

- connecting to channel providers such as Feishu, WeCom, and DingTalk
- receiving raw channel payloads
- normalizing inbound events into a stable transport shape
- sending outbound messages back to the channel
- capturing optional replay/debug metadata at the adapter edge

It is not responsible for:

- session ownership
- conversation identity
- agent selection
- routing policy beyond channel normalization

### `im_host` Bridge

The `im_host` bridge is transport and dispatch only.

It is responsible for:

- accepting normalized channel events from the sidecar boundary
- translating them into runtime turn requests
- forwarding reply and lifecycle signals back to the desktop runtime
- preserving delivery trace and failure visibility

It is not responsible for:

- deciding long-term session reuse rules
- deriving alternate WorkClaw-specific IM semantics after OpenClaw has already resolved them
- acting as a second session authority

### OpenClaw-Owned Semantics

OpenClaw owns the semantic rules that should be shared across IM channels:

- agent identity
- conversation identity
- session identity
- continue/reset rules
- parent conversation or topic linkage
- compaction behavior
- reply lifecycle meaning

### WorkClaw-Owned Surfaces

WorkClaw owns the product surfaces that sit around OpenClaw semantics:

- desktop shell and runtime hosting
- local UI and diagnostics
- persistence projections for startup and observability
- channel adapter plumbing
- desktop-specific delivery traces

## Keep / Shrink / Replace / Delete

| Module area | Action | Contract |
| --- | --- | --- |
| `apps/runtime/sidecar/src/adapters/*` | Keep, shrink | Channel-specific I/O, normalization, outbound delivery only |
| `apps/runtime/sidecar/src/openclaw-bridge/*` | Keep, shrink | Bridge OpenClaw-style normalized events into the runtime boundary |
| `apps/runtime/src-tauri/src/commands/im_host/*` | Keep, shrink | Transport bridge, runtime dispatch, delivery lifecycle plumbing |
| `apps/runtime/src-tauri/src/commands/employee_agents/*` | Replace / demote | Keep catalog and UI-facing config; remove IM routing authority |
| `apps/runtime/src-tauri/src/im/*` | Replace | New OpenClaw-aligned binding and projection core |
| `apps/runtime/src-tauri/src/db/*` | Replace / extend | Persist new binding tables and projections instead of thread-first session state |
| Legacy thread-first IM lookup paths | Delete | No longer primary source of truth for conversation or session identity |

## Target Storage Model

The target storage model should store projections of OpenClaw semantics, not recreate a separate WorkClaw IM model.

### Primary tables

- `agent_conversation_bindings`
  - binds a channel conversation to an agent/session target
  - records channel, account, agent, session, and parent/topic context

- `channel_delivery_routes`
  - records how a session should deliver replies back to a channel
  - keeps the reply target and channel delivery metadata stable

- `agent_session_projection`
  - stores the local projection used by the desktop runtime and startup flows
  - tracks the latest known session state, status, and delivery timestamps

### Migration intent

- New writes should go to the new binding and projection tables
- Legacy thread-first tables should stop being authoritative
- Transitional readers are acceptable only while the cutover is in progress
- The long-term goal is to remove session routing that depends on `thread_id` plus `employee_id` heuristics
- Existing local databases should be migrated in a backward-compatible way or provided a safe fallback until cutover is complete

## Practical Cutover Rules

1. Normalize channel input at the sidecar boundary.
2. Resolve agent/session/conversation identity using OpenClaw-aligned semantics.
3. Pass only resolved turn and delivery context through `im_host`.
4. Persist projections for local visibility and recovery.
5. Do not rebuild a second WorkClaw-native IM session model in parallel.

## Non-Goals

- Reintroducing thread-first session ownership
- Keeping `employee_agents` as the IM authority
- Making adapters or bridges responsible for policy decisions
- Treating local fallback heuristics as a permanent architecture

## Verification Gate

- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_normalized_im_conversation_identity -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_feishu_conversation_identity -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_route_session_mapping -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_host_windows_regressions -- --test-threads=1 --nocapture`
- `cargo run --manifest-path apps/runtime/src-tauri/Cargo.toml --bin employee_im_heavy_regression`
