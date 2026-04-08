# Desktop Local Chat Runtime Kernel Follow-Up Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Finish the must-do follow-up work so WorkClaw's desktop local chat runtime behaves like one coherent harness-grade kernel instead of several partially overlapping runtimes.

**Architecture:** Build on the kernel work already landed in `session_engine`, `execution_plan`, `turn_preparation`, `routed_prompt`, and `outcome_commit`. The remaining work is to move the rest of route-lane contracts, capability and context assembly, continuity state, control boundaries, and validation into kernel-owned services so `session_runtime`, `skill_routing::runner`, and `tool_setup` become thin adapters.

**Tech Stack:** Rust, Tauri, sqlx, serde_json, runtime-chat-app, Cargo tests, pnpm real-agent eval harness

---

## Current Baseline

Already landed in this worktree:

- `092ae40` `refactor(runtime): unify outcome commit path`
- `aa51e60` `test(runtime): cover terminal skill command outcomes`
- `add9741` `refactor(runtime): activate execution context snapshot`
- `81daca8` `refactor(runtime): split turn execution context`
- `f845b64` `refactor(runtime): pass turn and execution contexts explicitly`
- `aeccd67` `refactor(runtime): remove prepared send message wrapper`
- `cd7515f` `refactor(runtime): extract local turn preparation`
- `dd01e84` `refactor(runtime): extract routed prompt preparation`
- `1655a1b` `refactor(runtime): extract routed prompt execution`

This follow-up plan starts from that baseline and lists the remaining must-do optimization tasks.

## Execution Order

Implement in this order:

1. Kernel closure
2. Capability and context unification
3. Continuity and control
4. Harness-grade validation

Do not start continuity or validation work before kernel closure is finished, because those layers depend on stable kernel contracts.

### Task 1: Move Remaining Route-Lane Contracts Into `kernel`

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/route_lane.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`

**Goal:** Stop `skill_routing::runner` from owning runtime-facing lane contracts.

**Must move into `kernel`:**
- `RouteRunPlan`
- `RouteRunOutcome`
- `RoutedSkillToolSetup`
- any helper types that represent lane execution contracts instead of route heuristics

**Step 1: Add a contract test that still maps all implicit route decisions to valid lanes**

Run: `cargo test --lib skill_routing::runner -- --nocapture`
Expected: PASS before refactor

**Step 2: Create `kernel/route_lane.rs` and move the lane-owned types there**

The new module should own:

```rust
pub(crate) enum RouteRunPlan { ... }
pub(crate) enum RouteRunOutcome { ... }
pub(crate) struct RoutedSkillToolSetup { ... }
```

**Step 3: Update `runner.rs` to import these contracts instead of defining them locally**

`runner.rs` should keep:
- recall
- adjudication
- observation building
- minimal route-to-lane adaptation

It should stop owning runtime contracts.

**Step 4: Re-run the route runner tests**

Run: `cargo test --lib skill_routing::runner -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/route_lane.rs apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs
git commit -m "refactor(runtime): move route lane contracts into kernel"
```

### Task 2: Extract Direct-Dispatch Skill Execution Into `kernel`

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/direct_dispatch.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`

**Goal:** Make direct-dispatch skills a kernel execution bridge instead of a `runner` responsibility.

**Step 1: Preserve current direct-dispatch behavior with existing tests**

Run: `cargo test --lib skill_routing::runner -- --nocapture`
Expected: PASS before refactor

**Step 2: Add `kernel/direct_dispatch.rs`**

Move into this module:
- tool context setup for direct dispatch
- `ToolDispatchContext` construction
- dispatch invocation

The helper should look like:

```rust
pub(crate) async fn execute_direct_dispatch_skill(...) -> Result<String, String> { ... }
```

**Step 3: Update `runner.rs` so the direct-dispatch lane only calls the kernel helper**

After this step, `runner.rs` should not directly build `ToolDispatchContext`.

**Step 4: Re-run route runner tests**

Run: `cargo test --lib skill_routing::runner -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/direct_dispatch.rs apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs
git commit -m "refactor(runtime): extract direct dispatch lane execution"
```

### Task 3: Add a Single Lane Executor Boundary

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`

**Goal:** Make `SessionEngine` call one lane executor boundary instead of coordinating separate lane helpers manually.

**Step 1: Add a failing execution-lane contract test**

The test should assert all lanes can be dispatched through one executor boundary.

**Step 2: Introduce `lane_executor.rs`**

It should own:
- `execute_open_task`
- `execute_prompt_lane`
- `execute_direct_dispatch`
- final conversion into `ExecutionOutcome`

**Step 3: Update `SessionEngine` to delegate to `lane_executor`**

After this step:
- `SessionEngine` owns heartbeat
- `lane_executor` owns per-lane execution handoff
- `runner` is no longer a partial runtime

**Step 4: Verify execution plan and runtime tests**

Run: `cargo test --lib execution_plan -- --nocapture`
Run: `cargo test --lib session_runtime -- --nocapture`
Run: `cargo test --lib skill_routing::runner -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs
git commit -m "refactor(runtime): add kernel lane executor boundary"
```

### Task 4: Split `tool_setup` Into Capability Assembly Layers

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/tool_registry_setup.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/workspace_skill_context.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/capability_snapshot.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`

