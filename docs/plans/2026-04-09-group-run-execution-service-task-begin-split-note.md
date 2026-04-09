## Why this note exists

`apps/runtime/src-tauri/src/commands/employee_agents/group_run_execution_service.rs` is above the 800-line governance threshold, but the current task only needs a small runtime-boundary change.

## Change scope

This step only replaces the local `start_task + project_task_state + delegation apply` sequence with a shared `TaskEngine::begin_task_run(...)` helper so employee-step execution enters the task engine through the same path as local chat and hidden child sessions.

## Why this stays here for now

- The behavior is still specific to employee-step execution startup.
- Extracting a full employee-step runtime module split in the same change would create much larger churn than the current task requires.
- The new helper reduces logic in this file rather than adding another bespoke branch.

## Follow-up split direction

If employee-step task lifecycle logic grows again, the next safe split is:

- keep request shaping and group-run entry wiring in `group_run_execution_service.rs`
- move employee-step runtime startup/finalize lifecycle into a sibling module such as `employee_step_runtime.rs`
- let this file call that module as an adapter
