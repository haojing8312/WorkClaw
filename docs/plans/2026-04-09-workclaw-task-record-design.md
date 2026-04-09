# WorkClaw Task Record Design

**Date:** 2026-04-09

## Goal

Upgrade WorkClaw from "task lineage exists in runtime projections" to "task entities exist as first-class persisted runtime objects."

This design is the next layer above the already-landed `TaskEngine` skeleton:

- [task_state.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/task_state.rs)
- [task_engine.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/task_engine.rs)
- [session_journal.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/session_journal.rs)
- [trace_builder.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/trace_builder.rs)
- [session_runs.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/session_runs.rs)

The immediate objective is not scheduling, task assignment UI, or swarm orchestration. The immediate objective is:

**separate persisted task entities from execution-time task state.**

## Why This Design Exists

WorkClaw now has the beginnings of task-first runtime modeling:

- `TaskIdentity`
- `TaskKind`
- `TaskSurfaceKind`
- task lineage for local chat, hidden child sessions, and employee step sessions
- task read models in session recovery, trace, and export

That work was necessary, but it is still only a foundation.

Right now WorkClaw mostly has:

- runtime task identity
- task path projection
- task graph read helpers

What it does **not** yet have is a real task entity model with lifecycle state.

That means the system can answer:

- "Which task lineage did this run belong to?"

but it still cannot cleanly answer:

- "What task exists right now?"
- "Is it pending, running, completed, failed, or cancelled?"
- "Which runtime execution created it?"
- "What is the durable parent-child task relationship?"
- "What terminal reason ended it?"

Those are the minimum ingredients for a real task system.

## Architectural Lesson From `close-code`

The most useful contrast in `close-code` is not the query loop this time. It is the distinction between two different "task" layers:

### 1. Persisted task objects

In [tasks.ts](/e:/code/yzpd/close-code/src/utils/tasks.ts), `close-code` has a durable task model that includes:

- `id`
- `subject`
- `description`
- `activeForm`
- `owner`
- `status`
- `blocks`
- `blockedBy`
- `metadata`

This layer is explicitly mutable through:

- [TaskCreateTool.ts](/e:/code/yzpd/close-code/src/tools/TaskCreateTool/TaskCreateTool.ts)
- [TaskUpdateTool.ts](/e:/code/yzpd/close-code/src/tools/TaskUpdateTool/TaskUpdateTool.ts)

### 2. Runtime task state

In [tasks/types.ts](/e:/code/yzpd/close-code/src/tasks/types.ts), `close-code` separately models background task execution state for:

- local shell work
- local agent work
- remote agent work
- in-process teammates
- workflows

This distinction is important:

**persisted task objects are not the same thing as runtime execution state.**

WorkClaw currently has only the second half in embryonic form.

## Current WorkClaw Gap

Today [task_state.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/task_state.rs) carries:

- `task_identity`
- `task_kind`
- `surface_kind`
- `session_id`
- `user_message_id`
- `run_id`

This is useful, but it is still an execution-anchor model.

It is not yet a first-class task entity because it does not directly carry:

- lifecycle status
- terminal outcome
- timestamps
- durable record semantics
- repository access patterns

If WorkClaw keeps pushing everything into `TaskState`, it will eventually blur three concerns:

1. persisted task identity
2. runtime execution state
3. read-model projection

That would make later steps harder:

- task continuation policy
- delegated task ownership
- task recovery across surfaces
- teammate/swarm coordination
- verification task creation

## Design Principle

Introduce a new persisted domain object:

**`TaskRecord`**

and keep:

**`TaskState`**

as the runtime execution view.

This produces a clean three-layer model:

1. `TaskRecord`
   durable task entity
2. `TaskState`
   execution-time task view for the current run
3. `SessionRunTaskIdentitySnapshot` / `task_path` / `task_graph`
   read-model projections

## Core Concepts

### `TaskRecord`

The durable task entity for WorkClaw runtime tasks.

Suggested first version:

```rust
pub(crate) struct TaskRecord {
    pub task_identity: TaskIdentity,
    pub task_kind: TaskKind,
    pub surface_kind: TaskSurfaceKind,
    pub session_id: String,
    pub user_message_id: String,
    pub run_id: String,
    pub status: TaskLifecycleStatus,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub terminal_reason: Option<String>,
}
```

This first version is intentionally narrow.

It does **not** yet add:

- owner
- subject
- description
- blocks
- blockedBy
- metadata blobs

Those may become necessary later, but forcing them in now would overfit the first phase to the richer `close-code` task list model before WorkClaw even has a stable task entity lifecycle.

### `TaskLifecycleStatus`

Suggested first version:

- `Pending`
- `Running`
- `Completed`
- `Failed`
- `Cancelled`

This is enough to establish durable lifecycle semantics across local chat, hidden child sessions, and employee step sessions.

### `TaskRepo`

