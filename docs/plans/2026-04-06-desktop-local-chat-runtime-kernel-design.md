# Desktop Local Chat Runtime Kernel Design

**Date:** 2026-04-06

**Goal:** Refactor WorkClaw's desktop local chat runtime so one coherent kernel owns local session preparation, route planning, prompt and capability assembly, turn execution, and outcome persistence without changing IM, employee orchestration, or other cross-surface systems in this phase.

## Scope

This design only covers the desktop local chat runtime path.

Included:

- local session preparation
- local message-to-route planning
- prompt and context assembly for desktop local chat
- tool and skill capability assembly
- route-lane execution handoff
- local run outcome persistence and observability

Explicitly excluded in this phase:

- IM and Feishu bridge flows
- employee team orchestration
- group run and review workflows
- sidecar vendor boundary changes
- release packaging behavior
- a full compaction or swarm redesign

## Why This Refactor Is Needed

WorkClaw already has strong runtime capabilities for desktop local chat:

- message entry and run ownership in [session_runtime.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs)
- model candidate execution and failover in [attempt_runner.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/attempt_runner.rs)
- route planning and lane selection in [skill_routing/runner.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs)
- tool lifecycle and approval in [tool_dispatch.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs)
- transcript reconstruction in [transcript.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/transcript.rs)
- prompt assembly in [prompt_assembly.rs](/d:/code/WorkClaw/packages/runtime-chat-app/src/prompt_assembly.rs)

The problem is not missing capability. The problem is that runtime ownership is still fragmented.

Today the desktop local chat path effectively spreads kernel responsibilities across:

- `session_runtime`
- `skill_routing::runner`
- `tool_setup`
- `attempt_runner`
- `runtime_events`
- `prompt_assembly`

That means a single user turn can still branch into multiple execution shapes that each partially own:

- prompt assembly
- tool allowlist narrowing
- skill contract overrides
- run lifecycle persistence
- trace and route observability

This fragmentation is the biggest remaining gap between WorkClaw and a top-tier harness agent runtime.

## Architectural Reference From `close-code`

The relevant lesson from `close-code` is not "copy Claude Code prompts" or "copy one tool."

The important pattern is:

1. One session-level orchestrator owns the conversation lifecycle.
2. One loop-level executor owns the turn heartbeat.
3. Prompt, context, memory, tools, skills, permissions, and tasking are service layers around that heartbeat.
4. Subsystems do not each own their own partial runtime.

In `close-code`, that ownership is visibly centered around:

- [QueryEngine.ts](/e:/code/yzpd/close-code/src/QueryEngine.ts)
- [query.ts](/e:/code/yzpd/close-code/src/query.ts)
- [context.ts](/e:/code/yzpd/close-code/src/context.ts)

WorkClaw should not try to reproduce that architecture literally, because:

- WorkClaw uses Rust + Tauri rather than Bun + TS
- WorkClaw already has a functioning `AgentExecutor`
- WorkClaw already invested in route planning, real-agent evals, and SQLite-backed run projection

The right goal is to reproduce the ownership model, not the code shape.

## Design Principles

1. One desktop-local kernel owns one user turn from prepared input to committed outcome.
2. Route planning decides the lane, but does not own prompt assembly, capability setup, or persistence.
3. Prompt-visible capabilities and actual executable capabilities must come from the same snapshot.
4. Run outcome persistence must have one authoritative path for success, failure, stop, and partial output.
5. Existing behavior should be preserved unless the refactor intentionally narrows ambiguity.
6. The first phase should extract kernel contracts before changing user-visible runtime semantics.

## Current Problem Areas

### 1. `session_runtime` owns too much

[session_runtime.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs) currently mixes:

- session entry
- session preparation
- explicit command dispatch
- implicit route planning
- skill override handling
- message reconstruction
- runtime tool setup
- direct route execution
- final outcome commit

That file is the entrypoint, planner, executor coordinator, and persistence caller at the same time.

### 2. Route runners still behave like mini-runtimes

[skill_routing/runner.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs) currently does more than translate route decisions into lanes. It also participates in:

- runtime tool preparation
- prompt and tool narrowing
- fork-message shaping
- direct-dispatch execution

That makes the route subsystem own pieces of the kernel.

### 3. Tool setup still mixes capability assembly and prompt generation

[tool_setup.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs) currently:

- registers tools into the runtime
- resolves allowed tools
- builds skill command specs
- resolves search fallback behavior
- loads memory
- prepares workspace skills prompt
- composes the system prompt

