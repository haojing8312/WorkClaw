# WorkClaw Session Spine Design

**Date:** 2026-04-08

## Goal

Upgrade WorkClaw's current desktop local chat kernel into a unified `Session Spine` so local chat, hidden child sessions, and employee step sessions all run through one session-level execution backbone with shared preparation, capability control, continuity policy, and state persistence.

## Problem Statement

WorkClaw has already made real progress on the desktop local chat kernel:

- [session_engine.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs)
- [execution_plan.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs)
- [turn_preparation.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs)
- [turn_state.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs)
- [lane_executor.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs)
- [tool_setup.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs)
- [session_journal.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/session_journal.rs)

That work solved the first hard problem: desktop local chat is no longer just a pile of partial runtimes.

The next hard problem is larger:

**WorkClaw still does not have one runtime backbone for all session-like execution surfaces.**

Today these surfaces still diverge:

- desktop local chat
- hidden child sessions
- employee group execute steps
- team-entry and future collaboration-linked session surfaces

Each surface still owns some part of:

- system prompt shaping
- tool allowlist setup
- session persistence policy
- continuation semantics
- partial/final outcome handling
- observability projection

This is the remaining architectural gap between WorkClaw and a top-tier harness agent.

## What `close-code` Actually Gets Right

The most important lesson from `close-code` is not "it has multiple windows" or "it supports subagents."

`close-code` absolutely does support:

- session restore
- compaction and post-compaction continuation
- subagents and swarm execution
- pane-backed teammates
- worktree sessions

That is visible in:

- [QueryEngine.ts](/e:/code/yzpd/close-code/src/QueryEngine.ts)
- [query.ts](/e:/code/yzpd/close-code/src/query.ts)
- [state.ts](/e:/code/yzpd/close-code/src/bootstrap/state.ts)
- [sessionRestore.ts](/e:/code/yzpd/close-code/src/utils/sessionRestore.ts)
- [inProcessRunner.ts](/e:/code/yzpd/close-code/src/utils/swarm/inProcessRunner.ts)
- [PaneBackendExecutor.ts](/e:/code/yzpd/close-code/src/utils/swarm/backends/PaneBackendExecutor.ts)

The real lesson is this:

1. One session-level engine owns the conversation lifecycle.
2. One loop-level executor owns turn progression and recovery.
3. Subagents, swarm backends, compaction, permissions, and persistence attach to that backbone.
4. The product may expose multiple windows or channels, but the runtime does not fragment its core ownership.

So WorkClaw should learn from `close-code` by copying the ownership model, not the file layout.

## Current WorkClaw Baseline

### What is already strong

WorkClaw already has the right direction in the local chat kernel:

- explicit `ExecutionContext`
- explicit `TurnContext`
- explicit `ExecutionPlan`
- explicit `TurnStateSnapshot`
- compaction boundary persistence
- continuation preference and continuation turn policy
- capability snapshot and sectioned prompt assembly

This is already a better foundation than ad hoc patching.

### What is still split

The split now sits between surfaces:

#### 1. Local chat is on the new kernel, but child sessions are not

[child_session_runtime.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs) still runs a separate execution shape with its own:

- system prompt construction
- message assembly
- executor invocation
- persistence policy

It borrows some of the same infrastructure, but it is not actually owned by the same kernel spine.

#### 2. Employee group execute steps still build their own runtime slice

[group_run_entry.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/commands/employee_agents/group_run_entry.rs) still owns:

- employee-step system prompt shaping
- employee-step default tool policy
- employee-step iteration fallback output
- employee-step execution context

This means WorkClaw still has more than one way to define "what kind of agent turn is this?"

#### 3. Journal and recovery are improving, but still local-chat biased

[session_journal.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/session_journal.rs) and [session_runs.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/commands/session_runs.rs) now preserve `TurnStateSnapshot`, but the strongest continuity semantics still mostly reflect the local chat path.

#### 4. Permission and capability control are not yet fully surface-agnostic

WorkClaw now has a strong control plane in:

- [tool_dispatch.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs)
- [approval_gate.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/approval_gate.rs)
- [capability_snapshot.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/kernel/capability_snapshot.rs)

But not every execution surface is forced through those contracts yet.

## Architectural Goal

The next stage should not be named "more local chat refactoring."

It should be named:

**Desktop Local Chat Kernel -> Unified Session Spine**

The design goal is:

**Any WorkClaw execution surface that behaves like a session should run through one session-level spine.**

That does not mean every surface must share identical prompts or tools.

It means they must share:

- session preparation contract
- execution ownership model
- turn state contract
- continuation policy contract
- capability and permission control plane
- persistence and observability contract

## Target Architecture

### 1. `Session Spine` as the primary runtime backbone

Extend the current kernel so it becomes the owner of all session-shaped turns.

The session spine should accept a `SessionSurfaceKind`:

- `LocalChat`
- `HiddenChildSession`
- `EmployeeStepSession`
- future: `TeamEntrySession`

And a `SessionExecutionProfile` that describes:

- persona source
- prompt profile
- persistence visibility
- capability policy
- continuation policy
- stop/admission policy

The session spine should then perform:

1. load or construct the surface execution profile
2. prepare turn context
3. prepare execution context
4. resolve continuation policy
5. select execution lane
6. execute through the lane executor
7. commit normalized outcome and turn state

