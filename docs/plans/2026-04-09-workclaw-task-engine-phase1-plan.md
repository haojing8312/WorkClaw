# WorkClaw Task Engine Phase 1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Introduce a new top-level `TaskEngine` and `TaskState` skeleton so WorkClaw starts executing local chat through a task-first runtime boundary without destabilizing the existing session spine.

**Architecture:** Build the new engine above the current `Session Spine` instead of replacing it immediately. Phase 1 adds the correct top-level ownership model, threads task identity through local chat, and keeps actual lane execution delegated to the existing `session_engine` and kernel contracts.

**Tech Stack:** Rust, Tauri, sqlx, serde, runtime-chat-app, Cargo tests, pnpm fast Rust verification

---

## Scope

This plan covers only the first phase of `Task Engine`.

It intentionally does **not**:

- migrate hidden child sessions yet
- migrate employee step execution yet
- replace `TurnStateSnapshot`
- replace `session_journal`
- replace `tool_setup` or `approval_gate`
- introduce swarm or teammate backends

Phase 1 exists to establish the new top-level owner only.

## Files To Touch

Primary files:

- Create: `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/task_state.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Modify: `apps/runtime/src-tauri/src/session_journal.rs`

Likely tests:

- `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`
- `apps/runtime/src-tauri/src/agent/runtime/task_state.rs`
- `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- `apps/runtime/src-tauri/src/session_journal.rs`

## Required Verification

Run at minimum:

- `cargo test --lib task_engine -- --nocapture`
- `cargo test --lib task_state -- --nocapture`
- `cargo test --lib session_runtime -- --nocapture`
- `cargo test --lib session_journal -- --nocapture`
- `pnpm test:rust-fast`

If runtime entry behavior changes more than expected, also run:

- `pnpm test:e2e:runtime`

## Task 1: Define TaskState Contracts

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/task_state.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/mod.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/task_state.rs`

**Goal:** Introduce the first task-level contracts without disturbing existing execution.

**Step 1: Write the failing tests**

Add tests for:

- `TaskKind::PrimaryUserTask`
- `TaskSurfaceKind::LocalChatSurface`
- `TaskState::new_primary_local_chat(...)`
- default parent/root task identity behavior

**Step 2: Run the tests to verify they fail**

Run: `cargo test --lib task_state -- --nocapture`
Expected: FAIL because the module and contracts do not exist yet

**Step 3: Implement minimal contracts**

Add:

- `TaskKind`
- `TaskSurfaceKind`
- `TaskIdentity`
- `TaskState`

Keep `TaskState` intentionally small in Phase 1. Suggested fields:

- `task_id`
- `parent_task_id`
- `root_task_id`
- `task_kind`
- `surface_kind`
- `session_id`
- `user_message_id`
- `run_id`

Do not pull in full continuation, route, or capability state yet.

**Step 4: Re-run the tests**

Run: `cargo test --lib task_state -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/task_state.rs apps/runtime/src-tauri/src/agent/runtime/mod.rs
git commit -m "refactor(runtime): add task state contracts"
```

## Task 2: Add TaskEngine Skeleton

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`

**Goal:** Introduce a top-level engine that can own a primary local chat task while delegating real execution to the current session spine.

**Step 1: Write the failing tests**

Add tests proving:

- `TaskEngine` can construct a primary local chat task state
- `TaskEngine` forwards local chat execution into the existing session engine contract
- task state is attached to the execution path metadata

Keep tests narrow and avoid mocking the entire runtime stack if possible.

**Step 2: Run the tests to verify they fail**

Run: `cargo test --lib task_engine -- --nocapture`
Expected: FAIL because the module and delegation entry do not exist yet

**Step 3: Implement minimal engine**

Create `task_engine.rs` with:

- `TaskEngine`
- `run_primary_local_chat_task(...)`
- a small result wrapper that carries both:
  - `TaskState`
  - delegated `ExecutionOutcome`

Do not add generalized transitions yet.

**Step 4: Re-run the tests**