That creates a hidden coupling where prompt assembly and actual capability assembly are intertwined.

### 4. Persistence ownership is still distributed

Run lifecycle commits currently happen from several places:

- route attempt execution
- direct dispatch handling
- final assistant reconstruction
- runtime events

This makes it harder to add future continuity features such as:

- compact boundary ownership
- invoked skill ledger
- turn replay
- kernel-level state snapshots

## Target Architecture

### 1. `SessionEngine`

Add a new session-level kernel owner under:

- `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`

Responsibilities:

- accept one prepared desktop-local message submission
- load the current turn state
- request route planning
- request context and capability assembly
- invoke the correct execution adapter
- commit one normalized outcome

`SessionEngine` becomes the desktop local runtime equivalent of a session orchestrator.

It should not:

- execute tool calls directly
- implement model failover directly
- own route heuristics inline
- serialize SQL row shapes inline

### 2. `ExecutionPlan`

Add a shared execution contract under:

- `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`

Core types:

- `ExecutionLane`
- `ExecutionPlan`
- `ExecutionContext`
- `ExecutionOutcome`

The purpose of this contract is to stop each lane from inventing its own mini-runtime.

The first-phase `ExecutionLane` values should be:

- `OpenTask`
- `PromptInline`
- `PromptFork`
- `DirectDispatch`

Each user turn should produce exactly one `ExecutionPlan`.

### 3. `CapabilitySnapshot`

Extract capability assembly from [tool_setup.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs) into:

- `apps/runtime/src-tauri/src/agent/runtime/kernel/capability_snapshot.rs`

`CapabilitySnapshot` should own:

- allowed tool list
- resolved tool names for prompt display
- runtime notes such as browser/search fallback state
- skill command specs
- alias mapping and compatibility alias state
- prompt-visible capability inventory

The design rule is:

**The prompt must describe the exact same capability snapshot the executor can actually use.**

This eliminates an entire category of drift bugs.

### 4. `ContextBundle`

Extract prompt and context composition into:

- `apps/runtime/src-tauri/src/agent/runtime/kernel/context_bundle.rs`

`ContextBundle` should gather:

- base skill or general system prompt
- workspace skills prompt
- memory content
- temporal context
- structured tool result guidance
- runtime notes derived from `CapabilitySnapshot`

The first phase can continue to call [compose_system_prompt()](/d:/code/WorkClaw/packages/runtime-chat-app/src/prompt_assembly.rs), but that call should happen in one place only.

Longer term, this bundle should evolve into section-based prompt ownership similar to the strengths seen in `close-code`, but without introducing a large user-visible prompt rewrite in the same phase.

### 5. `OutcomeCommitter`

Extract outcome persistence into:

- `apps/runtime/src-tauri/src/agent/runtime/kernel/outcome_commit.rs`

This module should be the only place that commits:

- run started
- partial assistant output
- success
- failure
- stopped outcome
- reconstructed final content
- route metadata and trace metadata

This module should reuse existing infrastructure:

- [transcript.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/transcript.rs)
- `runtime_io::runtime_events`
- `session_journal`

The goal is not to replace that infrastructure, only to unify ownership over when and how it is used.

## New Runtime Flow

After the refactor, the desktop local chat runtime should conceptually behave like this:

1. `prepare_session_state`
2. `plan_route`
3. `assemble_context_and_capabilities`
4. `execute_turn`
5. `commit_outcome`

### Stage 1: `prepare_session_state`

Owned by `SessionEngine`.

Inputs:

- session id
- user message
- user message parts
- max-iteration override

Outputs:

- reconstructed message history
- permission mode
- work dir
- route candidates
- workspace skill entries
- route index
- execution guidance
- memory bucket employee id

This mostly reuses current session preparation logic from [session_runtime.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs).

### Stage 2: `plan_route`

Owned by route planning modules, but returning `ExecutionPlan` only.

Inputs:

- route index
- workspace skill entries
- skill command specs
- user message

Outputs:

- one `ExecutionPlan`
- route observation payload

This lets [skill_routing/runner.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs) become a planner plus adapter rather than a partial runtime.

### Stage 3: `assemble_context_and_capabilities`

Owned by `ContextBundle` + `CapabilitySnapshot`.

Inputs:

- `ExecutionPlan`
- prepared session state

Outputs:

- prompt-ready system bundle
- execution-ready capability snapshot
- narrowed tool allowlist
- skill overrides

