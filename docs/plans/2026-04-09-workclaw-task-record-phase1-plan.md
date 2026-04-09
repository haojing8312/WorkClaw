# WorkClaw Task Record Phase 1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Introduce a first-class `TaskRecord` entity and a journal-backed `TaskRepo` so WorkClaw runtime tasks become durable lifecycle objects instead of lineage-only projections.

**Architecture:** Keep the current `TaskEngine` and `TaskState` as execution-layer contracts. Add a separate persisted `TaskRecord` layer that is written through a small `TaskRepo`, backed by `session_journal` events in phase 1. This avoids immediate schema migration risk while upgrading WorkClaw from task-lineage to real task entities.

**Tech Stack:** Rust, Tauri, serde, chrono, sqlx-backed runtime helpers, session journal, Cargo tests, pnpm fast Rust verification

---

## Scope

This phase adds the minimum viable task entity layer.

It intentionally does **not**:

- add task assignment UI
- add task dependencies or `blockedBy`
- add swarm/teammate scheduling
- add a new SQLite table
- replace `TaskState`
- redesign session export or chat UI

The only objective is to establish a durable task entity lifecycle.

## Files To Touch

Primary files:

- Create: `apps/runtime/src-tauri/src/agent/runtime/task_record.rs`
- Create: `apps/runtime/src-tauri/src/agent/runtime/task_repo.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`
- Modify: `apps/runtime/src-tauri/src/session_journal.rs`
- Modify: `apps/runtime/src-tauri/src/commands/session_runs.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/trace_builder.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_session_io/session_export.rs`

Likely tests:

- `apps/runtime/src-tauri/src/agent/runtime/task_record.rs`
- `apps/runtime/src-tauri/src/agent/runtime/task_repo.rs`
- `apps/runtime/src-tauri/src/session_journal.rs`
- `apps/runtime/src-tauri/src/commands/session_runs.rs`
- `apps/runtime/src-tauri/src/agent/runtime/trace_builder.rs`

## Required Verification

Run at minimum:

- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_record --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_repo --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib session_journal --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib session_runs --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib trace_builder --no-run`
- `pnpm test:rust-fast`

If the local loader issue is resolved on the machine, also run:

- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_record -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_repo -- --nocapture`

## Task 1: Define TaskRecord Contracts

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/task_record.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/mod.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/task_record.rs`

**Goal:** Introduce a durable task entity model that is distinct from `TaskState`.

**Step 1: Write the failing test**

Add tests for:

- `TaskLifecycleStatus` variants
- `TaskRecord::new_pending(...)`
- parent/root lineage preservation from `TaskIdentity`
- terminal update helpers preserving timestamps and terminal reason

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_record --no-run`
Expected: FAIL because the module and contracts do not exist yet

**Step 3: Write minimal implementation**

Create `task_record.rs` with:

- `TaskLifecycleStatus`
- `TaskRecord`
- helper constructors:
  - `new_pending(...)`
  - `mark_running(...)`
  - `mark_completed(...)`
  - `mark_failed(...)`
  - `mark_cancelled(...)`

Keep fields minimal:

- `task_identity`
- `task_kind`
- `surface_kind`
- `session_id`
- `user_message_id`
- `run_id`
- `status`
- `created_at`
- `updated_at`
- `started_at`
- `completed_at`
- `terminal_reason`

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_record --no-run`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/task_record.rs apps/runtime/src-tauri/src/agent/runtime/mod.rs
git commit -m "refactor(runtime): add task record contracts"
```

## Task 2: Add TaskRepo Skeleton

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/task_repo.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/mod.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/task_repo.rs`

**Goal:** Create a narrow persistence boundary for task entities without exposing journal internals everywhere.

**Step 1: Write the failing test**

Add tests proving:

- `TaskRepo` can build a `TaskRecordUpserted` event payload
- `TaskRepo` can build a `TaskStatusChanged` payload
- `TaskRepo` preserves `task_id`, `parent_task_id`, and terminal reason when projecting events

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_repo --no-run`
Expected: FAIL because the module does not exist yet

**Step 3: Write minimal implementation**

Create `task_repo.rs` with:

- `TaskRepo`
- write helpers:
  - `upsert_task(...)`
  - `mark_running(...)`
  - `mark_terminal(...)`
- event helper functions so business logic stays out of `session_journal.rs`

