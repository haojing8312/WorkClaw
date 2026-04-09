# WorkClaw Task Engine Design

**Date:** 2026-04-09

## Goal

Upgrade WorkClaw from a unified `Session Spine` into a unified `Task Engine` that owns task lifecycle, subtask delegation, continuation, permissions, and persistence through one runtime backbone, aligned with the architectural strengths seen in `close-code`.

## Why This Design Exists

WorkClaw has already completed an important architectural step:

- [session_engine.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs)
- [execution_plan.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs)
- [turn_preparation.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs)
- [turn_state.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs)
- [session_profile.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/kernel/session_profile.rs)
- [session_journal.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/session_journal.rs)

That work unified desktop-local chat, hidden child sessions, and employee step sessions well enough to establish one **session-level** execution backbone.

But the original product goal was not merely "a better chat runtime." The goal was:

**move WorkClaw toward top-tier harness-agent product capability.**

That requires the next architectural jump:

**Session Spine -> Task Engine**

The missing step is not another round of session refactoring. The missing step is a higher-level runtime model where the primary unit is a `Task`, not a `Session` or a `Turn`.

## Key Observation From `close-code`

The architectural lesson from `close-code` is not "it has multiple windows" or "it supports subagents."

It already supports:

- query lifecycle ownership in [QueryEngine.ts](/e:/code/yzpd/close-code/src/QueryEngine.ts) and [query.ts](/e:/code/yzpd/close-code/src/query.ts)
- durable session state in [state.ts](/e:/code/yzpd/close-code/src/bootstrap/state.ts)
- session persistence and recovery in [sessionStorage.ts](/e:/code/yzpd/close-code/src/utils/sessionStorage.ts) and [sessionRestore.ts](/e:/code/yzpd/close-code/src/utils/sessionRestore.ts)
- unified tool orchestration in [toolOrchestration.ts](/e:/code/yzpd/close-code/src/services/tools/toolOrchestration.ts)
- in-process and pane-backed teammate execution in [inProcessRunner.ts](/e:/code/yzpd/close-code/src/utils/swarm/inProcessRunner.ts) and [PaneBackendExecutor.ts](/e:/code/yzpd/close-code/src/utils/swarm/backends/PaneBackendExecutor.ts)
- explicit task management in [TaskCreateTool.ts](/e:/code/yzpd/close-code/src/tools/TaskCreateTool/TaskCreateTool.ts)
- agent orchestration in [runAgent.ts](/e:/code/yzpd/close-code/src/tools/AgentTool/runAgent.ts)

The real advantage is this:

1. There is one dominant runtime loop.
2. There is one dominant mutable state spine.
3. Tool execution, permission handling, compaction, session restore, and subagent delegation are all subordinate to that loop.
4. Surface diversity exists, but the runtime does not fragment its ownership model.

WorkClaw has solved the lower layer of this problem with `Session Spine`.

`Task Engine` is the next layer.

## Current WorkClaw Baseline

### What is already strong

WorkClaw now has explicit contracts for:

- execution context
- turn context
- route plans and lane execution
- continuation preference and retry clamp
- compaction boundary persistence
- cross-surface turn-state projection
- capability snapshot and prompt assembly

These are already good ingredients for a harness-class runtime.

### What is still missing

The strongest remaining architectural gap is that WorkClaw still models the world primarily in terms of:

- session
- turn
- lane
- runner

That is enough to run local chat well.

It is not enough to cleanly own:

- delegated tasks
- child-agent tasks
- employee-executed tasks
- review or verification tasks
- future teammate / swarm tasks
- task-level continuation after interruptions, permission denials, or compaction

Right now those concepts exist, but are still expressed mostly as:

- surface-specific adapters
- session-specialized continuations
- command-level orchestration
- journal projections after the fact

## Problem Statement

WorkClaw's current architecture still answers the question:

**"How does this session turn run?"**

But a top-tier harness agent must answer a higher-level question:

**"How does this task live, delegate, pause, recover, continue, and finish?"**

As long as WorkClaw's runtime is session-first, future capabilities like generalized subagents, multi-agent coordination, verification workers, and durable long-running task continuation will continue to grow as special cases.