This stage must happen exactly once per turn and must serve all lanes.

### Stage 4: `execute_turn`

Lane-specific adapters call existing executors:

- `OpenTask` -> [attempt_runner.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/attempt_runner.rs) through the current candidate execution path
- `PromptInline` -> same executor with narrowed prompt and capability snapshot
- `PromptFork` -> same executor with fork-shaped messages and narrowed snapshot
- `DirectDispatch` -> [tool_dispatch.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs)

The key design point is:

**The executor differs by lane, but the kernel contract does not.**

### Stage 5: `commit_outcome`

All lanes convert into one `ExecutionOutcome`.

That outcome is committed through one `OutcomeCommitter`.

This gives WorkClaw a future-safe place to add:

- compact boundary ownership
- invoked skill ledger
- unified stop-reason persistence
- route-to-outcome trace correlation

## File-Level Refactor Map

### New files

- `apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs`
- `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- `apps/runtime/src-tauri/src/agent/runtime/kernel/capability_snapshot.rs`
- `apps/runtime/src-tauri/src/agent/runtime/kernel/context_bundle.rs`
- `apps/runtime/src-tauri/src/agent/runtime/kernel/outcome_commit.rs`

### Existing files that should become thinner

- [session_runtime.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs)
- [skill_routing/runner.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs)
- [tool_setup.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs)

### Existing files that should stay specialized

- [attempt_runner.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/attempt_runner.rs)
- [tool_dispatch.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs)
- [transcript.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/transcript.rs)
- [turn_executor.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/turn_executor.rs)

## Migration Phases

### Phase 1: Extract contracts without changing lane behavior

Goals:

- add `ExecutionPlan`
- add `ExecutionContext`
- add `ExecutionOutcome`
- add `SessionEngine`
- keep existing lane behavior intact

This is a structural refactor, not a behavioral rewrite.

### Phase 2: Extract capability and context snapshots

Goals:

- move tool and skill runtime preparation into `CapabilitySnapshot`
- move prompt and runtime-note assembly into `ContextBundle`
- remove prompt composition from `tool_setup`

This phase removes the biggest source of hidden coupling.

### Phase 3: Reduce route runners to planners and adapters

Goals:

- make route planning return `ExecutionPlan`
- stop `skill_routing::runner` from acting like a mini-runtime
- make `DirectDispatch` just another execution lane

### Phase 4: Unify outcome commit and observability

Goals:

- move final result commit into `OutcomeCommitter`
- make run lifecycle ownership uniform across all lanes
- prepare for later continuity features

## Risks

### Runtime regression risk

The biggest risk is silently changing behavior while moving ownership.

Mitigation:

- phase the refactor
- lock current route behavior with tests before moving responsibilities
- keep existing executors in place during phase 1

### Prompt drift risk

If `CapabilitySnapshot` and prompt assembly diverge during migration, WorkClaw could briefly regress into "prompt says tool exists, runtime says it does not."

Mitigation:

- make `ContextBundle` consume `CapabilitySnapshot`
- prohibit prompt capability listing from any other source in new code

### Persistence split-brain risk

If old and new commit paths both partially write run results, the journal may get inconsistent success/failure states.

Mitigation:

- centralize the final commit path before deleting old helpers
- keep outcome commit tests focused on terminal state transitions

## Success Criteria

The first-phase refactor is successful when:

- one `SessionEngine` owns desktop local chat turn orchestration
- every turn yields one `ExecutionPlan`
- every plan executes against one `CapabilitySnapshot`
- every lane commits through one `OutcomeCommitter`
- `session_runtime` becomes a thin entrypoint instead of a runtime god object
- existing explicit skill, implicit skill, and open-task desktop behaviors remain stable

## Verification Expectations

Implementation should ship with:

- focused Rust tests for `execution_plan`
- focused Rust tests for `capability_snapshot`
- focused Rust tests for `context_bundle`
- focused Rust tests for outcome commit behavior
- regression coverage for `session_runtime`
- regression coverage for `skill_routing::runner`
- regression coverage for `tool_dispatch`
- `pnpm test:rust-fast`
- at least one `pnpm eval:agent-real --scenario <id>` comparison on a desktop-local baseline

## Out Of Scope Follow-Ups

After this desktop local kernel refactor lands, likely next-step follow-ups are:

- compact boundary and invoked-skill continuity
- durable child-session alignment
- desktop and IM runtime contract convergence
- harder trust/permission/sandbox layering across local and remote lanes