Phase 1 repo implementation should be journal-backed, not table-backed.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_repo --no-run`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/task_repo.rs apps/runtime/src-tauri/src/agent/runtime/mod.rs
git commit -m "refactor(runtime): add task repo skeleton"
```

## Task 3: Extend Session Journal With Task Entity Events

**Files:**
- Modify: `apps/runtime/src-tauri/src/session_journal.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/task_repo.rs`
- Test: `apps/runtime/src-tauri/src/session_journal.rs`

**Goal:** Teach the journal to persist task entity lifecycle events and project minimal task entity state.

**Step 1: Write the failing test**

Add tests proving:

- `TaskRecordUpserted` projects a task entity into session state
- `TaskStatusChanged` updates the projected lifecycle status
- legacy sessions without task records still deserialize correctly

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib session_journal --no-run`
Expected: FAIL because the events and projection do not exist yet

**Step 3: Write minimal implementation**

Add:

- `SessionRunTaskRecordSnapshot`
- `TaskRecordUpserted`
- `TaskStatusChanged`

Project them into a minimal `tasks` collection in `SessionJournalState`.

Keep all projection logic small and push event-construction logic back into `TaskRepo`.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib session_journal --no-run`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/session_journal.rs apps/runtime/src-tauri/src/agent/runtime/task_repo.rs
git commit -m "refactor(runtime): persist task records in session journal"
```

## Task 4: Write TaskRecord Through TaskEngine

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/task_repo.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/task_engine.rs`

**Goal:** Ensure every runtime-created task produces a durable entity record when it starts and updates when it finishes.

**Step 1: Write the failing test**

Add tests proving:

- local chat task creation causes task-record upsert
- hidden child task creation causes task-record upsert with inherited lineage
- employee step task creation causes task-record upsert with inherited lineage
- terminal outcomes map to `Completed`, `Failed`, or `Cancelled`

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_engine --no-run`
Expected: FAIL because task repo writes are not wired into the engine yet

**Step 3: Write minimal implementation**

Update `TaskEngine` so it:

- creates `TaskRecord::new_pending(...)`
- marks it running before or when execution starts
- marks it terminal after outcome translation

Do not yet derive richer owner or dependency semantics.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_engine --no-run`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/task_engine.rs apps/runtime/src-tauri/src/agent/runtime/task_repo.rs
git commit -m "refactor(runtime): persist task lifecycle through task engine"
```

## Task 5: Expose Minimal Task Lifecycle Read Model

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/session_runs.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/trace_builder.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_session_io/session_export.rs`
- Test: `apps/runtime/src-tauri/src/commands/session_runs.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/trace_builder.rs`

**Goal:** Surface task lifecycle status alongside the lineage read model that already exists.

**Step 1: Write the failing test**

Add tests proving:

- session runs can read a projected task status
- trace output can summarize task lifecycle transitions
- session export can show minimal task status lines without requiring a UI panel

**Step 2: Run test to verify it fails**

Run:

- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib session_runs --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib trace_builder --no-run`

Expected: FAIL because lifecycle read fields are not projected yet

**Step 3: Write minimal implementation**

Expose:

- `task_status`
- optional terminal reason summary

Do not build a new frontend panel yet. Keep this phase read-only and diagnostic.

**Step 4: Run test to verify it passes**

Run:

- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib session_runs --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib trace_builder --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib chat_session_io --no-run`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/session_runs.rs apps/runtime/src-tauri/src/agent/runtime/trace_builder.rs apps/runtime/src-tauri/src/commands/chat_session_io/session_export.rs
git commit -m "refactor(runtime): expose task lifecycle read model"
```

## Final Verification

Run:

- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_record --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_repo --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib task_engine --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib session_journal --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib session_runs --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib trace_builder --no-run`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib chat_session_io --no-run`
- `pnpm test:rust-fast`

If the machine-specific loader issue is resolved, rerun the focused lib tests without `--no-run`.

## Acceptance Criteria

Phase 1 is complete when:

- every primary, hidden child, and employee task produces a durable `TaskRecord`
- task lineage remains stable
- lifecycle state is persisted and readable
- terminal reasons can be surfaced to recovery/export/trace
- there is no user-visible regression in chat execution behavior

## Out Of Scope Follow-On Work

Do next, not now:

- task owner and assignment semantics
- dependency edges like `blockedBy`
- task scheduling / claiming
- teammate/swarm task graph execution
- task UI panel

