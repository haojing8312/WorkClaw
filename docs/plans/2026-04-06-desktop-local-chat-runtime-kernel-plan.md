# Desktop Local Chat Runtime Kernel Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor WorkClaw's desktop local chat runtime so one coherent kernel owns local turn preparation, route planning, context and capability assembly, execution handoff, and normalized outcome persistence.

**Architecture:** Introduce a dedicated kernel under `apps/runtime/src-tauri/src/agent/runtime/kernel/` and move the desktop local chat path to a single `SessionEngine -> ExecutionPlan -> CapabilitySnapshot/ContextBundle -> ExecutionOutcome -> OutcomeCommitter` flow. Keep `attempt_runner`, `tool_dispatch`, and `turn_executor` as specialized executors, while shrinking `session_runtime`, `skill_routing::runner`, and `tool_setup` into thinner adapters.

**Tech Stack:** Rust, Tauri, sqlx, serde_json, runtime-chat-app, Cargo tests, pnpm real-agent eval harness

---

### Task 0: Create a Dedicated Implementation Worktree

**Files:**
- Modify: none
- Verify: `git worktree list`

**Step 1: Create the runtime-kernel worktree**

```bash
git worktree add ..\\WorkClaw-desktop-runtime-kernel -b feat/desktop-runtime-kernel
```

**Step 2: Verify the worktree exists**

Run: `git worktree list`
Expected: includes `..\WorkClaw-desktop-runtime-kernel`

**Step 3: Switch to the new worktree**

```bash
cd ..\\WorkClaw-desktop-runtime-kernel
git status --short
```

**Step 4: Confirm the branch is isolated**

Expected: branch is `feat/desktop-runtime-kernel` and the new worktree is clean before code changes

**Step 5: Commit**

No commit for this task.

### Task 1: Add the Kernel Module Skeleton and Shared Contracts

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/capability_snapshot.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/context_bundle.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/kernel/outcome_commit.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/mod.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`

**Step 1: Write the failing kernel contract test**

```rust
#[test]
fn execution_plan_supports_all_desktop_runtime_lanes() {
    use crate::agent::runtime::kernel::execution_plan::ExecutionLane;

    let lanes = [
        ExecutionLane::OpenTask,
        ExecutionLane::PromptInline,
        ExecutionLane::PromptFork,
        ExecutionLane::DirectDispatch,
    ];

    assert_eq!(lanes.len(), 4);
}
```

**Step 2: Run the focused test to verify it fails**

Run: `cargo test --lib execution_plan_supports_all_desktop_runtime_lanes -- --nocapture`
Expected: FAIL because the kernel module and execution plan types do not exist yet

**Step 3: Add the minimal kernel module surface**

```rust
// apps/runtime/src-tauri/src/agent/runtime/kernel/mod.rs
pub(crate) mod capability_snapshot;
pub(crate) mod context_bundle;
pub(crate) mod execution_plan;
pub(crate) mod outcome_commit;
pub(crate) mod session_engine;
```

```rust
// apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExecutionLane {
    OpenTask,
    PromptInline,
    PromptFork,
    DirectDispatch,
}
```

**Step 4: Run the focused test to verify it passes**

Run: `cargo test --lib execution_plan_supports_all_desktop_runtime_lanes -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/mod.rs apps/runtime/src-tauri/src/agent/runtime/kernel
git commit -m "refactor(runtime): add desktop runtime kernel contracts"
```

### Task 2: Move Session Preparation Behind `SessionEngine`

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`

**Step 1: Add a regression test for thin entry behavior**

```rust
#[test]
fn session_runtime_still_parses_user_skill_commands_after_engine_extraction() {
    let parsed = SessionRuntime::parse_user_skill_command("/pm_summary --employee xt");
    assert_eq!(
        parsed,
        Some(("pm_summary".to_string(), "--employee xt".to_string()))
    );
}
```

**Step 2: Run the session runtime tests before refactoring**

Run: `cargo test --lib session_runtime -- --nocapture`
Expected: PASS

**Step 3: Extract `PreparedSendMessageContext` handoff into `SessionEngine`**

```rust
// session_engine.rs
pub(crate) struct SessionEngine;

impl SessionEngine {
    pub(crate) async fn run_local_turn(...) -> Result<ExecutionOutcome, String> {
        // prepare session state
        // plan route
        // assemble context and capabilities
        // execute lane
        // commit outcome
    }
}
```

**Step 4: Re-run the session runtime tests**

Run: `cargo test --lib session_runtime -- --nocapture`
Expected: PASS with `session_runtime` acting as a thinner entry wrapper

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs
git commit -m "refactor(runtime): route local session entry through session engine"
```

### Task 3: Extract `CapabilitySnapshot` From `tool_setup`

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/capability_snapshot.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`

**Step 1: Add a failing capability snapshot test**

```rust
#[test]
fn capability_snapshot_keeps_prompt_visible_tools_and_dispatch_specs_together() {
    use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;

    let snapshot = CapabilitySnapshot::default();
    assert!(snapshot.resolved_tool_names.is_empty());
    assert!(snapshot.skill_command_specs.is_empty());
}
```

**Step 2: Run the focused test to verify current failure**

Run: `cargo test --lib capability_snapshot_keeps_prompt_visible_tools_and_dispatch_specs_together -- --nocapture`
Expected: FAIL because `CapabilitySnapshot` does not exist yet

**Step 3: Extract capability state from `prepare_runtime_tools`**

```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct CapabilitySnapshot {
    pub allowed_tools: Option<Vec<String>>,
    pub resolved_tool_names: Vec<String>,
    pub skill_command_specs: Vec<chat_io::WorkspaceSkillCommandSpec>,
    pub runtime_notes: Vec<String>,
}
```