### 2. Surface adapters instead of surface runtimes

The existing local chat runtime should stay an adapter.

The child session runtime and employee step runtime should become adapters too.

That means:

- [session_runtime.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs) stays a user-facing desktop adapter
- [child_session_runtime.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs) becomes a child-session adapter
- [group_run_entry.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/commands/employee_agents/group_run_entry.rs) should eventually stop being an execution owner and instead request an `EmployeeStepSession`

The key rule is:

**Adapters may supply profile and surface metadata, but they must not own a separate mini-runtime.**

### 3. One `TurnStateSnapshot` contract for all session surfaces

`TurnStateSnapshot` should become the common session-state unit for:

- local chat runs
- hidden child session runs
- employee execute step runs

Minimum unified fields:

- execution lane
- selected runner
- invoked skills
- allowed tools
- route observation
- continuation metadata
- stop reason
- compaction boundary
- reconstructed history length
- partial/final output summary

This creates one common continuity and observability language.

### 4. One capability and permission control plane

Any session surface must derive prompt-visible and runtime-actual capability facts from the same source:

- [capability_snapshot.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/kernel/capability_snapshot.rs)
- [context_bundle.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/kernel/context_bundle.rs)
- [tool_dispatch.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs)
- [approval_gate.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/approval_gate.rs)

This means employee-step sessions should stop hardcoding default tools in their own command module and instead request an employee-step capability profile through the same kernel-owned assembly path.

### 5. Continuation policy should become session-wide policy

WorkClaw already has the right first pieces:

- compaction boundary propagation
- journal persistence
- UI projection
- continuation preference
- continuation turn policy

The next step is to make continuation policy surface-aware but kernel-owned.

That means the spine should decide:

- whether this turn is a continuation turn
- whether retries should be clamped
- whether the previous selected skill or runner should be preferred
- whether resumed execution should narrow admission or route fallback behavior

This should not live only in local chat logic.

### 6. Journal should become the universal session memory boundary

[session_journal.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/session_journal.rs) should become the common persistence contract for all session-shaped surfaces.

That does not require the same user-facing projection for every surface.

It does require:

- one turn-state persistence model
- one normalized terminal outcome model
- one trace/event vocabulary

That is what will let WorkClaw eventually support harness-grade replay, resume, and regression tests across surfaces.

## Migration Plan

### Phase 1: Promote local chat kernel to session spine

Add the missing abstractions but do not change local chat behavior:

- `SessionSurfaceKind`
- `SessionExecutionProfile`
- surface-aware turn preparation
- spine-owned continuation resolver

This phase is mostly contract work.

### Phase 2: Move hidden child sessions onto the spine

Refactor [child_session_runtime.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs) so it stops calling the executor as its own runtime owner.

Instead it should:

- build a hidden-child profile
- request a spine turn
- receive a normalized outcome
- project child-session-specific visibility back to the caller

### Phase 3: Move employee execute steps onto the spine

Refactor [group_run_entry.rs](/d:/code/WorkClaw/.worktrees/dr-cb/apps/runtime/src-tauri/src/commands/employee_agents/group_run_entry.rs) so employee step execution uses a dedicated `EmployeeStepSession` profile instead of building a prompt/tool/runtime slice inline.

This is the biggest architectural payoff because it removes an entire duplicate runtime pattern from WorkClaw.

### Phase 4: Unify observability and session projection

Once local chat, child sessions, and employee execute steps share one turn-state backbone, unify:

- session run projections
- trace export
- child session linkage
- employee step linkage
- session recovery summaries

This creates a shared diagnostic model across surfaces.

### Phase 5: Add harness-grade validation

After the spine exists, add regression coverage across surface types:

- local open task
- local implicit skill
- local explicit skill
- hidden child session
- employee execute step
- compaction then continue
- stop then continue
- permission clamp after continuation

This is the point where WorkClaw starts to behave like a top-tier harness runtime rather than just a capable chat runtime.

## Non-goals For This Stage

- Rebuilding the frontend window model
- Reproducing `close-code` pane or tmux UX literally
- Collapsing all employee group orchestration into the local chat runtime in one phase
- Rewriting `AgentExecutor`
- Replacing SQLite session projections with another storage model

## Main Risks

### 1. Over-unifying too early

The spine should unify execution ownership, not erase surface differences.

Child sessions and employee-step sessions still need different visibility and persona policies.

### 2. Breaking stable desktop local chat behavior

The current local chat kernel is the strongest baseline WorkClaw has ever had.

The spine migration must preserve that behavior while expanding ownership.

### 3. Creating a "god module"

This design should produce one backbone, not one giant file.

The correct shape is:

- one spine owner
- a small number of kernel contracts
- thin surface adapters

## Success Criteria

This design succeeds when:

1. Local chat, hidden child sessions, and employee execute steps all run through one session-level execution backbone.
2. Prompt-visible capabilities and actual executable capabilities are generated by the same control plane across surfaces.
3. `TurnStateSnapshot` becomes the common state contract for all session-like turns.
4. Compaction and continuation policies become kernel-owned session policies rather than local-chat-only behavior.
5. WorkClaw gains the same architectural strength that `close-code` shows: many surfaces, one runtime spine.