Run: `cargo test --lib task_engine -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/task_engine.rs apps/runtime/src-tauri/src/agent/runtime/mod.rs apps/runtime/src-tauri/src/agent/runtime/kernel/session_engine.rs
git commit -m "refactor(runtime): add task engine skeleton"
```

## Task 3: Route Local Chat Through TaskEngine

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`

**Goal:** Make local chat enter the runtime through `TaskEngine` first, while preserving existing execution semantics.

**Step 1: Write the failing tests**

Add tests proving:

- local chat still uses the `LocalChat` surface profile behavior
- local chat now creates a `PrimaryUserTask`
- direct dispatch, route execution, and failure outcomes still project correctly

**Step 2: Run the tests to verify they fail**

Run: `cargo test --lib session_runtime -- --nocapture`
Expected: FAIL because session runtime still enters `session_engine` directly

**Step 3: Implement minimal integration**

Refactor local chat entry so the flow becomes:

- `session_runtime`
- `task_engine`
- existing `session_engine`

Keep existing lane behavior unchanged.

**Step 4: Re-run the tests**

Run: `cargo test --lib session_runtime -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs apps/runtime/src-tauri/src/agent/runtime/task_engine.rs
git commit -m "refactor(runtime): route local chat through task engine"
```

## Task 4: Persist Task Identity In Session Journal

**Files:**
- Modify: `apps/runtime/src-tauri/src/session_journal.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`
- Test: `apps/runtime/src-tauri/src/session_journal.rs`

**Goal:** Ensure the journal begins recording task identity so recovery and later phases can grow from a real task model.

**Step 1: Write the failing tests**

Add tests proving that a completed local chat run can persist:

- `task_id`
- `task_kind`
- `surface_kind`

in a backward-compatible way.

**Step 2: Run the tests to verify they fail**

Run: `cargo test --lib session_journal -- --nocapture`
Expected: FAIL because task identity is not yet projected

**Step 3: Implement minimal projection**

Add optional task identity fields to journal state or event projection.

Do not break old session records.

Follow the repo rule:

- if any new SQLite-facing read depends on new shape, add fallback behavior

**Step 4: Re-run the tests**

Run: `cargo test --lib session_journal -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/session_journal.rs apps/runtime/src-tauri/src/agent/runtime/task_engine.rs
git commit -m "refactor(runtime): project task identity into session journal"
```

## Task 5: Run Phase 1 Verification

**Files:**
- No code changes unless a verification failure requires a narrow fix

**Goal:** Prove that the new top-level owner exists and local chat behavior remains stable.

**Step 1: Run focused Rust verification**

Run: `cargo test --lib task_engine -- --nocapture`
Expected: PASS

Run: `cargo test --lib task_state -- --nocapture`
Expected: PASS

Run: `cargo test --lib session_runtime -- --nocapture`
Expected: PASS

Run: `cargo test --lib session_journal -- --nocapture`
Expected: PASS

**Step 2: Run repo fast-path verification**

Run: `pnpm test:rust-fast`
Expected: PASS

**Step 3: Run E2E if runtime entry behavior changed materially**

Run: `pnpm test:e2e:runtime`
Expected: PASS

Only run this if the local chat entry path changed enough that fast Rust tests would be insufficient.

**Step 4: Commit any follow-up fixes**

If verification required narrow fixes:

```bash
git add <affected files>
git commit -m "test(runtime): stabilize task engine phase 1 verification"
```

## Phase 1 Exit Criteria

Phase 1 is complete only when all of these are true:

- local chat enters through `TaskEngine`
- `TaskState` exists and is used for primary local chat
- session runtime behavior remains stable
- journal projection records task identity in a backward-compatible way
- hidden child and employee step execution are unchanged functionally
- all required verification commands pass

## Important Constraints

- Do not move child session execution in this phase
- Do not move employee step execution in this phase
- Do not rewrite continuation semantics in this phase
- Do not widen the engine to generalized backend transitions yet
- Keep the first phase ruthlessly narrow

## Recommended Next Phase

After Phase 1 is stable, the next phase should be:

**HiddenChildBackend adoption**

That is the smallest real proof that `TaskEngine` can own more than one surface without destabilizing the whole runtime.
