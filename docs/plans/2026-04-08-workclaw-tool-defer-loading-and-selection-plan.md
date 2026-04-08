# WorkClaw Tool Defer Loading And Selection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Turn WorkClaw's unified tool pool into a staged exposure system with recommended tools, deferred tools, and a safe expansion path to the full tool set.

**Architecture:** Keep `effective_tool_set` as the single policy-valid source of truth, then layer a small internal loading planner on top of it. The runtime will expose `active_tools` first, retain `full_allowed_tools` as authority, and expand when the initial recommendation set proves insufficient.

**Tech Stack:** Rust, Tauri runtime, existing WorkClaw agent runtime, serde/serde_json, runtime-policy, runtime-skill-core, Rust unit tests, `pnpm test:rust-fast`.

---

### Task 1: Add A Stable Tool Loading Planner Model

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/effective_tool_set.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_catalog.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/effective_tool_set.rs`

**Step 1: Write the failing test**

Add planner tests that construct a policy-valid tool set and assert the runtime can derive:
- `full_allowed_tools`
- `recommended_tools`
- `active_tools`
- `deferred_tools`
- `loading_policy`

Include at least:
- one case where recommendation narrows exposure
- one case where recommendation is empty and the planner falls back to full exposure

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile-only verification passes before implementation, but the new planner tests are not yet present.

**Step 3: Write minimal implementation**

Introduce a small planner state in `effective_tool_set.rs` that derives:
- `full_allowed_tools`
- `recommended_tools`
- `active_tools`
- `deferred_tools`

Keep the current decision record compatible by adding the new data rather than replacing old fields.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile succeeds with the new planner model.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/effective_tool_set.rs apps/runtime/src-tauri/src/agent/runtime/tool_catalog.rs
git commit -m "feat(tooling): add staged tool loading planner"
```

### Task 2: Switch Tool Setup To Active Tool Exposure

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`

**Step 1: Write the failing test**

Add a runtime tool setup test that proves:
- the planner can keep a larger full allowed set
- the initial exposed tool list is the smaller active set
- deferred tools remain recorded in planner output

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile-only verification before implementation.

**Step 3: Write minimal implementation**

Update `prepare_runtime_tools()` so the model-facing tool list uses `active_tools`, not the full set. Keep the full set in planner state and logs.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile succeeds and tool setup fixtures reflect active-vs-full separation.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs
git commit -m "refactor(tooling): expose active tools from loading planner"
```

### Task 3: Add One-Step Expansion To Full Tool Exposure

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/attempt_runner.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`

**Step 1: Write the failing test**

Add a runtime-focused test that simulates a deferred tool need and asserts the planner can mark the session as expanded to full allowed tools.

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile-only verification before implementation.

**Step 3: Write minimal implementation**

Implement one-step expansion:
- start with active/recommended exposure
- detect a deferred-tool insufficiency signal
- switch to full allowed exposure
- record that expansion occurred

Do not add multiple expansion stages in this pass.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile succeeds with expansion state and tests in place.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/attempt_runner.rs apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs
git commit -m "feat(tooling): expand deferred tool exposure on demand"
```

### Task 4: Record Loading Policy And Expansion State

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/observability.rs`
- Modify: `apps/runtime/src-tauri/src/session_journal.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_events.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/observability.rs`
- Test: `apps/runtime/src-tauri/src/session_journal.rs`

**Step 1: Write the failing test**

Add journal/observability tests that assert route records now include:
- loading policy
- deferred tool count
- whether expansion happened

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile-only verification before implementation.

**Step 3: Write minimal implementation**

Extend the decision record and recent route snapshots so loading strategy becomes first-class runtime telemetry.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile succeeds and telemetry fixtures carry the new planner state.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/observability.rs apps/runtime/src-tauri/src/session_journal.rs apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_events.rs
git commit -m "feat(tooling): record deferred loading planner state"
```

### Task 5: Add Tool-Side Route Explanation

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/observability.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/observability.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/observability.rs`

**Step 1: Write the failing test**

Add a route observation test that asserts route records can explain:
- which tool capabilities were recommended
- whether route selection aligned with them
- whether fallback happened while tool-side recommendation was still strong

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile-only verification before implementation.

**Step 3: Write minimal implementation**

Extend implicit route observation with a compact tool-side explanation summary based on the planner's recommendation and loading state. Keep route scoring unchanged.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: compile succeeds and route observation carries the new explanation fields.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/skill_routing/observability.rs apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs apps/runtime/src-tauri/src/agent/runtime/observability.rs
git commit -m "feat(tooling): explain route outcomes with tool-side recommendation state"
```

### Task 6: Run Focused Verification And Document Limits

**Files:**
- Modify: `docs/plans/2026-04-08-workclaw-tool-platform-optimization-plan.md`
- Modify: `docs/plans/2026-04-08-workclaw-tool-defer-loading-and-selection-design.md`
- Modify: `docs/plans/2026-04-08-workclaw-tool-defer-loading-and-selection-plan.md`

**Step 1: Run Rust fast verification**

Run: `pnpm test:rust-fast`

Expected: PASS

**Step 2: Run runtime compile verification**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`

Expected: PASS

**Step 3: Document verification limits**

Update the plan docs to note whether the local machine still blocks full runtime Rust test execution and which commands were used as reliable proof instead.

**Step 4: Commit**

```bash
git add docs/plans/2026-04-08-workclaw-tool-platform-optimization-plan.md docs/plans/2026-04-08-workclaw-tool-defer-loading-and-selection-design.md docs/plans/2026-04-08-workclaw-tool-defer-loading-and-selection-plan.md
git commit -m "docs(tooling): add deferred loading design and verification notes"
```

---

## Execution Status

Executed on 2026-04-08 in the current workspace.

Completed:

- Task 1: staged tool loading planner model
- Task 2: active tool exposure in runtime setup
- Task 3: one-step expansion to full tool exposure on conservative retry
- Task 4: loading policy and expansion state carried into decision records and route updates
- Task 5: tool-side route explanation summary
- Task 6: verification and documentation update

Verification actually run:

- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`
- `pnpm test:rust-fast`

Known verification limit:

- Full `apps/runtime/src-tauri` Rust test binary execution was not used as the final gate on this machine because previous runs in this environment hit `STATUS_ENTRYPOINT_NOT_FOUND`.
