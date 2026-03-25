# WorkClaw Agent Kernel Alignment Design

**Date:** 2026-03-25

**Goal:** Align WorkClaw's general-purpose agent runtime with the production-grade architectural patterns used by OpenClaw, without changing the employee orchestration, IM bridge, or other WorkClaw-specific collaboration layers in this phase.

## Scope

This design covers only the general agent kernel:

- execution preparation
- system prompt assembly
- route candidate preparation
- model failover and retry
- tool lifecycle
- transcript reconstruction and persistence
- compaction and overflow control
- approval and policy gates
- subagent runtime foundations
- runtime event projection

This design explicitly excludes:

- employee group orchestration
- Feishu / IM bridge flows
- group run state machines
- employee collaboration UX

## Current Assessment

WorkClaw already has most of the raw runtime capabilities needed for a strong agent system:

- execution preparation and routing in [packages/runtime-chat-app/src/service.rs](/d:/code/WorkClaw/packages/runtime-chat-app/src/service.rs)
- the ReAct loop in [apps/runtime/src-tauri/src/agent/turn_executor.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/turn_executor.rs)
- route execution and retry in [apps/runtime/src-tauri/src/commands/chat_route_execution.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_route_execution.rs)
- tool registration and prompt wiring in [apps/runtime/src-tauri/src/commands/chat_tool_setup.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_tool_setup.rs)
- transcript reconstruction in [apps/runtime/src-tauri/src/commands/chat_runtime_io/message_reconstruction.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_runtime_io/message_reconstruction.rs)
- structured tool-result handling in [packages/runtime-executor-core/src/lib.rs](/d:/code/WorkClaw/packages/runtime-executor-core/src/lib.rs)
- approval bus and recovery in [apps/runtime/src-tauri/src/approval_bus.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/approval_bus.rs)

The problem is not capability absence. The problem is capability dispersion.

Compared with OpenClaw, WorkClaw currently behaves more like a system that assembles an agent runtime from several adjacent modules, while OpenClaw behaves more like a single coherent runtime kernel with strong operational guardrails.

## Baseline Architectural Differences

### OpenClaw baseline

OpenClaw's core runtime is centered on a unified embedded agent loop with:

- session-serialized execution
- transcript sanitation and repair
- provider-specific protocol adaptation
- explicit overflow recovery and compaction
- tool lifecycle interception
- durable run state
- model and auth failover
- subagent lifecycle management

### WorkClaw current state

WorkClaw already has:

- a functioning ReAct loop
- structured tools
- route candidates and retry classification
- approval gating
- partial progress guards
- transcript reconstruction
- subagent-like task delegation

WorkClaw does not yet have a single runtime module that owns these concerns end-to-end.

## Design Principles

1. One runtime kernel owns one agent run from prepared input to persisted output.
2. Preparation, runtime, integrations, and projections must have separate responsibilities.
3. Transcript handling must become a first-class runtime concern, not an incidental reconstruction helper.
4. Failover must become a runtime policy, not a command-layer behavior.
5. Approval, compaction, and run guards must be middleware-like kernel contracts.
6. Chat entrypoints and UI should consume runtime projections rather than runtime internals.

## Target Architecture

### 1. Agent Preparation Layer

This layer prepares execution but does not run the loop.

Responsibilities:

- normalize session execution context
- resolve route candidates
- assemble system prompt inputs
- compute effective work directory
- narrow tools and permission mode
- resolve memory and workspace skill inputs

Recommended landing zone:

- split [packages/runtime-chat-app/src/service.rs](/d:/code/WorkClaw/packages/runtime-chat-app/src/service.rs) into preparation-focused modules

### 2. Agent Kernel Layer

This becomes the only place that owns run execution.

Responsibilities:

- create and track active runs
- execute attempts
- manage tool lifecycle
- manage transcript state
- recover from context overflow
- apply run guards
- apply approval waits and resume
- execute model failover
- emit normalized runtime events

Recommended landing zone:

- new `apps/runtime/src-tauri/src/agent/runtime/` module tree

### 3. Agent Integration Layer

This layer provides capabilities but does not own loop semantics.

Responsibilities:

- model adapters
- browser sidecar bridge
- MCP integration
- search providers
- approval surfaces
- filesystem / shell / browser tools

### 4. Projection Layer

This layer turns runtime state into stable outputs for other consumers.

Responsibilities:

- session journal updates
- route attempt logs
- exported transcript shape
- UI-ready stream events
- stop-reason projections

## Required Refactoring Direction

### A. Move to a single runtime contract

Create an `AgentRuntime` contract that owns:

- `prepare_run_input`
- `start_run`
- `execute_attempt`
- `handle_tool_calls`
- `recover_overflow`
- `persist_run_output`
- `emit_runtime_events`

### B. Demote current command modules to orchestration

The following modules should become thin orchestration entrypoints:

- [apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs)
- [apps/runtime/src-tauri/src/commands/chat_route_execution.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_route_execution.rs)
- [apps/runtime/src-tauri/src/commands/chat_tool_setup.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_tool_setup.rs)

### C. Promote transcript management to kernel status

Transcript behavior should no longer be split across:

- reconstruction
- stream fallback
- tool-result attachment
- provider-format conversion

Instead, introduce one runtime transcript module that owns:

- history normalization
- tool-call/tool-result pairing
- reconstruction of persisted assistant content
- provider-specific message shaping
- stream fallback repair
- truncation and compaction compatibility

### D. Promote failover to kernel status

Failover should become one subsystem that owns:

- candidate ordering
- retry budgets
- error-kind classification
- backoff timing
- candidate advancement
- interaction with stop reasons and overflow recovery

### E. Upgrade subagent runtime later

Current `task`-based delegation is useful, but still behaves like synchronous nested execution. It should eventually evolve into a runtime-managed child-session system closer to OpenClaw's subagent lifecycle model.

That upgrade is intentionally deferred until after the main runtime kernel is unified.

## Prioritized Refactor Roadmap

### P0

- split `runtime-chat-app` preparation responsibilities
- split `turn_executor` into coherent runtime submodules
- introduce `agent/runtime/` as the new center of gravity
- move route execution and transcript ownership into the runtime kernel

### P1

- add run registry and session serialization
- upgrade compaction from helper behavior to kernel pipeline
- convert tool lifecycle checks into explicit middleware-like stages
- normalize runtime events and stop reasons through one projection path

### P2

- replace synchronous `task` child execution with durable subagent sessions
- further reduce UI ownership of runtime state details

## File-Level Governance Triggers

The following files are already in split-design territory and should not absorb more core logic:

- [packages/runtime-chat-app/src/service.rs](/d:/code/WorkClaw/packages/runtime-chat-app/src/service.rs)
- [apps/runtime/src-tauri/src/agent/turn_executor.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/turn_executor.rs)
- [apps/runtime/src/App.tsx](/d:/code/WorkClaw/apps/runtime/src/App.tsx)
- [apps/runtime/src/components/ChatView.tsx](/d:/code/WorkClaw/apps/runtime/src/components/ChatView.tsx)

## Implementation Constraint

All implementation work for this refactor must begin in a dedicated git worktree. The initial implementation plan must include worktree creation and verification before any code changes.

## Success Criteria

The refactor is successful when:

- one runtime module owns execution semantics end-to-end
- command modules become thin entrypoints
- transcript behavior has one authoritative implementation path
- route retry and failover are no longer split across preparation and command layers
- approval, compaction, and run guards are explicit kernel stages
- the UI consumes stable runtime projections rather than hidden runtime coupling