## Design Objective

Introduce a new top-level runtime core:

**`TaskEngine`**

This engine becomes the primary lifecycle owner for:

- task preparation
- task execution
- route and capability planning
- permission and approval control
- continuation and recovery
- delegation and child-task creation
- task-level persistence and observability

`Session Spine` should not be discarded immediately. It should be treated as the first successful runtime substrate beneath the new task layer.

## Non-Goals

This design intentionally does **not** do these things in the first stage:

- rewrite `AgentExecutor`
- replace every tool implementation
- implement swarm panes or terminal multiplexing
- redesign the frontend to mirror `close-code`
- delete all session-level code at once
- unify all IM and external-channel surfaces immediately

The first priority is architectural ownership, not UI parity.

## Core Concepts

### 1. `TaskEngine`

The new top-level runtime loop.

Responsibilities:

- accept a task request
- build initial task state
- resolve surface and execution profile
- plan route and capabilities
- run execution through a backend
- evaluate transitions after execution or interruption
- persist task projection and recovery state

It is conceptually closer to the combination of:

- [QueryEngine.ts](/e:/code/yzpd/close-code/src/QueryEngine.ts)
- [query.ts](/e:/code/yzpd/close-code/src/query.ts)

than to the current WorkClaw `session_engine`.

### 2. `TaskState`

The single task-level fact source.

Minimum first-version fields:

- `task_id`
- `parent_task_id`
- `root_task_id`
- `task_kind`
- `surface_kind`
- `session_id`
- `messages`
- `selected_route`
- `selected_runner`
- `selected_skill`
- `allowed_tools`
- `effective_tool_plan`
- `continuation_policy`
- `compaction_boundary`
- `partial_output`
- `stop_reason`
- `retry_budget`
- `reconstructed_history_len`
- `child_task_refs`

This is the critical shift:

WorkClaw should stop treating task continuity as a derived effect of turn continuity and start treating it as first-class runtime state.

### 3. `TaskKind`

The primary runtime classification.

Suggested first version:

- `PrimaryUserTask`
- `DelegatedSkillTask`
- `SubAgentTask`
- `EmployeeStepTask`
- `RecoveryTask`

This is higher-level than the existing `SessionSurfaceKind`.

### 4. `TaskSurfaceKind`

The execution surface or carrier.

Suggested first version:

- `LocalChatSurface`
- `HiddenChildSurface`
- `EmployeeStepSurface`

The key rule is:

**task kind expresses what the task is; surface kind expresses where it runs.**

### 5. `TaskTransition`

The unified post-execution and post-interruption state transition model.

Suggested first version:

- `ContinueWithRoute`
- `ContinueWithToolCalls`
- `ContinueAfterCompaction`
- `ContinueAfterApproval`
- `DelegateToChildTask`
- `DelegateToEmployeeTask`
- `StopCompleted`
- `StopFailed`
- `StopCancelled`
- `StopNeedsUserInput`

This transition layer should replace scattered continuation logic, fallback logic, and special-case stop handling.

### 6. `TaskBackend`

The execution backend abstraction.

Suggested first version:

- `InteractiveChatBackend`
- `HiddenChildBackend`
- `EmployeeStepBackend`

Future backends:

- `TeammateBackend`
- `ReviewBackend`
- `VerificationBackend`

This preserves WorkClaw's existing reality of multiple execution surfaces without letting those surfaces become separate runtime owners.

### 7. `TaskJournalProjection`

The task-level persistence and replay boundary.

This generalizes current session-journal behavior into a projection that can support:

- recovery
- continuity
- debug
- export
- eval
- cross-surface analysis

## Target Runtime Flow

The target top-level runtime flow should become:

`TaskEngine::run_task()`
-> `prepare_task()`
-> `plan_route()`
-> `build_capabilities()`
-> `resolve_permissions()`
-> `execute_backend()`
-> `resolve_transition()`
-> `commit_task_projection()`

This is the key ownership change.

Today WorkClaw still tends to:

- prepare a surface-specific execution context
- execute a route
- project journal state after the fact