**Goal:** Stop `tool_setup` from mixing tool registration, capability snapshots, workspace skill prompt generation, memory injection, and prompt composition in one function.

**Split target:**
- `tool_registry_setup.rs`: registry mutation and tool alias setup
- `workspace_skill_context.rs`: workspace skills prompt + skill command specs
- `capability_snapshot.rs`: final prompt-visible capability facts
- `tool_setup.rs`: temporary thin wrapper only, then eventual removal

**Step 1: Preserve existing `tool_setup` tests**

Run: `cargo test --lib tool_setup -- --nocapture`
Expected: PASS before refactor

**Step 2: Extract registry mutation logic**

Move:
- `bash` replacement
- `exec` setup
- browser tools
- aliases
- task tool registration
- search fallback registration
- memory tool registration

**Step 3: Extract workspace skill prompt and command-spec assembly**

Move:
- `sync_workspace_skills_to_directory`
- workspace skill prompt generation
- `build_workspace_skill_command_specs`

**Step 4: Rebuild `CapabilitySnapshot` from these sublayers**

The snapshot must become the only source for:
- prompt-visible tool names
- skill command specs
- runtime notes
- allowed tool list

**Step 5: Verify**

Run: `cargo test --lib tool_setup -- --nocapture`
Run: `cargo test --lib capability_snapshot -- --nocapture`
Expected: PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/tool_registry_setup.rs apps/runtime/src-tauri/src/agent/runtime/kernel/workspace_skill_context.rs apps/runtime/src-tauri/src/agent/runtime/kernel/capability_snapshot.rs apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs
git commit -m "refactor(runtime): split tool setup into capability layers"
```

### Task 5: Finish `ContextBundle` as the Single Prompt Assembly Path

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/context_bundle.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
- Modify: `packages/runtime-chat-app/src/prompt_assembly.rs`
- Test: `packages/runtime-chat-app/tests/prompt_assembly.rs`

**Goal:** Ensure every desktop local turn uses one prompt assembly path, regardless of general chat, explicit skill, or implicit route skill.

**Must include explicit sections for:**
- base prompt
- capability snapshot
- runtime notes
- workspace skills prompt
- memory content
- employee collaboration guidance
- temporal execution guidance

**Step 1: Preserve prompt assembly regressions**

Run: `cargo test -p runtime-chat-app prompt_assembly -- --nocapture`
Expected: PASS before refactor

**Step 2: Expand `ContextBundle` to carry explicit prompt sections**

Avoid raw string glue in multiple places. `compose_system_prompt_from_tool_names` should be called once from the bundle builder.

**Step 3: Make `tool_setup` and routed prompt preparation consume only `ContextBundle` output**

There should be one final `system_prompt` assembly step.

**Step 4: Verify**

Run: `cargo test -p runtime-chat-app prompt_assembly -- --nocapture`
Run: `cargo test --lib context_bundle -- --nocapture`
Run: `cargo test --lib routed_prompt -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/context_bundle.rs apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs packages/runtime-chat-app/src/prompt_assembly.rs packages/runtime-chat-app/tests/prompt_assembly.rs
git commit -m "refactor(runtime): finish single prompt assembly path"
```

### Task 6: Add Turn-State Snapshot for Continuity, Resume, and Compaction

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/outcome_commit.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/compaction_pipeline.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs`

**Goal:** Introduce one kernel-owned turn snapshot so continuity features have a stable contract.

**Snapshot must hold at least:**
- route observation
- execution lane
- allowed tools
- invoked skill ledger
- partial assistant text
- tool failure streak
- compaction boundary metadata
- stop reason when present

**Step 1: Add a failing turn-state contract test**

The test should assert route and tool state can live in one snapshot.

**Step 2: Introduce `turn_state.rs`**

Create:

```rust
pub(crate) struct TurnStateSnapshot { ... }
```

**Step 3: Thread the snapshot through `SessionEngine` and `OutcomeCommitter`**

Do not change user-visible behavior yet. First make the snapshot authoritative.

**Step 4: Hook the snapshot into compaction and transcript reconstruction**

This is the foundation for future resume and long-task continuity.

**Step 5: Verify**