**Step 4: Re-run `tool_setup` and capability tests**

Run: `cargo test --lib tool_setup capability_snapshot -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs apps/runtime/src-tauri/src/agent/runtime/kernel/capability_snapshot.rs apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs
git commit -m "refactor(runtime): extract capability snapshot from tool setup"
```

### Task 4: Extract `ContextBundle` and Stop Building Prompts Inside `tool_setup`

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/context_bundle.rs`
- Modify: `packages/runtime-chat-app/src/prompt_assembly.rs`
- Test: `packages/runtime-chat-app/tests/prompt_assembly.rs`

**Step 1: Add a failing context bundle test**

```rust
#[test]
fn context_bundle_uses_runtime_notes_and_memory_in_one_prompt_path() {
    use crate::agent::runtime::kernel::context_bundle::ContextBundle;

    let bundle = ContextBundle::default();
    assert!(bundle.system_prompt.is_empty());
}
```

**Step 2: Run the prompt assembly regression tests**

Run: `cargo test -p runtime-chat-app prompt_assembly -- --nocapture`
Expected: PASS before refactor

**Step 3: Introduce a single prompt-bundle builder**

```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct ContextBundle {
    pub system_prompt: String,
    pub workspace_skills_prompt: Option<String>,
    pub memory_content: Option<String>,
}
```

```rust
impl ContextBundle {
    pub(crate) fn build(...) -> Self {
        // call compose_system_prompt exactly once here
    }
}
```

**Step 4: Re-run prompt assembly and kernel tests**

Run: `cargo test -p runtime-chat-app prompt_assembly -- --nocapture`
Run: `cargo test --lib context_bundle -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs apps/runtime/src-tauri/src/agent/runtime/kernel/context_bundle.rs packages/runtime-chat-app/src/prompt_assembly.rs packages/runtime-chat-app/tests/prompt_assembly.rs
git commit -m "refactor(runtime): extract prompt and context bundle assembly"
```

### Task 5: Reduce `skill_routing::runner` to Planning and Lane Adapters

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`

**Step 1: Add a failing route-plan contract test**

```rust
#[test]
fn implicit_route_planning_returns_execution_lane_instead_of_owning_runtime_setup() {
    use crate::agent::runtime::kernel::execution_plan::ExecutionLane;

    let lane = ExecutionLane::OpenTask;
    assert_eq!(lane, ExecutionLane::OpenTask);
}
```

**Step 2: Run the route runner tests before moving ownership**

Run: `cargo test --lib skill_routing::runner -- --nocapture`
Expected: PASS

**Step 3: Make route planning return `ExecutionPlan` plus route observation**

```rust
impl ExecutionPlan {
    pub(crate) fn from_route_plan(...) -> Self {
        // map OpenTask / PromptInline / PromptFork / DirectDispatch
    }
}
```

**Step 4: Re-run the route runner tests**

Run: `cargo test --lib skill_routing::runner -- --nocapture`
Expected: PASS with route runner reduced to planning and adapter duties

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs
git commit -m "refactor(runtime): reduce skill routing runner to lane adapters"
```

### Task 6: Add `OutcomeCommitter` and Unify Terminal Persistence

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/outcome_commit.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_events.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_events.rs`

**Step 1: Add a failing outcome commit test**

```rust
#[tokio::test]
async fn outcome_committer_writes_success_and_failure_through_one_path() {
    use crate::agent::runtime::kernel::outcome_commit::ExecutionOutcome;

    let outcome = ExecutionOutcome::default();
    assert!(outcome.partial_text.is_empty());
}
```

**Step 2: Run runtime event tests before the move**

Run: `cargo test --lib runtime_events -- --nocapture`
Expected: PASS

**Step 3: Centralize commit behavior**

```rust
pub(crate) struct OutcomeCommitter;

impl OutcomeCommitter {
    pub(crate) async fn commit(...) -> Result<(), String> {
        // started / partial / success / failed / stopped
    }
}
```

**Step 4: Re-run runtime event and session runtime tests**

Run: `cargo test --lib runtime_events -- --nocapture`
Run: `cargo test --lib session_runtime -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel/outcome_commit.rs apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_events.rs apps/runtime/src-tauri/src/agent/runtime/transcript.rs
git commit -m "refactor(runtime): unify desktop runtime outcome commit path"
```

### Task 7: Verification and Baseline Comparison

**Files:**
- Modify: only if verification reveals fixes
- Verify: existing runtime files and tests above

**Step 1: Run focused Rust runtime verification**

Run:
- `cargo test --lib session_runtime -- --nocapture`
- `cargo test --lib skill_routing::runner -- --nocapture`
- `cargo test --lib tool_dispatch -- --nocapture`
- `cargo test --lib runtime_events -- --nocapture`

Expected: PASS

**Step 2: Run the fast Rust lane**

Run: `pnpm test:rust-fast`
Expected: PASS

**Step 3: Run one real desktop-local scenario**

Run: `pnpm eval:agent-real --scenario pm_weekly_summary_xietao_2026_03_30_2026_04_04`
Expected: PASS or an explained `warn`, with route and runner data preserved

**Step 4: Record before/after kernel observations**

Capture:

- selected runner
- route observation
- turn count
- tool count
- total duration

Expected: no behavior regression on explicit skill, implicit skill, and open-task baselines

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/kernel apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_events.rs apps/runtime/src-tauri/src/agent/runtime/transcript.rs packages/runtime-chat-app/src/prompt_assembly.rs packages/runtime-chat-app/tests/prompt_assembly.rs docs/plans/2026-04-06-desktop-local-chat-runtime-kernel-design.md docs/plans/2026-04-06-desktop-local-chat-runtime-kernel-plan.md
git commit -m "refactor(runtime): align desktop local chat kernel ownership"
```