The new model should instead:

- enter the task engine first
- build task state first
- push everything else behind the task engine

## Mapping From Existing WorkClaw Modules

### Modules to keep but demote

- [session_engine.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs)
  - becomes a transitional bridge or an internal execution helper
- [execution_plan.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs)
  - should be split into task-state-oriented contracts
- [turn_preparation.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs)
  - should evolve into `task_preparation`
- [turn_state.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs)
  - should become one projection layer of `TaskState`, not the top state unit

### Modules to turn into backend adapters

- [child_session_runtime.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs)
- [group_run_execution_service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/group_run_execution_service.rs)

These should stop owning mini-runtimes and instead request task execution from the shared engine.

### Modules that should become engine-owned stages

- [tool_setup.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs)
- [effective_tool_set.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/effective_tool_set.rs)
- [approval_gate.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/approval_gate.rs)
- [tool_dispatch.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs)
- [session_journal.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/session_journal.rs)

These are already good primitives. They need to be re-owned by the task engine rather than invoked from loosely coupled surface flows.

## Proposed New Module Layout

Suggested new runtime modules:

- `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`
- `apps/runtime/src-tauri/src/agent/runtime/task_state.rs`
- `apps/runtime/src-tauri/src/agent/runtime/task_transition.rs`
- `apps/runtime/src-tauri/src/agent/runtime/task_backend.rs`
- `apps/runtime/src-tauri/src/agent/runtime/task_preparation.rs`
- `apps/runtime/src-tauri/src/agent/runtime/task_journal.rs`

The first implementation should **not** delete the existing kernel tree. It should sit on top of it and progressively absorb it.

## Migration Strategy

### Phase 1: Top-Level Task Engine Skeleton

Introduce the new engine and state contracts, but keep local chat execution delegated to the current session spine.

Goal:

- prove that the correct top-level runtime boundary exists
- avoid destabilizing already working execution behavior

### Phase 2: TaskState Adoption

Translate `ExecutionContext`, `TurnContext`, and `TurnStateSnapshot` into task-state-backed execution.

Goal:

- make continuation and projection task-native instead of session-native

### Phase 3: Child / Employee Backend Migration

Migrate hidden child sessions and employee step sessions into task backends.

Goal:

- remove remaining surface-owned mini-runtimes

### Phase 4: TaskTransition Ownership

Move continuation, stop, recovery, approval, and delegation into one transition layer.

Goal:

- stop scattering transition semantics across route runners and surface adapters

### Phase 5: Generalized Multi-Agent Expansion

Once the task engine exists, add:

- teammate backends
- verification workers
- future swarm orchestration

without inventing another top-level runtime owner.

## Major Risks

### 1. Dual-runtime overlap during migration

There will be a temporary overlap between:

- the old session-spine contracts
- the new task-engine contracts

That is acceptable if the top-level owner is clear.

### 2. Overreach in the first phase

If Phase 1 tries to absorb child sessions, employee steps, continuation, and journal all at once, the migration will become too risky.

The first phase must stay narrow.

### 3. Regressing local chat

Local chat is the most validated current surface. The migration must keep its execution path stable while the top-level contracts move.

## Recommended First-Phase Scope

The first real implementation phase should do only this:

- create `task_engine.rs`
- create `task_state.rs`
- introduce `TaskKind::PrimaryUserTask`
- introduce `TaskSurfaceKind::LocalChatSurface`
- route local chat through `TaskEngine -> existing session spine`
- store task identity in journal projection
- add focused tests proving the new top-level runtime boundary

This is intentionally narrow.

The purpose of Phase 1 is not to "finish Task Engine."

It is to make sure WorkClaw now has the correct top-level runtime owner.

## Final Recommendation

WorkClaw should adopt **Scheme C** only if it accepts that this is a real runtime re-foundation, not another refactor of prompt routing or session plumbing.

That said, this direction is still the most correct long-term architecture if the product goal remains:

**become a top-tier harness agent with durable task execution, delegation, and continuity.**

The implementation strategy should therefore be:

**introduce the new correct top-level engine first, then gradually absorb the old spine into it.**
