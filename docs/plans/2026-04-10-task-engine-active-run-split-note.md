# 2026-04-10 TaskEngine Active Run Split Note

## Why
- [task_engine.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/task_engine.rs) had grown into both the orchestration entry and the active-task execution skeleton.
- The next Task Engine steps need a clearer boundary for `begin -> run_started -> prepare -> execute -> backend-failure` so primary and delegated backends can keep converging without making `task_engine.rs` harder to reason about.

## Current split
- Keep `TaskEngine` as the public orchestration entry for:
  - task identity creation
  - task lifecycle transitions
  - task record persistence helpers
  - top-level run/finalize entrypoints
- Move active-task backend run orchestration into:
  - `task_active_run.rs`

## Immediate scope
- Extract the shared active-task backend skeleton used by local chat and delegated runtimes.
- Preserve `TaskEngine` as the owner of begin/fail/finalize semantics.

## Follow-up candidates
- Continue shrinking `task_engine.rs` by moving more backend-policy-specific glue out of primary/delegated entrypoints.
- Revisit whether `run_and_finalize_*` wrappers should collapse into a thinner policy-driven facade once the backend policy layer stabilizes.
