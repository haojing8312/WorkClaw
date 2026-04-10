# 2026-04-09 TaskEngine Terminal Finalization Split Note

## Why
- [task_engine.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/task_engine.rs) has grown past the 801-line governance trigger.
- The next safe step in the Task Engine rollout is delegated terminal finalization, but adding that logic directly into `task_engine.rs` would make the file harder to reason about.

## Current split
- Keep `TaskEngine` as the public orchestration entry for:
  - task identity creation
  - lifecycle begin helpers
  - backend execution helpers
- Move delegated terminal commit/finalization logic into a focused helper module:
  - `task_terminal.rs`

## Immediate scope
- Extract shared delegated terminal outcome handling for hidden child + employee step runtimes.
- Preserve surface-specific fallback behavior in callers.

## Follow-up candidates
- Move more terminal outcome mapping out of delegated runtime adapters.
- Revisit `run_primary_local_chat_task(...)` and task begin helpers as the next split boundary in `task_engine.rs`.
