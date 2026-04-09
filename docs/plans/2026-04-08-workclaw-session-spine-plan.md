# WorkClaw Session Spine Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade WorkClaw from a desktop-local chat kernel to a unified session spine so local chat, hidden child sessions, and employee execute steps share one execution backbone.

**Architecture:** Build on the current local chat kernel in `session_engine`, `execution_plan`, `turn_preparation`, `turn_state`, `lane_executor`, `session_journal`, and the continuity work in `dr-cb`. The implementation should expand those kernel contracts into session-surface-aware runtime ownership rather than adding more surface-specific mini-runtimes.

**Tech Stack:** Rust, Tauri, sqlx, serde, runtime-chat-app, Cargo tests, pnpm fast Rust verification

---

## Current Baseline

This plan assumes the implementation starts from the compaction-boundary worktree baseline that already includes:

- `ExecutionContext`
- `TurnContext`
- `ExecutionPlan`
- `TurnStateSnapshot`
- `CapabilitySnapshot`
- `ContextBundle`
- compaction boundary persistence
- continuation preference
- continuation turn policy

Do not restart from pre-kernel code paths.

## Phase Order

Implement in this order:

1. Spine contracts
2. Hidden child session adoption
3. Employee step session adoption
4. Unified session projection
5. Validation expansion

Do not start employee-step migration before hidden child session migration is stable. Hidden child sessions are the smaller and safer proving ground.

### Task 1: Introduce Session Surface Contracts

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_profile.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`

**Goal:** Teach the kernel that a turn belongs to a session surface, not just a local chat path.

**Step 1: Preserve baseline kernel behavior**

Run: `cargo test --lib execution_plan -- --nocapture`
Run: `cargo test --lib turn_preparation -- --nocapture`
Run: `cargo test --lib session_runtime -- --nocapture`
Expected: PASS before refactor

**Step 2: Add session-surface contracts**

Create `session_profile.rs` with contracts similar to:

```rust
pub(crate) enum SessionSurfaceKind {
    LocalChat,
    HiddenChildSession,
    EmployeeStepSession,
}

pub(crate) struct SessionExecutionProfile {
    pub surface: SessionSurfaceKind,
    pub visibility: SessionVisibilityPolicy,
    pub persona: SessionPersonaProfile,
    pub capability_mode: SessionCapabilityProfile,
    pub continuation_mode: SessionContinuationProfile,
}
```

Keep the first version small. Do not over-generalize.

**Step 3: Thread the profile through turn preparation and session engine**

The local chat path should explicitly pass `SessionSurfaceKind::LocalChat`.

**Step 4: Add focused tests**

Add tests proving:

- local chat still maps to the existing preparation path
- the new profile contracts default safely

**Step 5: Verify**

Run: `cargo test --lib execution_plan -- --nocapture`
Run: `cargo test --lib turn_preparation -- --nocapture`
Run: `cargo test --lib session_runtime -- --nocapture`
Expected: PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_profile.rs apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs
git commit -m "refactor(runtime): add session spine surface contracts"
```

### Task 2: Make Session Engine Surface-Aware

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs`

**Goal:** Make the kernel emit surface-aware turn state and outcomes without changing local chat behavior.

**Step 1: Add surface metadata to turn state**

Extend `TurnStateSnapshot` so it can record the session surface kind.

**Step 2: Ensure lane execution receives the profile**

The lane executor should receive profile/surface metadata through `ExecutionContext` or an adjacent contract, not by reading globals.

**Step 3: Add tests**

Add tests proving a turn state can carry:

- session surface
- execution lane
- continuation metadata together

**Step 4: Verify**

Run: `cargo test --lib turn_state -- --nocapture`
Run: `cargo test --lib session_runtime -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs
git commit -m "refactor(runtime): make session engine surface aware"
```

### Task 3: Move Hidden Child Session Execution Onto The Spine

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs`
- Modify: `apps/runtime/src-tauri/src/session_journal.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs`

**Goal:** Stop hidden child sessions from being a separate runtime owner.

**Step 1: Preserve child-session behavior**

Run: `cargo test --lib child_session_runtime -- --nocapture`
Expected: PASS before refactor

**Step 2: Add a hidden-child surface profile**

The child-session adapter should supply:

- `SessionSurfaceKind::HiddenChildSession`
- hidden-session visibility policy
- child-session persona metadata
- child-session persistence policy

**Step 3: Replace direct executor ownership**

Refactor `child_session_runtime.rs` so it no longer builds a standalone execution loop with its own prompt and executor lifecycle.

It should:

- prepare hidden child session persistence shell
- request a spine turn
- translate the normalized outcome into child-session-specific visibility

**Step 4: Persist turn-state uniformly**

Make child sessions emit compatible turn-state metadata into the session journal.

**Step 5: Verify**