Run: `cargo test --lib turn_state -- --nocapture`
Run: `cargo test --lib session_runtime -- --nocapture`
Run: `cargo test --lib outcome_commit -- --nocapture`
Expected: PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs apps/runtime/src-tauri/src/agent/runtime/kernel/outcome_commit.rs apps/runtime/src-tauri/src/agent/runtime/compaction_pipeline.rs apps/runtime/src-tauri/src/agent/runtime/transcript.rs
git commit -m "refactor(runtime): add kernel turn state snapshot"
```

### Task 7: Unify Permission, Approval, and Sandbox Boundaries

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/approval_gate.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs`

**Goal:** Make tool control a single runtime boundary rather than a per-lane convention.

**Must centralize:**
- permission mode interpretation
- approval requests
- sandbox-related route execution inputs
- stop reason mapping for blocked or denied tools

**Step 1: Preserve tool dispatch tests**

Run: `cargo test --lib tool_dispatch -- --nocapture`
Expected: PASS before refactor

**Step 2: Add a shared control-plane input contract**

Move per-lane control facts into a shared kernel-facing struct instead of assembling them ad hoc.

**Step 3: Update lanes to consume only the shared contract**

After this step, no lane should hand-roll its own permission or approval inputs.

**Step 4: Verify**

Run: `cargo test --lib tool_dispatch -- --nocapture`
Run: `cargo test --lib session_runtime -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs apps/runtime/src-tauri/src/agent/runtime/approval_gate.rs apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs
git commit -m "refactor(runtime): unify tool control boundaries"
```

### Task 8: Add Harness-Grade Real-Agent Eval Matrix for Kernel Lanes

**Files:**
- Modify: `agent-evals/scenarios/*.yaml`
- Modify: `agent-evals/config/config.example.yaml`
- Modify: `apps/runtime/src-tauri/src/bin/agent_eval.rs`
- Modify: `docs/plans/2026-04-04-real-agent-evals-harness-plan.md`
- Test: local real-agent eval runs

**Goal:** Prove the kernel refactor preserves and improves real desktop runtime behavior across all important lane shapes.

**Must cover at least these scenarios:**
- open task
- implicit prompt skill inline
- implicit prompt skill fork
- explicit prompt-following skill
- direct-dispatch skill
- stopped run
- denied or blocked tool
- compaction or continuity boundary

**Step 1: Keep the existing golden scenario passing**

Run: `pnpm eval:agent-real --scenario pm_weekly_summary_xietao_2026_03_30_2026_04_04`
Expected: still routes through the skill session runner family and produces a pass report

**Step 2: Add new anonymous scenarios for each missing lane**

Do not include secrets, real local paths, or raw credentials in tracked files.

**Step 3: Update local-only guidance for validating kernel behavior**

Document expected fields:
- `selected_runner`
- `selected_skill`
- `turn_count`
- `tool_count`
- route observation
- stop reason when relevant

**Step 4: Verify local harness runs**

Run:
- `pnpm eval:agent-real --scenario <open_task_case>`
- `pnpm eval:agent-real --scenario <implicit_skill_case>`
- `pnpm eval:agent-real --scenario <explicit_skill_case>`
- `pnpm eval:agent-real --scenario <direct_dispatch_case>`

Expected: all produce `pass` or an explicitly understood `warn`, never silent drift

**Step 5: Commit**

```bash
git add agent-evals/scenarios agent-evals/config/config.example.yaml apps/runtime/src-tauri/src/bin/agent_eval.rs docs/plans/2026-04-04-real-agent-evals-harness-plan.md
git commit -m "test(runtime): expand real-agent kernel lane coverage"
```

## Required Verification Gates By Phase

### Kernel Closure
- `cargo test --lib execution_plan -- --nocapture`
- `cargo test --lib skill_routing::runner -- --nocapture`
- `cargo test --lib session_runtime -- --nocapture`

### Capability And Context Unification
- `cargo test --lib tool_setup -- --nocapture`
- `cargo test --lib capability_snapshot -- --nocapture`
- `cargo test --lib context_bundle -- --nocapture`
- `cargo test -p runtime-chat-app prompt_assembly -- --nocapture`

### Continuity And Control
- `cargo test --lib outcome_commit -- --nocapture`
- `cargo test --lib tool_dispatch -- --nocapture`
- `cargo test --lib session_runtime -- --nocapture`

### Full Fast Regression
- `pnpm test:rust-fast`

### Harness Validation
- `pnpm eval:agent-real --scenario <scenario_id>`

## Exit Criteria

This follow-up plan is complete only when all of the following are true:

- `SessionEngine` is the only desktop-local turn heartbeat
- `skill_routing::runner` is reduced to planning and adapter duties
- prompt-visible capabilities and real executable capabilities come from one capability snapshot
- prompt assembly has one authoritative path
- continuity state is owned by a kernel turn snapshot
- tool control and approval boundaries are centralized
- real-agent evals cover every important lane shape

## Notes

- Do not mix these commits with unrelated worktree changes.
- Prefer isolated commits per task so regressions are easy to bisect.
- Do not claim harness-grade parity until the real-agent eval matrix is in place.
