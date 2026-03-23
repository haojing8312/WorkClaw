# Rust Chat Runtime IO Split Design

**Goal:** Turn [chat_runtime_io.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_runtime_io.rs) into the next formal Rust data-plane split after `chat_session_io` and `employee_agents` by extracting workspace skill projection, shared DTOs, and related helper utilities first, while preserving the existing command-facing API.

## Why This Is Next

[chat_runtime_io.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_runtime_io.rs) is currently a 2k+ line shared helper hub. It is already used by several command surfaces:

- [chat.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat.rs)
- [chat_send_message_flow.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs)
- [chat_tool_setup.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_tool_setup.rs)
- [chat_route_execution.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_route_execution.rs)
- [chat_session_io/session_compaction.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_session_io/session_compaction.rs)
- [employee_agents/service.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/employee_agents/service.rs)

That makes it a good next split target, but not a good place to start with deep runtime-flow changes. The safest first step is to peel off the workspace skill projection cluster, which is cohesive, mostly helper-oriented, and already has existing tests.

## Current Responsibility Clusters

This file currently mixes at least six distinct concerns:

1. Session title normalization and first-message title updates
2. Run-event persistence and team-entry pre-execution orchestration
3. Session runtime input loading and skill source loading
4. Workspace skill projection, file-tree syncing, and prompt rendering
5. Memory directory helpers and tool-name resolution
6. Message reconstruction and assistant-content shaping

Those clusters are logically related to chat runtime, but they should not keep growing inside one file.

## Implemented First Batches

The first implemented batches extracted three low-risk helper clusters:

- `types.rs`
  - `WorkspaceSkillPromptEntry`
  - `WorkspaceSkillContent`
  - `WorkspaceSkillRuntimeEntry`
- `workspace_skills.rs`
  - `normalize_workspace_skill_dir_name`
  - `build_workspace_skill_markdown_path`
  - `build_workspace_skill_prompt_entry`
  - `build_workspace_skills_prompt`
  - `resolve_workspace_skill_runtime_entry`
  - `sync_workspace_skills_to_directory`
  - `build_workspace_skill_prompt_entries`
  - `prepare_workspace_skills_prompt`
  - `load_workspace_skill_runtime_entries_with_pool`
  - `extract_skill_prompt_from_decrypted_files`
  - `read_local_skill_prompt`
  - `load_skill_prompt`
  - `build_skill_roots`
- `session_titles.rs`
  - `maybe_update_session_title_from_first_user_message_with_pool`
  - `is_generic_session_title`
  - `normalize_candidate_session_title`
  - `derive_meaningful_session_title_from_messages`
- `runtime_support.rs`
  - `load_memory_content`
  - `resolve_tool_names`
  - `sanitize_memory_bucket_component`
  - `build_memory_dir_for_session`
  - `tool_ctx_from_work_dir`
- `runtime_inputs.rs`
  - `load_session_runtime_inputs_with_pool`
  - `load_installed_skill_source_with_pool`
  - `load_session_history_with_pool`
  - `load_default_search_provider_config_with_pool`
- `message_reconstruction.rs`
  - `reconstruct_llm_messages`
  - `extract_new_messages_after_reconstructed_history`
  - `reconstruct_history_messages`
  - `build_assistant_content_from_final_messages`
  - `build_assistant_content_with_stream_fallback`
- `runtime_events.rs`
  - `insert_session_message_with_pool`
  - `record_route_attempt_log_with_pool`
  - `append_run_started_with_pool`
  - `append_run_failed_with_pool`
  - `append_run_guard_warning_with_pool`
  - `append_run_stopped_with_pool`
  - `append_partial_assistant_chunk_with_pool`
  - `finalize_run_success_with_pool`
  - `maybe_handle_team_entry_pre_execution_with_pool`

The root file should keep the same public `pub(crate)` names through re-exports so existing callers do not need to change.

## Phase 2 And Beyond

At this point the root file is down to a thin compatibility facade only. The module-local tests have already been migrated into the matching child modules. The remaining follow-on work is mostly cleanup:

- review any leftover root re-exports that no longer need to stay visible
- decide whether any of the new child modules should be split again if they start growing too quickly

Those should be handled in later passes, not in the first extraction.

## Risks

- Changing workspace skill path projection can break prompt file locations used by `chat_tool_setup`
- Changing local/builtin/encrypted skill loading can change prompt text or fallback behavior
- Moving file-tree sync logic too aggressively can accidentally alter directory cleanup
- Creating a new giant helper module instead of a focused one

## Success Criteria

- [chat_runtime_io.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/chat_runtime_io.rs) becomes a thinner compatibility facade
- workspace skill DTOs and projection helpers live in focused child modules
- root-level re-exports preserve the visible API for downstream callers
- existing skill-projection tests continue to pass
- the same pattern is reusable for the remaining chat-runtime helper clusters
