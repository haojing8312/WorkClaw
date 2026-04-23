# IM Conversation Identity Cutover Design

## Summary

This design defines the smallest safe cutover path for WorkClaw IM session reuse.
The goal is to stop using thread-first reuse as the primary authority and move to
conversation-first binding, while keeping legacy fallback paths available during
transition.

This design intentionally does not change compaction behavior. It only ensures
that compaction, when it happens, occurs inside the correct session boundary.

## Problem

WorkClaw currently has user-visible risk in long-running IM conversations because
session reuse is too coarse in some paths. A channel thread or route key can be
reused across distinct logical conversations, which can make the runtime attach
new messages to the wrong existing session. When that happens, later compaction
and context growth make the failure more visible, but the first bug is often the
binding decision, not the compactor itself.

The immediate requirement is to make IM session ownership follow a stable
conversation identity rather than raw thread heuristics.

## Goal

- Make `conversation_id + agent_id` the primary reuse key for IM session binding.
- Keep current local databases working through backward-compatible migration and
  fallback reads.
- Let Feishu use the new path first, while normalized IM inputs from other
  channels can use the same model when available.
- Remove thread-first routing from the authority path without breaking startup,
  reply delivery, or existing sessions.

## Non-Goals

- Reworking `agent/compactor.rs` or `agent/turn_executor.rs`
- Designing a new compaction boundary protocol in this round
- Removing all legacy tables immediately
- Replacing the employee or agent catalog model
- Redesigning sidecar adapter payload formats beyond what is needed to carry
  normalized conversation metadata

## Current State

The current in-flight changes already introduce the first pieces of the new
model:

- conversation identity helpers under `apps/runtime/src-tauri/src/im/*`
- Feishu and normalized IM event derivation in
  `apps/runtime/src-tauri/src/commands/im_host/inbound_bridge.rs`
- new binding and projection tables in `apps/runtime/src-tauri/src/db/migrations.rs`
- updated tests for IM route/session mapping and conversation identity

This design locks the target contract for finishing that cutover cleanly.

## Design Principles

1. The channel adapter is transport only. It does not own session semantics.
2. `im_host` is a bridge and dispatcher, not a second source of routing truth.
3. Conversation identity must be explicit and stable across retries and restarts.
4. New writes should go to the new model first.
5. Legacy tables remain readable during migration, but they stop being the
   decision authority.
6. The smallest safe path is better than a full rewrite because WorkClaw is in
   active development and local databases must keep working.

## Architecture

### 1. Conversation Identity

Each inbound IM event must resolve to a normalized conversation surface and then
to a stable `conversation_id`.

The normalized shape is conceptually:

```ts
type ImConversationSurface = {
  channel: string;
  accountId: string;
  tenantId?: string;
  peerKind: "direct" | "group";
  peerId: string;
  topicId?: string;
  senderId?: string;
  scope: "peer" | "peer_sender" | "topic" | "topic_sender";
  messageId?: string;
  rawThreadId?: string;
  rawRootId?: string;
};
```

Derived identifiers:

- `conversation_id`
  The concrete logical conversation boundary used for reuse decisions.
- `base_conversation_id`
  The peer-level base conversation used for inheritance and fallback.
- `parent_conversation_candidates`
  Ordered candidate parents used for future inheritance or scoped fallback.
- `conversation_scope`
  The scope used to explain why two messages belong together or not.

Example shapes:

- `feishu:tenant-a:group:chat-1`
- `feishu:tenant-a:group:chat-1:topic:topic-42`
- `wecom:agent-1:group:room-1`

### 2. Binding Authority

The new primary authority is the binding of conversation to agent to session.

That authority lives in:

- `agent_conversation_bindings`

This table answers:

- for this `conversation_id`
- on this `channel/account`
- for this `agent_id`
- which WorkClaw `session_id` owns the dialogue

The key contract is:

- reuse is allowed only when `conversation_id + agent_id` matches
- reuse is not allowed based only on coarse route keys or thread IDs

### 3. Delivery Projection

Reply routing stays separate from session ownership.

That projection lives in:

- `channel_delivery_routes`

This table stores how a given session key sends replies back to the external IM
surface. It is a delivery concern, not a reuse concern. Keeping it separate
prevents session identity from being rebuilt indirectly from reply metadata.

### 4. Transitional Session Projection

The transition keeps:

- `im_conversation_sessions`
- `im_thread_sessions`

`im_conversation_sessions` becomes the compatibility projection for conversation
lookups and migration reads.

`im_thread_sessions` remains only as:

- an input-era legacy projection
- a fallback read source for old local databases
- a migration source for backfill

It is no longer the authoritative long-term reuse model.

## Data Model

### Primary Tables

#### `agent_conversation_bindings`

Purpose:

- primary semantic binding for conversation ownership

Important fields:

- `conversation_id`
- `channel`
- `account_id`
- `agent_id`
- `session_key`
- `session_id`
- `base_conversation_id`
- `parent_conversation_candidates_json`
- `scope`
- `peer_kind`
- `peer_id`
- `topic_id`
- `sender_id`
- `created_at`
- `updated_at`

Primary key:

- `(conversation_id, agent_id)`

#### `channel_delivery_routes`

Purpose:

- reply target projection by session key

Important fields:

- `session_key`
- `channel`
- `account_id`
- `conversation_id`
- `reply_target`
- `updated_at`

#### `im_conversation_sessions`

Purpose:

- compatibility and startup projection for conversation-to-session mapping

Important fields:

- `conversation_id`
- `employee_id`
- `thread_id`
- `session_id`
- `route_session_key`
- `channel`
- `account_id`
- `base_conversation_id`
- `parent_conversation_candidates_json`
- `scope`
- `peer_kind`
- `peer_id`
- `topic_id`
- `sender_id`
- `created_at`
- `updated_at`

