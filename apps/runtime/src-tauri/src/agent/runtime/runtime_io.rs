#![allow(unused_imports)]

mod message_reconstruction;
mod runtime_events;
mod runtime_inputs;
mod runtime_support;
mod session_titles;
mod types;
mod workspace_skills;

pub(crate) use message_reconstruction::{
    build_assistant_content_from_final_messages, build_assistant_content_with_stream_fallback,
    reconstruct_history_messages,
};
pub(crate) use runtime_events::{
    append_partial_assistant_chunk_with_pool, append_run_failed_with_pool,
    append_run_guard_warning_with_pool, append_run_started_with_pool, append_run_stopped_with_pool,
    append_skill_route_recorded_with_pool, finalize_run_success_with_pool,
    insert_session_message_with_pool, persist_partial_assistant_message_for_run_with_pool,
    record_route_attempt_log_with_pool,
};
pub(crate) use runtime_inputs::{
    load_default_search_provider_config_with_pool, load_installed_skill_source_with_pool,
    load_session_history_with_pool, load_session_runtime_inputs_with_pool,
};
pub(crate) use runtime_support::{build_memory_dir_for_session, load_memory_content};
pub(crate) use session_titles::{
    derive_meaningful_session_title_from_messages, is_generic_session_title,
    maybe_update_session_title_from_first_user_message_with_pool,
};
pub(crate) use types::WorkspaceSkillPromptEntry;
pub use types::{
    WorkspaceSkillCommandSpec, WorkspaceSkillContent, WorkspaceSkillRouteExecutionMode,
    WorkspaceSkillRouteProjection, WorkspaceSkillRuntimeEntry,
};
pub(crate) use workspace_skills::{
    build_skill_roots, build_workspace_skill_markdown_path, build_workspace_skill_prompt_entries,
    build_workspace_skill_prompt_entry, build_workspace_skills_prompt,
    extract_assistant_text_content, extract_skill_prompt_from_decrypted_files, load_skill_prompt,
    normalize_workspace_skill_dir_name, prepare_workspace_skills_prompt,
    resolve_directory_backed_skill_root, resolve_workspace_skill_runtime_entry,
    sync_workspace_skills_to_directory,
};
pub use workspace_skills::{
    build_workspace_skill_command_specs, load_workspace_skill_runtime_entries_with_pool,
};
