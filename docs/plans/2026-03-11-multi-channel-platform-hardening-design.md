# Multi-Channel Platform Hardening P0 Design

**Date:** 2026-03-11

**Goal:** Complete the minimum platform hardening required before adding more IM channels beyond Feishu and WeCom.

## Scope

This P0 only covers four items:

1. A unified connector state and error model
2. A channel capability contract
3. An independent diagnostics view for connectors
4. Adapter-level acknowledgement and replay support

This design explicitly does not include P1/P2 work such as permissions, full audit history, connector lifecycle backoff policies, or vendor automation expansion.

## Problem

The current branch already has a workable multi-channel path, but it still behaves like a productized prototype:

- Connector health is a loose shape with raw strings
- Connector capabilities are implied, not declared
- Diagnostics are mixed into the configuration surface
- `ack` exists in the adapter ABI but is effectively a no-op, and there is no replay entrypoint

That is enough for Feishu and WeCom development, but not enough to support a third channel with confidence.

## Design Principles

### 1. Keep the core business model stable

WorkClaw business code should continue to treat channels as connector-backed message sources. Channel-specific behavior stays inside sidecar adapters and their shim layer.

### 2. Split user-facing diagnostics from technical diagnostics

Default UI should show status, impact, and next action in plain language. Raw connector details remain available in an explicit technical details surface.

### 3. Capabilities must be declarative

A connector must tell the rest of the system what it supports. The frontend and routing surfaces should not infer behavior from connector names.

### 4. Event handling must be replayable

If the user or runtime needs to re-process a message batch, the sidecar must retain a bounded replay window and expose it through a stable API.

## Architecture

### Sidecar

The sidecar becomes the source of truth for connector platform metadata.

New sidecar responsibilities:

- Publish a connector catalog with metadata and capabilities
- Normalize adapter health into a connector issue model
- Retain a bounded in-memory replay buffer per adapter instance
- Track acknowledgements for drained events
- Expose catalog, diagnostics, ack, and replay APIs

The `ChannelAdapter` ABI stays intentionally small. We do not move replay storage into adapters. Adapters remain responsible for protocol I/O. The kernel owns replay and acknowledgement bookkeeping.

### Runtime

Rust runtime becomes the bridge between the sidecar platform contract and the desktop UI.

New runtime responsibilities:

- Fetch connector catalog and diagnostics through generic commands
- Forward ack and replay requests to the sidecar
- Keep channel-neutral command names and data types

Runtime does not reinterpret connector capabilities. It relays them for UI and later routing constraints.

### Frontend

The settings experience is split into three concepts:

- Connector overview and configuration
- Message handling rules
- Connector diagnostics

The diagnostics panel becomes a first-class sibling, not a detail buried in the connector form.

## Data Model

### Connector State

Connector state is normalized into:

- `needs_configuration`
- `connected`
- `degraded`
- `authentication_error`
- `connection_error`
- `stopped`

These map from sidecar runtime health plus the connector issue category. The goal is a stable user-facing model, not a direct mirror of adapter internals.

### Connector Issue

Each connector health payload gains a structured issue object:

- `code`
- `category`
- `user_message`
- `technical_message`
- `retryable`
- `occurred_at`

`last_error` remains available only for compatibility while the new structure rolls through the stack.

### Connector Capabilities

Each connector declares:

- `channel`
- `display_name`
- `capabilities`

P0 capability set:

- `receive_text`
- `send_text`
- `group_route`
- `direct_route`

This list is intentionally small. Mention detection and rich media stay out of scope for P0.

### Replay and Acknowledgement

The kernel tracks drained events in a bounded per-instance replay store keyed by `message_id`.

Each stored replay entry keeps:

- normalized event payload
- first seen time
- last drained time
- ack status

Ack marks the replay entry; replay returns a filtered list of retained events.

## API Design

### Sidecar APIs

New endpoints:

- `POST /api/channels/catalog`
- `POST /api/channels/ack`
- `POST /api/channels/replay-events`
- `POST /api/channels/diagnostics`

Existing endpoints remain:

- `start`
- `stop`
- `health`
- `drain-events`
- `send-message`

Diagnostics payload combines:

- normalized state
- structured issue
- capability list
- queue depth
- reconnect attempts
- last healthy time
- replay window counts

### Runtime Commands

New generic commands:

- `list_channel_connectors`
- `get_channel_connector_diagnostics`
- `ack_channel_events`
- `replay_channel_events`

These commands are channel-neutral and must not encode Feishu or WeCom into the interface.

## UI Design

### Connector Overview

Connector cards show:

- connector display name
- normalized status
- enabled rule count
- recent problem summary

### Diagnostics Panel

Separate diagnostics panel shows:

- current status
- user-facing problem summary
- capability chips
- replay/queue counters
- technical details disclosure

This panel is not the same as configuration. It is read-only and aimed at troubleshooting.

## Error Handling

### Sidecar

- Missing connector instance returns a structured `connection_error`
- Unknown channel returns a 404-style API error
- Replay ignores entries already evicted from the bounded window

### Runtime

- Converts sidecar transport failures into connector diagnostics fetch failures
- Does not synthesize channel-specific strings

### Frontend

- Shows user message by default
- Shows technical message only inside technical details
- Treats absent diagnostics as a fetch error, not as "healthy"

## Testing Strategy

### Sidecar

- Catalog contract test
- Diagnostics contract test
- Ack test with replay entry mutation
- Replay test proving drained events are retained and returned

### Runtime

- Command tests for catalog and diagnostics bridge
- Command tests for ack and replay forwarding

### Frontend

- Diagnostics panel rendering test
- Capability chip rendering test
- Status translation test

## Success Criteria

P0 is complete when:

1. Every connector advertises a declared capability list
2. Every connector health response can be rendered as a normalized state plus structured issue
3. Users can inspect diagnostics in a dedicated UI section
4. Drained events can be acknowledged and replayed through stable generic APIs