The persistence boundary for `TaskRecord`.

Responsibilities:

- create or upsert task records
- mark task running
- mark task terminal
- query recent task records for a session
- query task records by lineage

The important design rule is:

**TaskEngine talks to TaskRepo.**

It should not directly scatter raw task-entity persistence logic through session runtime and journal files.

## Why The First Version Should Be Journal-Backed

It is tempting to immediately add a SQLite `runtime_tasks` table.

I do not recommend that for phase 1.

### Recommended phase 1

Use a journal-backed repository layered on top of [session_journal.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/session_journal.rs).

Why:

1. WorkClaw already persists task lineage there.
2. Recovery, export, and trace already consume journal projections.
3. This avoids adding a new migration surface before the entity shape stabilizes.
4. It keeps the smallest safe implementation path.

### Recommended phase 2 or later

Once `TaskRecord` semantics stabilize, add a SQLite projection for:

- faster querying
- cross-session task graph analysis
- richer task search and diagnostics

## Event Model

Current event:

- `TaskStateProjected`

This should remain, but it is no longer sufficient by itself.

Suggested additions:

- `TaskRecordUpserted { run_id, task_record }`
- `TaskStatusChanged { run_id, task_id, from, to, reason }`

These events serve different purposes:

- `TaskStateProjected`
  execution-side lineage hook
- `TaskRecordUpserted`
  durable entity snapshot
- `TaskStatusChanged`
  explicit lifecycle transition

This gives WorkClaw a better separation between:

- runtime execution
- durable entity state
- read-model derivation

## File-Level Design

### New files

- `apps/runtime/src-tauri/src/agent/runtime/task_record.rs`
- `apps/runtime/src-tauri/src/agent/runtime/task_repo.rs`

### Existing files to modify

- [task_state.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/task_state.rs)
  only for bridge helpers if needed, not for lifecycle growth
- [task_engine.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/task_engine.rs)
  to create/update `TaskRecord`
- [session_journal.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/session_journal.rs)
  to store task entity events and project minimal record state
- [session_runs.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/session_runs.rs)
  to expose optional `task_status`
- [trace_builder.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/trace_builder.rs)
  to summarize lifecycle transitions
- [session_export.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_session_io/session_export.rs)
  to render task entity summaries when useful

## Runtime Flow After Phase 1

For a primary local chat task:

1. `TaskEngine` builds `TaskState`
2. `TaskEngine` creates `TaskRecord(status=Pending|Running)`
3. journal receives:
   - `TaskStateProjected`
   - `TaskRecordUpserted`
4. execution continues through the existing session spine
5. terminal outcome arrives
6. `TaskEngine` or the task repo bridge writes:
   - `TaskStatusChanged`
   - updated `TaskRecord`
7. read models consume both lineage and lifecycle

The same pattern should apply to:

- hidden child sessions
- employee step sessions

## What This Unlocks Later

Once `TaskRecord` exists, WorkClaw can safely add:

- task continuation policy separate from session continuation
- owner assignment and delegation metadata
- dependency / blocked-by modeling
- verification task generation
- teammate/swarm task nodes
- richer task dashboards and developer diagnostics

Without `TaskRecord`, those capabilities would be built on projections and adapters alone, which is too fragile.

## Risks

### 1. Duplicating state between `TaskState` and `TaskRecord`

Mitigation:

- keep `TaskState` execution-oriented
- keep `TaskRecord` lifecycle-oriented
- do not put transient route/capability fields into `TaskRecord`

### 2. Overloading `session_journal.rs`

That file is already large.

Mitigation:

- add task-entity helpers in `task_repo.rs`
- keep event definitions and minimal projection logic in journal
- avoid embedding complex business logic directly into the giant journal file

### 3. Designing too much too early

Mitigation:

- first phase only adds lifecycle persistence
- postpone owner/dependency/scheduler semantics

## Phase 1 Scope

Phase 1 should do only this:

- add `TaskLifecycleStatus`
- add `TaskRecord`
- add `TaskRepo`
- create/update task records for local chat, hidden child, employee step
- project minimal task status into journal-backed read models

Phase 1 should explicitly **not** do:

- task assignment UI
- dependency editing
- teammate scheduling
- task queueing
- task graph frontend panel
- cross-session global task list UX

## Acceptance Criteria

The design is successful when:

1. every runtime-created task has a durable `TaskRecord`
2. task lineage remains intact across local chat, hidden child, and employee step
3. terminal task lifecycle status is persisted
4. session recovery, trace, and export can read both:
   - lineage
   - lifecycle
5. no visible chat runtime behavior regresses

## Final Recommendation

The next WorkClaw move should be:

**TaskState -> TaskRecord + TaskRepo**

not:

**TaskState -> a larger catch-all runtime struct**

That keeps WorkClaw aligned with the strongest lesson from `close-code`:

**durable task entities and runtime execution state must not collapse into one model.**