Run: `cargo test --lib child_session_runtime -- --nocapture`
Run: `cargo test --lib session_runtime -- --nocapture`
Run: `cargo test --lib outcome_commit -- --nocapture`
Expected: PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs apps/runtime/src-tauri/src/session_journal.rs
git commit -m "refactor(runtime): move hidden child sessions onto session spine"
```

### Task 4: Introduce Employee Step Session Profiles

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents/group_run_entry.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/employee_step_profile.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_profile.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
- Test: `apps/runtime/src-tauri/src/commands/employee_agents/group_run_entry.rs`

**Goal:** Stop employee group execute steps from defining their own ad hoc runtime slice.

**Step 1: Preserve existing employee-step execution behavior**

Run: `cargo test --lib employee_agents -- --nocapture`
Expected: PASS before refactor

**Step 2: Extract employee-step execution profile assembly**

Move employee-step-specific decisions out of `group_run_entry.rs`:

- persona prompt policy
- allowed tool policy
- max-iteration policy
- fallback delivery policy

Create a kernel-owned employee-step profile builder instead.

**Step 3: Replace inline runtime shaping in `group_run_entry.rs`**

`group_run_entry.rs` should stop being an execution owner and instead:

- request an employee-step session profile
- request spine execution
- consume the normalized outcome

**Step 4: Make employee-step capability assembly use the shared control plane**

Do not leave employee-step tool allowlists as a hardcoded side path if the same facts should exist in prompt and execution.

**Step 5: Verify**

Run: `cargo test --lib employee_agents -- --nocapture`
Run: `cargo test --lib tool_setup -- --nocapture`
Run: `pnpm test:rust-fast`
Expected: PASS

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/employee_agents/group_run_entry.rs apps/runtime/src-tauri/src/agent/runtime/kernel/employee_step_profile.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_profile.rs apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs
git commit -m "refactor(runtime): move employee step sessions onto session spine"
```

### Task 5: Unify Session Journal Projection Across Surfaces

**Files:**
- Modify: `apps/runtime/src-tauri/src/session_journal.rs`
- Modify: `apps/runtime/src-tauri/src/commands/session_runs.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents/group_run_entry.rs`
- Test: `apps/runtime/src-tauri/src/commands/session_runs.rs`

**Goal:** Make journal projections understand different session surfaces while keeping one turn-state vocabulary.

**Step 1: Add projection tests**

Add or update tests so `SessionRunProjection` can expose turn state from:

- local chat
- hidden child session
- employee step session

**Step 2: Extend projection carefully**

Add only the metadata needed to distinguish surface and resume semantics. Do not create a new incompatible state model.

**Step 3: Verify**

Run: `cargo test --lib session_journal -- --nocapture`
Run: `cargo test --lib session_runs -- --nocapture`
Expected: PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/session_journal.rs apps/runtime/src-tauri/src/commands/session_runs.rs apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs apps/runtime/src-tauri/src/commands/employee_agents/group_run_entry.rs
git commit -m "refactor(runtime): unify session journal projection across surfaces"
```

### Task 6: Expand Continuation Policy Into Session-Wide Policy

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_profile.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs`

**Goal:** Make compaction/recovery/continue semantics kernel-owned across session surfaces rather than local-chat-only.

**Step 1: Preserve current continuation behavior**

Run: `cargo test --lib turn_preparation -- --nocapture`
Run: `cargo test --lib skill_routing::runner -- --nocapture`
Expected: PASS before refactor

**Step 2: Add surface-aware continuation resolver logic**

Allow the spine to distinguish:

- normal continuation
- compacted continuation
- child-session continuation
- employee-step continuation

Keep the first version narrow. Prefer profile-level policy switches over branching everywhere.

**Step 3: Verify**

Run: `cargo test --lib turn_preparation -- --nocapture`
Run: `cargo test --lib session_runtime -- --nocapture`
Run: `cargo test --lib skill_routing::runner -- --nocapture`
Expected: PASS

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_profile.rs apps/runtime/src-tauri/src/agent/runtime/kernel/turn_state.rs
git commit -m "refactor(runtime): make continuation policy session wide"
```

### Task 7: Add Harness-Grade Cross-Surface Validation

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/*` as needed for tests only
- Modify: `agent-evals/scenarios/*` only if safe anonymous tracked scenarios are appropriate
- Test: focused Rust tests plus repo fast path

**Goal:** Prove the session spine works across surfaces and continuity states.

**Minimum regression matrix:**

- local chat open task
- local implicit skill
- local explicit skill
- hidden child session
- employee execute step
- compaction then continue
- stop then continue
- permission-clamped continuation

**Step 1: Add missing focused Rust tests**

Cover the kernel contracts directly before relying on slower evals.

**Step 2: Run the required verification**

Run: `cargo test --lib execution_plan -- --nocapture`
Run: `cargo test --lib turn_preparation -- --nocapture`
Run: `cargo test --lib turn_state -- --nocapture`
Run: `cargo test --lib child_session_runtime -- --nocapture`
Run: `cargo test --lib employee_agents -- --nocapture`
Run: `cargo test --lib session_journal -- --nocapture`
Run: `cargo test --lib session_runs -- --nocapture`
Run: `pnpm test:rust-fast`
Expected: PASS

**Step 3: Optional real-agent follow-up**

If local config exists, run:

```bash
pnpm eval:agent-real --scenario pm_weekly_summary_xietao_2026_03_30_2026_04_04
```

Only do this when `agent-evals/config/config.local.yaml` is available in the active worktree.

**Step 4: Commit**

```bash
git add apps/runtime/src-tauri/src agent-evals/scenarios
git commit -m "test(runtime): add session spine cross-surface regression coverage"
```

## Completion Criteria

This plan is complete when:

1. Local chat remains on the kernel and becomes explicitly one `SessionSurfaceKind`.
2. Hidden child sessions run through the same session spine instead of their own execution owner.
3. Employee execute steps run through the same session spine instead of building inline runtime slices.
4. `TurnStateSnapshot` becomes the common cross-surface turn-state contract.
5. Continuation policy becomes session-wide kernel policy.
6. Journal and projection layers can explain cross-surface runtime state with one vocabulary.