### Legacy Table

#### `im_thread_sessions`

This table remains during migration, but with additional fields for conversation
metadata. Its role becomes:

- migration source
- fallback read source
- observability aid during cutover

It must not remain the primary reuse authority.

## Read/Write Rules

### Write Path

For every new normalized inbound IM event:

1. Derive `conversation_id`, `base_conversation_id`,
   `parent_conversation_candidates`, and `conversation_scope`.
2. Resolve the target agent.
3. Write or update `agent_conversation_bindings`.
4. Write or update `channel_delivery_routes`.
5. Write or update `im_conversation_sessions`.
6. Optionally mirror enough metadata into `im_thread_sessions` for transition and
   diagnostics.

### Read Path

Session lookup order:

1. `agent_conversation_bindings`
2. `im_conversation_sessions`
3. `im_thread_sessions` fallback only when the above are absent

This order ensures new semantics win immediately without breaking older local
data.

### Reuse Rules

Allowed:

- same `conversation_id`
- same target `agent_id`
- same channel/account boundary

Not allowed:

- different conversation IDs inside the same thread family
- different agents inside the same conversation
- reuse based only on `route_session_key`
- reuse based only on `thread_id`

## Module Responsibilities

### Sidecar

Files under `apps/runtime/sidecar/src/adapters/*` remain responsible for:

- receiving provider payloads
- normalizing channel events
- sending outbound messages

They must not become session authorities.

### `im_host`

Files under `apps/runtime/src-tauri/src/commands/im_host/*` remain responsible
for:

- accepting normalized events
- deriving or carrying conversation metadata
- dispatching runtime turn requests
- emitting delivery lifecycle signals

They must not recreate a second WorkClaw-native session policy.

### `im/*`

Files under `apps/runtime/src-tauri/src/im/*` become the semantic core for:

- conversation identity derivation
- agent/session binding lookup
- delivery route projection
- future startup/session runtime projections

### `employee_agents/*`

Files under `apps/runtime/src-tauri/src/commands/employee_agents/*` continue to
own:

- agent catalog data
- compatibility entrypoints
- agent selection support

They should not remain the long-term home of IM routing authority.

## Migration Strategy

This cutover should be backward compatible and progressive.

### Step 1. Schema Extension

- create `agent_conversation_bindings`
- create `channel_delivery_routes`
- create `im_conversation_sessions`
- extend `im_thread_sessions` with conversation metadata fields
- add indexes for conversation and channel/account lookups

### Step 2. Backfill

At migration time:

- copy existing `im_thread_sessions` data into `im_conversation_sessions`
- use `conversation_id` when already present
- otherwise fall back to `thread_id`

This preserves existing local session continuity while allowing new writes to
upgrade behavior immediately.

### Step 3. Writer Cutover

All new IM inbound processing writes to the new tables first.

### Step 4. Reader Cutover

All reuse lookups prefer new binding tables first and only fall back to thread
projections when necessary.

### Step 5. Legacy Demotion

After validation is complete:

- keep `im_thread_sessions` readable
- stop treating it as a routing authority
- leave deletion or full retirement for a later maintenance task

## Error Handling

If conversation metadata cannot be fully derived:

- the event should still get a deterministic fallback `conversation_id`
- fallback should be stable and explicit, usually based on channel/account/thread
- lookup should still avoid route-key-only reuse

If a new-table write succeeds but a legacy mirror write fails:

- the runtime should keep the new-table write authoritative
- the failure should be logged as a projection degradation, not as a reuse reason

If only legacy schema exists in a user database:

- startup migration must add the needed columns and tables
- lookup must remain functional after migration without requiring manual cleanup

## Testing Strategy

### Unit Tests

- conversation ID derivation for peer conversations
- conversation ID derivation for topic conversations
- parent conversation candidate ordering
- agent binding lookup precedence
- fallback behavior when conversation metadata is absent

### Integration Tests

- different threads do not reuse the same session
- same thread with different agents does not reuse the same session
- topic-level conversation stays isolated from peer-level conversation
- new-table lookup wins over legacy thread mapping
- migrated legacy databases still resolve existing sessions
- reply routes still point to the correct target after binding updates

### Regression Coverage

At least one regression test must use a legacy schema fixture and prove that:

- an older database can be opened
- migrations complete successfully
- conversation-based lookup works after migration

## Rollout Plan

The smallest safe rollout is:

1. Finish the conversation identity and binding cutover for Feishu and normalized
   IM paths.
2. Keep legacy fallback reads during validation.
3. Validate with IM routing tests and Windows regression coverage.
4. Only after binding stability is confirmed, open a separate design for
   compaction behavior.

## Risks

### Main Risk

The largest risk is partial cutover, where some paths still make reuse decisions
from thread-first state while others use conversation identity. That would make
behavior inconsistent and hard to debug.

### Secondary Risks

- local databases with partially migrated state
- delivery projections lagging behind binding updates
- channels that normalize too little metadata and collapse into peer-level reuse
- compatibility code living too long and becoming a second permanent architecture

## Success Criteria

The design is successful when:

- distinct IM conversations do not attach to the same session by mistake
- a conversation only reuses a session when the same agent owns that conversation
- local migrated databases continue to load safely
- reply delivery still goes back to the correct channel target
- no code path uses route-key-only reuse as the primary authority

## Follow-Up Work Explicitly Deferred

This design defers a separate spec for:

- compaction boundary behavior
- summary plus tail-message retention model
- transcript retention semantics after compaction
- alignment with OpenClaw's compact-boundary design
