# Close-Code Aligned Agent Runtime Design

## Background

The current WorkClaw employee-agent stack mixes three responsibilities in a few hot paths:

1. Team topology inference
2. Delegation target selection
3. Step execution profile construction

This creates a structural gap between "who may execute" and "who should be dispatched to". That gap is the direct reason self-dispatch can leak into runtime behavior, such as `大皇子 -> 大皇子` repeatedly appearing in execution history.

The target direction is:

- Core agent runtime behavior should align with `E:\code\yzpd\close-code`
- WorkClaw-specific upper layers should adapt to that core
- `references\openclaw` should inform top-layer definitions such as persona, workspace layout, and Feishu/openclaw-style employee metadata, not the core runtime loop

## Strategy Summary

- Change surface: employee team orchestration, execution dispatch policy, employee execution profile construction
- Affected modules:
  - `apps/runtime/src-tauri/src/commands/employee_agents/team_rules.rs`
  - `apps/runtime/src-tauri/src/commands/employee_agents/group_run_execution_service.rs`
  - `apps/runtime/src-tauri/src/agent/runtime/kernel/employee_step_profile.rs`
  - new adapter/catalog modules under `apps/runtime/src-tauri/src/`
- Main risk: changing hot execution paths directly may conflict with parallel work and regress existing group-run behavior
- Recommended smallest safe path: introduce compatibility adapters first, then migrate callers incrementally
- Required verification: targeted Rust tests for topology/delegation/profile behavior, then `pnpm test:rust-fast`
- Release impact: low for packaging, medium for runtime behavior because employee group execution is user-visible

## Problem Analysis

### Symptom Layer

Observed behavior:

- the same employee keeps assigning work to itself
- history shows repeated `A -> A`
- runtime remains in execute/running rounds without producing meaningful role handoff

### Deeper Design Causes

1. Team topology and dispatch semantics are conflated
   - Existing logic treats "belongs to executable member set" and "valid dispatch target" as almost the same concept.
   - In practice, an employee can be a valid executor for a step without being a valid reassignment target from itself.

2. Legacy execute target shape cannot represent self-execution explicitly
   - `GroupRunExecuteTarget` models a dispatch edge only.
   - It cannot encode the distinction between:
     - "dispatch to another employee"
     - "same employee should execute locally without creating a dispatch edge"
   - As a result, fallback behavior can collapse into fake dispatch edges where `from == to`.

3. Agent role intent is implicit and scattered
   - Persona, workspace, tool permissions, and execution capability are assembled ad hoc in execution-profile code.
   - There is no central `AgentDefinition` abstraction carrying role kind, memory scope, permissions, and delegation capability.
   - Without that abstraction, higher layers cannot make reliable decisions such as "this role may execute but may not spawn/delegate".

4. Core runtime protections are not yet surfaced at employee-team boundary
   - WorkClaw already contains a meaningful task runtime (`task_record`, `task_lifecycle`, `task_transition`, `task_terminal`).
   - The employee-team layer does not currently map its delegation behavior into an explicit spawn/delegation policy aligned with a close-code-like core.
   - This creates a local orchestration bubble where invalid dispatch patterns can emerge before core lifecycle protections ever engage.

### Architectural Consequence

The bug is not just "missing `from != to` validation". That validation is necessary, but insufficient alone. The deeper issue is a missing separation between:

- team topology
- delegation policy
- agent capability definition
- execution entry into the core runtime

## Target Architecture

### Layer 1: Existing Core Task Runtime Remains the Runtime Kernel

WorkClaw already has reusable task lifecycle primitives:

- `task_record`
- `task_repo`
- `task_lifecycle`
- `task_transition`
- `task_terminal`
- `task_entry`
- `task_active_run`

These should remain the source of truth for task identity, lineage, lifecycle status, and journaling.

This phase will not replace the runtime kernel. Instead, upper layers will be aligned so later phases can enter that kernel with a cleaner close-code-like policy model.

### Layer 2: Agent Catalog

Introduce a narrow `agent_catalog` layer to normalize employee definitions into a runtime-facing shape:

- role kind
- workspace dir
- persona text
- allowed tools
- permission mode
- model id
- memory scope
- capability flags

This is the bridge between OpenClaw-style employee metadata and close-code-style core agent capability modeling.

### Layer 3: Employee Runtime Adapter

Introduce `employee_runtime_adapter` as a boundary layer that converts existing group/team data into explicit runtime decisions:

- `team_topology`
  - coordinator
  - planner
  - reviewer
  - executor set
- `delegation_policy`
  - explicit `DispatchToOther`
  - explicit `SelfExecute`
  - deduped legal targets only

This is where self-dispatch is structurally prevented.

### Layer 4: Core-Facing Spawn Policy

Later phases will add a core-facing policy surface:

- whether spawning/subagent delegation is allowed
- when self-execution is allowed
- when a handoff must target another employee
- when execution should remain local

That policy should align to `close-code` semantics rather than ad hoc group-run branching.

## Phase Plan

### Phase 1: Low-Conflict Compatibility Refactor

Goals:

- add `agent_catalog`
- add `employee_runtime_adapter`
- keep `team_rules.rs` as a compatibility wrapper
- add tests proving self-dispatch is not emitted as a dispatch edge

This phase should avoid deep edits to `group_run_execution_service.rs` and avoid touching task runtime internals.

### Phase 2: Execution Profile Alignment

Goals:

- make `employee_step_profile.rs` source persona/workspace/tool permissions from `AgentDefinition`
- add spawn/delegation policy evaluation helpers
- keep current function signatures stable where possible

### Phase 3: Execution Path Integration

Goals:

- route group execute decisions through `TeamRuntimeView` and `SpawnPolicy`
- propagate `SelfExecute` distinctly from `DispatchToOther`
- integrate with existing task runtime entry points instead of relying on legacy local assumptions

## Compatibility Rules

1. Do not remove existing public command signatures in the first phase.
2. Keep `team_rules.rs` callable by existing code.
3. Filter `SelfExecute` out of legacy dispatch-only return types until call sites are migrated.
4. Prefer additive modules over deep rewrites in files likely touched by other sessions.
5. Reuse current task runtime modules instead of introducing a second task kernel.

## Testing Strategy

Tests must first lock the intended semantics:

1. member normalization remains stable
2. single-member team topology is valid
3. planner/reviewer resolution follows explicit rules
4. valid multi-member dispatch returns `DispatchToOther`
5. self-dispatch is downgraded to `SelfExecute`
6. multi-member teams do not silently fake self-dispatch when no legal target exists
7. legacy compatibility wrapper filters non-dispatch results safely

## Non-Goals For Phase 1

- rewriting the full group-run state machine
- changing packaging or release behavior
- replacing task lifecycle storage
- redesigning Feishu integration
- changing workspace/on-disk openclaw employee asset format

## Success Criteria

Phase 1 is successful when:

- self-dispatch is no longer represented as a legal dispatch edge in the compatibility layer
- topology and delegation logic are separated into focused modules
- execution-profile construction has a clear migration path to agent definitions
- no core task runtime files need to be rewritten yet
