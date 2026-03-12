# src-tauri Phase 4B Chat Execution Preparation Design

**Date:** 2026-03-12

## Goal

Move the remaining execution-preparation orchestration out of `apps/runtime/src-tauri/src/commands/chat.rs` and into `packages/runtime-chat-app` without changing user-visible behavior.

## Problem

Phase 4A established `runtime-chat-app` and moved helper logic plus the first preparation layer out of `chat.rs`.

`chat.rs` is still heavier than it should be because it continues to assemble the final execution request by mixing:

- session execution-context normalization
- workdir and imported-MCP guidance aggregation
- routing input assembly
- route candidate and fallback decision packaging
- command-layer request shaping before `AgentExecutor`

This keeps application-layer orchestration trapped in the Tauri command layer.

## Decision

Phase 4B should be a behavior-frozen architecture pass.

It should not:

- change product semantics
- change session behavior
- change employee/team flows
- change task-journey or side-panel UX
- change the executor main loop

It should only continue the dependency-direction cleanup started in Phase 4A.

## Recommended Approach

Introduce a more explicit execution-preparation boundary inside `packages/runtime-chat-app`.

Recommended service:

- `ChatExecutionPreparationService`

Recommended request/result contract:

- `ChatExecutionPreparationRequest`
- `PreparedChatExecution`

`chat.rs` should stop building execution-preparation state directly and should instead:

1. parse command input
2. read runtime state handles
3. call the app-layer execution-preparation service
4. pass the prepared request to the existing executor/runtime wiring

## Why This Cut Is Correct

This is the highest-value next cut because it removes the last large block of application orchestration from the command layer without forcing a risky executor rewrite.

It improves the architecture in the correct order:

- first fix dependency direction
- then consider future product or behavior changes

This avoids mixing refactor risk with UX changes.

## New App-Layer Boundary

`runtime-chat-app` should own:

- session execution-context aggregation
- workdir/imported-MCP execution guidance aggregation
- routing input aggregation
- route candidate and fallback execution decisions
- the final packaging of a prepared execution request

`src-tauri` should continue to own:

- Tauri command entrypoints
- `State` extraction
- SQLx adapters
- event emission
- responder plumbing
- `AgentExecutor` invocation
- persistence writes
- employee/team runtime integration

## Traits

Phase 4B should keep narrow read-oriented traits.

### `ChatSettingsRepository`

Responsibilities:

- read routing settings
- read chat/capability routing policy
- resolve default / usable model ids
- expose imported-MCP/default-workdir related settings required during execution preparation

### `ChatSessionContextRepository`

Responsibilities:

- read session mode
- read team / employee identifiers and related execution metadata
- expose only the session facts needed before execution starts

### `ChatRouteCatalog`

Responsibilities:

- expose provider/model candidate views needed during route preparation
- support capability-based route candidate assembly
- provide the inputs needed to form fallback execution decisions

### `ChatExecutionPolicy`

This can start as internal pure logic rather than a separate trait.

Responsibilities:

- normalize retry/fallback intent
- stabilize execution-preparation conclusions

## First Migration Scope

Phase 4B should move the following orchestration into the app layer:

1. execution context aggregation
   - session mode interpretation
   - employee/team metadata normalization
   - invocation and capability hints normalization

2. execution guidance aggregation
   - default workdir usage
   - imported MCP guidance shaping
   - route-related context hints

3. route decision assembly
   - routing settings input shaping
   - route candidate input preparation
   - fallback execution decision packaging
   - chat-side integration with default/usable model resolution

4. send-message request packaging
   - build a stable `PreparedChatExecution` that the command layer hands to the executor

## Explicit Non-Goals

Phase 4B does not move:

- `AgentExecutor` main loop
- tool confirmation plumbing
- ask-user flow
- event emission
- session/message persistence writes
- task-journey projection
- side-panel ownership logic
- employee/team business-flow rewrites

## Testing Strategy

### `runtime-chat-app`

Most new coverage should live here:

- execution-context aggregation tests
- execution-guidance aggregation tests
- route-decision assembly tests
- prepared-request packaging tests

These tests should use fake repositories and fake route catalogs.

### `src-tauri`

Keep only narrow smoke coverage for:

- adapter wiring
- command/service integration
- one or two focused send-message preparation scenarios

## Acceptance Criteria

Phase 4B is successful when:

- `chat.rs` is materially thinner in orchestration responsibility
- `runtime-chat-app` exposes a stable execution-preparation API
- execution context, guidance, and route packaging no longer mainly live in `chat.rs`
- behavior remains unchanged
- most verification happens in the lightweight crate rather than the heavy Tauri crate
