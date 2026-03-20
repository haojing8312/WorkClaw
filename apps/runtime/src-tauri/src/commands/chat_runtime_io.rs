use super::employee_agents::maybe_handle_team_entry_session_message_with_pool;
use super::session_runs::{
    append_session_run_event_with_pool, attach_assistant_message_to_run_with_pool,
};
use crate::agent::run_guard::RunStopReason;
use crate::agent::AgentExecutor;
use crate::session_journal::{SessionJournalStore, SessionRunEvent};
use chrono::Utc;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

pub(crate) async fn insert_session_message_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    role: &str,
    content: &str,
    content_json: Option<&str>,
) -> Result<String, String> {
    let msg_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, content_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&msg_id)
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(content_json)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(msg_id)
}

pub(crate) async fn maybe_update_session_title_from_first_user_message_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    user_message: &str,
) -> Result<(), String> {
    let msg_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM messages WHERE session_id = ?")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    if msg_count.0 <= 1 {
        let Some(title) = normalize_candidate_session_title(user_message) else {
            return Ok(());
        };
        sqlx::query(
            "UPDATE sessions SET title = ? WHERE id = ? AND (title = 'New Chat' OR title = '')",
        )
        .bind(&title)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

const DEFAULT_SESSION_TITLE: &str = "New Chat";
const MAX_SESSION_TITLE_CHARS: usize = 28;
const GENERIC_SESSION_TITLE_INPUTS: &[&str] = &[
    "",
    "hi",
    "hello",
    "hey",
    "start",
    "continue",
    "continueprevious",
    "continuefrombefore",
    "helpme",
    "needhelp",
    "你好",
    "您好",
    "在吗",
    "继续",
    "开始",
    "帮我一下",
    "帮我处理",
    "请帮我一下",
    "继续上次",
    "继续刚才",
];

fn canonicalize_session_title_match(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn trim_title_punctuation(value: &str) -> &str {
    value.trim_matches(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                ',' | '.'
                    | ':'
                    | ';'
                    | '!'
                    | '?'
                    | '-'
                    | '，'
                    | '。'
                    | '：'
                    | '；'
                    | '！'
                    | '？'
                    | '、'
                    | '…'
                    | '·'
                    | '|'
                    | '/'
                    | '\\'
                    | '"'
                    | '\''
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
            )
    })
}

pub(crate) fn is_generic_session_title(value: &str) -> bool {
    let normalized = canonicalize_session_title_match(value);
    normalized.is_empty()
        || normalized == canonicalize_session_title_match(DEFAULT_SESSION_TITLE)
        || GENERIC_SESSION_TITLE_INPUTS
            .iter()
            .any(|candidate| normalized == canonicalize_session_title_match(candidate))
}

pub(crate) fn normalize_candidate_session_title(value: &str) -> Option<String> {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = trim_title_punctuation(&collapsed);
    if trimmed.is_empty() || is_generic_session_title(trimmed) {
        return None;
    }
    let title: String = trimmed.chars().take(MAX_SESSION_TITLE_CHARS).collect();
    let title = trim_title_punctuation(&title).trim().to_string();
    if title.is_empty() || is_generic_session_title(&title) {
        None
    } else {
        Some(title)
    }
}

pub(crate) fn derive_meaningful_session_title_from_messages<'a, I>(messages: I) -> Option<String>
where
    I: IntoIterator<Item = &'a str>,
{
    messages
        .into_iter()
        .find_map(normalize_candidate_session_title)
}

pub(crate) async fn record_route_attempt_log_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    capability: &str,
    api_format: &str,
    model_name: &str,
    attempt_index: usize,
    retry_index: usize,
    error_kind: &str,
    success: bool,
    error_message: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO route_attempt_logs (id, session_id, capability, api_format, model_name, attempt_index, retry_index, error_kind, success, error_message, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(capability)
    .bind(api_format)
    .bind(model_name)
    .bind(attempt_index as i64)
    .bind(retry_index as i64)
    .bind(error_kind)
    .bind(success)
    .bind(error_message)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await;
}

pub(crate) async fn append_run_started_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    user_message_id: &str,
) -> Result<(), String> {
    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunStarted {
            run_id: run_id.to_string(),
            user_message_id: user_message_id.to_string(),
        },
    )
    .await
}

pub(crate) async fn append_run_failed_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    error_kind: &str,
    error_message: &str,
) {
    let _ = append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunFailed {
            run_id: run_id.to_string(),
            error_kind: error_kind.to_string(),
            error_message: error_message.to_string(),
        },
    )
    .await;
}

#[allow(dead_code)]
pub(crate) async fn append_run_guard_warning_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    warning_kind: &str,
    title: &str,
    message: &str,
    detail: Option<&str>,
    last_completed_step: Option<&str>,
) -> Result<(), String> {
    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunGuardWarning {
            run_id: run_id.to_string(),
            warning_kind: warning_kind.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            detail: detail.map(str::to_string),
            last_completed_step: last_completed_step.map(str::to_string),
        },
    )
    .await
}

pub(crate) async fn append_run_stopped_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    stop_reason: &RunStopReason,
) -> Result<(), String> {
    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunStopped {
            run_id: run_id.to_string(),
            stop_reason: stop_reason.clone(),
        },
    )
    .await
}

pub(crate) async fn append_partial_assistant_chunk_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    chunk: &str,
) {
    let _ = append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::AssistantChunkAppended {
            run_id: run_id.to_string(),
            chunk: chunk.to_string(),
        },
    )
    .await;
}

pub(crate) async fn finalize_run_success_with_pool(
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    run_id: &str,
    final_text: &str,
    has_tool_calls: bool,
    content: &str,
    reasoning_text: &str,
    reasoning_duration_ms: Option<u64>,
) -> Result<(), String> {
    if !final_text.is_empty() {
        append_session_run_event_with_pool(
            pool,
            journal,
            session_id,
            SessionRunEvent::AssistantChunkAppended {
                run_id: run_id.to_string(),
                chunk: final_text.to_string(),
            },
        )
        .await?;
    }

    if !final_text.is_empty() || has_tool_calls {
        let persisted_content = attach_reasoning_to_content(
            content,
            final_text,
            has_tool_calls,
            reasoning_text,
            reasoning_duration_ms,
        );
        let msg_id = insert_session_message_with_pool(
            pool,
            session_id,
            "assistant",
            &persisted_content,
            None,
        )
        .await?;
        attach_assistant_message_to_run_with_pool(pool, run_id, &msg_id).await?;
    }

    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunCompleted {
            run_id: run_id.to_string(),
        },
    )
    .await?;

    Ok(())
}

fn attach_reasoning_to_content(
    content: &str,
    final_text: &str,
    has_tool_calls: bool,
    reasoning_text: &str,
    reasoning_duration_ms: Option<u64>,
) -> String {
    if reasoning_text.trim().is_empty() {
        return content.to_string();
    }

    let base = if has_tool_calls {
        serde_json::from_str::<Value>(content).unwrap_or_else(|_| {
            json!({
                "text": final_text,
                "items": [],
            })
        })
    } else {
        json!({
            "text": final_text,
        })
    };

    let mut obj = base.as_object().cloned().unwrap_or_default();
    obj.insert(
        "reasoning".to_string(),
        json!({
            "status": "completed",
            "duration_ms": reasoning_duration_ms,
            "content": reasoning_text,
        }),
    );
    serde_json::to_string(&Value::Object(obj)).unwrap_or_else(|_| content.to_string())
}

pub(crate) async fn maybe_handle_team_entry_pre_execution_with_pool(
    app: &AppHandle,
    pool: &sqlx::SqlitePool,
    journal: &SessionJournalStore,
    session_id: &str,
    user_message_id: &str,
    user_message: &str,
) -> Result<bool, String> {
    let Some(group_run) =
        maybe_handle_team_entry_session_message_with_pool(pool, session_id, user_message).await?
    else {
        return Ok(false);
    };

    let run_id = Uuid::new_v4().to_string();
    append_run_started_with_pool(pool, journal, session_id, &run_id, user_message_id).await?;

    if !group_run.final_report.is_empty() {
        append_session_run_event_with_pool(
            pool,
            journal,
            session_id,
            SessionRunEvent::AssistantChunkAppended {
                run_id: run_id.clone(),
                chunk: group_run.final_report.clone(),
            },
        )
        .await?;

        let assistant_msg_id = insert_session_message_with_pool(
            pool,
            session_id,
            "assistant",
            &group_run.final_report,
            None,
        )
        .await?;
        attach_assistant_message_to_run_with_pool(pool, &run_id, &assistant_msg_id).await?;
    }

    append_session_run_event_with_pool(
        pool,
        journal,
        session_id,
        SessionRunEvent::RunCompleted { run_id },
    )
    .await?;

    let _ = app.emit(
        "stream-token",
        super::chat::StreamToken {
            session_id: session_id.to_string(),
            token: group_run.final_report.clone(),
            done: false,
            sub_agent: false,
        },
    );
    let _ = app.emit(
        "stream-token",
        super::chat::StreamToken {
            session_id: session_id.to_string(),
            token: String::new(),
            done: true,
            sub_agent: false,
        },
    );

    Ok(true)
}

pub(crate) async fn load_session_runtime_inputs_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<(String, String, String, String, String), String> {
    sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT skill_id, model_id, permission_mode, COALESCE(work_dir, ''), COALESCE(employee_id, '') FROM sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("会话不存在 (session_id={session_id}): {e}"))
}

pub(crate) async fn load_installed_skill_source_with_pool(
    pool: &sqlx::SqlitePool,
    skill_id: &str,
) -> Result<(String, String, String, String), String> {
    sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT manifest, username, pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?",
    )
    .bind(skill_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Skill 不存在 (skill_id={skill_id}): {e}"))
}

pub(crate) async fn load_session_history_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<Vec<(String, String, Option<String>)>, String> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT role, content, content_json FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|(role, content, content_json)| {
            if role == "assistant" {
                (role, extract_assistant_text_content(&content), None)
            } else {
                (role, content, content_json)
            }
        })
        .collect())
}

pub(crate) async fn load_default_search_provider_config_with_pool(
    pool: &sqlx::SqlitePool,
) -> Result<Option<(String, String, String, String)>, String> {
    sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT api_format, base_url, api_key, model_name FROM model_configs WHERE api_format LIKE 'search_%' AND is_default = 1 LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())
}

pub(crate) fn extract_skill_prompt_from_decrypted_files(
    files: &std::collections::HashMap<String, Vec<u8>>,
) -> Option<String> {
    for key in ["SKILL.md", "skill.md"] {
        if let Some(bytes) = files.get(key) {
            return Some(String::from_utf8_lossy(bytes).to_string());
        }
    }

    let candidate = files
        .iter()
        .find(|(path, _)| path.eq_ignore_ascii_case("SKILL.md"))
        .or_else(|| {
            files.iter().find(|(path, _)| {
                path.rsplit('/')
                    .next()
                    .map(|name| name.eq_ignore_ascii_case("skill.md"))
                    .unwrap_or(false)
            })
        });

    candidate.map(|(_, bytes)| String::from_utf8_lossy(bytes).to_string())
}

pub(crate) fn read_local_skill_prompt(pack_path: &str) -> Option<String> {
    let base = std::path::Path::new(pack_path);

    for file_name in ["SKILL.md", "skill.md"] {
        let candidate = base.join(file_name);
        if let Ok(content) = std::fs::read_to_string(&candidate) {
            return Some(content);
        }
    }

    let entries = std::fs::read_dir(base).ok()?;
    for entry in entries.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case("skill.md")
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                return Some(content);
            }
        }
    }

    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceSkillPromptEntry {
    pub skill_id: String,
    pub invoke_name: String,
    pub name: String,
    pub description: String,
    pub skill_md_path: String,
}

#[derive(Debug, Clone)]
pub(crate) enum WorkspaceSkillContent {
    LocalDir(std::path::PathBuf),
    FileTree(std::collections::HashMap<String, Vec<u8>>),
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceSkillRuntimeEntry {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub source_type: String,
    pub projected_dir_name: String,
    pub content: WorkspaceSkillContent,
}

pub(crate) fn normalize_workspace_skill_dir_name(skill_id: &str) -> String {
    let mut out = String::new();
    let mut last_sep = false;
    for ch in skill_id.trim().chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            out.push(normalized);
            last_sep = false;
        } else if matches!(normalized, '-' | '_') {
            out.push(normalized);
            last_sep = false;
        } else if !last_sep {
            out.push('-');
            last_sep = true;
        }
    }
    let trimmed = out.trim_matches(['-', '_']).to_string();
    if trimmed.is_empty() {
        "skill".to_string()
    } else {
        trimmed
    }
}

pub(crate) fn build_workspace_skill_markdown_path(
    work_dir: &std::path::Path,
    skill_id: &str,
) -> std::path::PathBuf {
    work_dir
        .join("skills")
        .join(normalize_workspace_skill_dir_name(skill_id))
        .join("SKILL.md")
}

pub(crate) fn build_workspace_skill_prompt_entry(entry: &WorkspaceSkillPromptEntry) -> String {
    format!(
        "<skill>\n<name>{}</name>\n<invoke_name>{}</invoke_name>\n<description>{}</description>\n<location>{}</location>\n</skill>",
        entry.name.trim(),
        entry.invoke_name.trim(),
        entry.description.trim(),
        entry.skill_md_path.trim()
    )
}

pub(crate) fn build_workspace_skills_prompt(entries: &[WorkspaceSkillPromptEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut blocks = Vec::with_capacity(entries.len() + 2);
    blocks.push("<available_skills>".to_string());
    blocks.extend(entries.iter().map(build_workspace_skill_prompt_entry));
    blocks.push("</available_skills>".to_string());
    blocks.join("\n")
}

pub(crate) fn resolve_workspace_skill_runtime_entry(
    skill_id: &str,
    manifest_json: &str,
    username: &str,
    pack_path: &str,
    source_type: &str,
) -> Result<WorkspaceSkillRuntimeEntry, String> {
    let manifest: skillpack_rs::SkillManifest =
        serde_json::from_str(manifest_json).map_err(|e| e.to_string())?;
    let projected_dir_name = normalize_workspace_skill_dir_name(skill_id);
    let content = match source_type {
        "local" => WorkspaceSkillContent::LocalDir(std::path::PathBuf::from(pack_path)),
        "builtin" => {
            let markdown = crate::builtin_skills::builtin_skill_markdown(skill_id)
                .unwrap_or(crate::builtin_skills::builtin_general_skill_markdown());
            let mut files = std::collections::HashMap::new();
            files.insert("SKILL.md".to_string(), markdown.as_bytes().to_vec());
            WorkspaceSkillContent::FileTree(files)
        }
        _ => {
            let unpacked = skillpack_rs::verify_and_unpack(pack_path, username)
                .map_err(|e| format!("解包 Skill 失败: {}", e))?;
            WorkspaceSkillContent::FileTree(unpacked.files)
        }
    };

    Ok(WorkspaceSkillRuntimeEntry {
        skill_id: skill_id.to_string(),
        name: manifest.name,
        description: manifest.description,
        source_type: source_type.to_string(),
        projected_dir_name,
        content,
    })
}

fn validate_relative_skill_file_path(path: &str) -> Result<std::path::PathBuf, String> {
    let candidate = std::path::PathBuf::from(path);
    if candidate.is_absolute() {
        return Err(format!("Skill 文件路径必须是相对路径: {}", path));
    }
    for component in candidate.components() {
        match component {
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(format!("Skill 文件路径不安全: {}", path));
            }
            _ => {}
        }
    }
    Ok(candidate)
}

fn copy_local_skill_dir_recursive(
    source_dir: &std::path::Path,
    dest_dir: &std::path::Path,
) -> Result<(), String> {
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    for entry in walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".git")
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        let rel = path.strip_prefix(source_dir).map_err(|e| e.to_string())?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dest_dir.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(path, &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn write_skill_file_tree(
    dest_dir: &std::path::Path,
    files: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    for (rel_path, bytes) in files {
        let safe_rel = validate_relative_skill_file_path(rel_path)?;
        let target = dest_dir.join(safe_rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(target, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn sync_workspace_skills_to_directory(
    work_dir: &std::path::Path,
    entries: &[WorkspaceSkillRuntimeEntry],
) -> Result<(), String> {
    let skills_root = work_dir.join("skills");
    if skills_root.exists() {
        std::fs::remove_dir_all(&skills_root).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&skills_root).map_err(|e| e.to_string())?;

    for entry in entries {
        let dest_dir = skills_root.join(&entry.projected_dir_name);
        match &entry.content {
            WorkspaceSkillContent::LocalDir(source_dir) => {
                copy_local_skill_dir_recursive(source_dir, &dest_dir)?
            }
            WorkspaceSkillContent::FileTree(files) => write_skill_file_tree(&dest_dir, files)?,
        }
    }

    Ok(())
}

pub(crate) fn build_workspace_skill_prompt_entries(
    work_dir: &std::path::Path,
    entries: &[WorkspaceSkillRuntimeEntry],
) -> Vec<WorkspaceSkillPromptEntry> {
    entries
        .iter()
        .map(|entry| WorkspaceSkillPromptEntry {
            skill_id: entry.skill_id.clone(),
            invoke_name: entry.skill_id.clone(),
            name: entry.name.clone(),
            description: entry.description.clone(),
            skill_md_path: build_workspace_skill_markdown_path(work_dir, &entry.skill_id)
                .to_string_lossy()
                .to_string(),
        })
        .collect()
}

pub(crate) fn prepare_workspace_skills_prompt(
    work_dir: &std::path::Path,
    entries: &[WorkspaceSkillRuntimeEntry],
) -> Result<String, String> {
    sync_workspace_skills_to_directory(work_dir, entries)?;
    let prompt_entries = build_workspace_skill_prompt_entries(work_dir, entries);
    Ok(build_workspace_skills_prompt(&prompt_entries))
}

pub(crate) async fn load_workspace_skill_runtime_entries_with_pool(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<WorkspaceSkillRuntimeEntry>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, manifest, username, pack_path, COALESCE(source_type, 'encrypted')
         FROM installed_skills
         ORDER BY installed_at ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    for (skill_id, manifest_json, username, pack_path, source_type) in rows {
        match resolve_workspace_skill_runtime_entry(
            &skill_id,
            &manifest_json,
            &username,
            &pack_path,
            &source_type,
        ) {
            Ok(entry) => entries.push(entry),
            Err(err) => {
                eprintln!(
                    "[skills] 跳过无法投影的 skill {} (source_type={}): {}",
                    skill_id, source_type, err
                );
            }
        }
    }

    Ok(entries)
}

pub(crate) fn extract_assistant_text_content(content: &str) -> String {
    let Ok(parsed) = serde_json::from_str::<Value>(content) else {
        return content.to_string();
    };

    parsed
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| content.to_string())
}

pub(crate) fn load_skill_prompt(
    skill_id: &str,
    manifest_json: &str,
    username: &str,
    pack_path: &str,
    source_type: &str,
) -> Result<String, String> {
    let raw_prompt = if source_type == "builtin" {
        crate::builtin_skills::builtin_skill_markdown(skill_id)
            .unwrap_or(crate::builtin_skills::builtin_general_skill_markdown())
            .to_string()
    } else if source_type == "local" {
        read_local_skill_prompt(pack_path).unwrap_or_else(|| {
            serde_json::from_str::<skillpack_rs::SkillManifest>(manifest_json)
                .map(|m| m.description)
                .unwrap_or_default()
        })
    } else {
        match skillpack_rs::verify_and_unpack(pack_path, username) {
            Ok(unpacked) => extract_skill_prompt_from_decrypted_files(&unpacked.files)
                .unwrap_or_else(|| {
                    serde_json::from_str::<skillpack_rs::SkillManifest>(manifest_json)
                        .map(|m| m.description)
                        .unwrap_or_default()
                }),
            Err(_) => {
                let manifest: skillpack_rs::SkillManifest =
                    serde_json::from_str(manifest_json).map_err(|e| e.to_string())?;
                manifest.description
            }
        }
    };

    Ok(crate::builtin_skills::apply_builtin_todowrite_governance(
        skill_id,
        source_type,
        &raw_prompt,
    ))
}

pub(crate) fn build_skill_roots(
    effective_work_dir: &str,
    source_type: &str,
    pack_path: &str,
) -> Vec<std::path::PathBuf> {
    let mut skill_roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(wd) = tool_ctx_from_work_dir(effective_work_dir) {
        skill_roots.push(wd.join(".claude").join("skills"));
        skill_roots.push(wd.join("skills"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        skill_roots.push(cwd.join(".claude").join("skills"));
    }
    if source_type == "local" {
        let skill_path = std::path::Path::new(pack_path);
        if let Some(parent) = skill_path.parent() {
            skill_roots.push(parent.to_path_buf());
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        skill_roots.push(
            std::path::PathBuf::from(profile)
                .join(".claude")
                .join("skills"),
        );
    }
    skill_roots.sort();
    skill_roots.dedup();
    skill_roots
}

pub(crate) fn load_memory_content(memory_dir: &std::path::Path) -> String {
    let memory_file = memory_dir.join("MEMORY.md");
    if memory_file.exists() {
        std::fs::read_to_string(memory_file).unwrap_or_default()
    } else {
        String::new()
    }
}

pub(crate) fn resolve_tool_names(
    allowed_tools: &Option<Vec<String>>,
    agent_executor: &AgentExecutor,
) -> String {
    match allowed_tools {
        Some(whitelist) => whitelist.join(", "),
        None => agent_executor
            .registry()
            .get_tool_definitions()
            .iter()
            .filter_map(|t| t["name"].as_str().map(String::from))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

pub(crate) fn sanitize_memory_bucket_component(raw: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for ch in raw.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_sep = false;
            continue;
        }
        if !prev_sep {
            out.push('_');
            prev_sep = true;
        }
    }
    let normalized = out.trim_matches('_').to_string();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

pub(crate) fn build_memory_dir_for_session(
    app_data_dir: &std::path::Path,
    skill_id: &str,
    employee_id: &str,
) -> std::path::PathBuf {
    let root = app_data_dir.join("memory");
    if employee_id.trim().is_empty() {
        return root.join(skill_id);
    }
    let employee_bucket = sanitize_memory_bucket_component(employee_id, "employee");
    root.join("employees")
        .join(employee_bucket)
        .join("skills")
        .join(skill_id)
}

pub(crate) fn tool_ctx_from_work_dir(work_dir: &str) -> Option<std::path::PathBuf> {
    if work_dir.trim().is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(work_dir))
    }
}

pub(crate) fn reconstruct_llm_messages(parsed: &Value, api_format: &str) -> Vec<Value> {
    let final_text = parsed["text"].as_str().unwrap_or("");
    let items = match parsed["items"].as_array() {
        Some(arr) => arr,
        None => return vec![json!({"role": "assistant", "content": final_text})],
    };

    let mut result = Vec::new();
    let mut tool_calls: Vec<(&Value, Option<&str>)> = Vec::new();
    let mut companion_texts: Vec<String> = Vec::new();

    for item in items {
        match item["type"].as_str() {
            Some("text") => {
                let text = item["content"].as_str().unwrap_or("");
                if !text.is_empty() {
                    companion_texts.push(text.to_string());
                }
            }
            Some("tool_call") => {
                let tc = if item.get("toolCall").is_some() {
                    &item["toolCall"]
                } else {
                    item
                };
                let output = tc["output"].as_str();
                tool_calls.push((tc, output));
            }
            _ => {}
        }
    }

    if !tool_calls.is_empty() {
        if api_format == "anthropic" {
            let mut content_blocks: Vec<Value> = Vec::new();
            for text in &companion_texts {
                content_blocks.push(json!({"type": "text", "text": text}));
            }
            for (tc, _) in &tool_calls {
                content_blocks.push(json!({
                    "type": "tool_use",
                    "id": tc["id"],
                    "name": tc["name"],
                    "input": tc["input"],
                }));
            }
            result.push(json!({"role": "assistant", "content": content_blocks}));

            let tool_results: Vec<Value> = tool_calls
                .iter()
                .map(|(tc, output)| {
                    json!({
                        "type": "tool_result",
                        "tool_use_id": tc["id"],
                        "content": output.unwrap_or("[已执行]"),
                    })
                })
                .collect();
            result.push(json!({"role": "user", "content": tool_results}));
        } else {
            let companion = companion_texts.join("\n");
            let content_val = if companion.is_empty() {
                Value::Null
            } else {
                Value::String(companion)
            };
            let tc_arr: Vec<Value> = tool_calls
                .iter()
                .map(|(tc, _)| {
                    json!({
                        "id": tc["id"],
                        "type": "function",
                        "function": {
                            "name": tc["name"],
                            "arguments": serde_json::to_string(&tc["input"]).unwrap_or_default(),
                        }
                    })
                })
                .collect();
            result.push(json!({"role": "assistant", "content": content_val, "tool_calls": tc_arr}));

            for (tc, output) in &tool_calls {
                result.push(json!({
                    "role": "tool",
                    "tool_call_id": tc["id"],
                    "content": output.unwrap_or("[已执行]"),
                }));
            }
        }
    }

    if !final_text.is_empty() {
        result.push(json!({"role": "assistant", "content": final_text}));
    }

    if result.is_empty() {
        result.push(json!({"role": "assistant", "content": ""}));
    }

    result
}

#[cfg(test)]
mod workspace_skill_projection_tests {
    use super::{
        build_skill_roots, build_workspace_skill_markdown_path,
        build_workspace_skill_prompt_entries, build_workspace_skill_prompt_entry,
        build_workspace_skills_prompt, normalize_workspace_skill_dir_name,
        prepare_workspace_skills_prompt, resolve_workspace_skill_runtime_entry,
        sync_workspace_skills_to_directory, WorkspaceSkillContent, WorkspaceSkillPromptEntry,
    };
    use chrono::Utc;
    use skillpack_rs::{pack, PackConfig, SkillManifest};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn normalize_workspace_skill_dir_name_uses_skill_id_and_sanitizes() {
        assert_eq!(
            normalize_workspace_skill_dir_name(" Local Skill/Auto Redbook "),
            "local-skill-auto-redbook"
        );
        assert_eq!(
            normalize_workspace_skill_dir_name("builtin.general"),
            "builtin-general"
        );
        assert_eq!(normalize_workspace_skill_dir_name("___"), "skill");
    }

    #[test]
    fn build_workspace_skill_markdown_path_uses_projected_skill_dir() {
        let path = build_workspace_skill_markdown_path(
            Path::new("E:\\workspace\\session-a"),
            "Local Skill/Auto Redbook",
        );
        assert_eq!(
            path,
            Path::new("E:\\workspace\\session-a")
                .join("skills")
                .join("local-skill-auto-redbook")
                .join("SKILL.md")
        );
    }

    #[test]
    fn build_skill_roots_include_projected_workspace_skills_directory() {
        let work_dir = Path::new("E:\\workspace\\session-a");
        let roots = build_skill_roots(&work_dir.to_string_lossy(), "builtin", "");

        assert!(roots.contains(&work_dir.join(".claude").join("skills")));
        assert!(roots.contains(&work_dir.join("skills")));
    }

    #[test]
    fn build_workspace_skill_prompt_entry_includes_location() {
        let entry = WorkspaceSkillPromptEntry {
            skill_id: "local-auto-redbook".to_string(),
            invoke_name: "local-auto-redbook".to_string(),
            name: "xhs-note-creator".to_string(),
            description: "Create Xiaohongshu content".to_string(),
            skill_md_path: "E:\\workspace\\skills\\local-auto-redbook\\SKILL.md".to_string(),
        };

        let prompt = build_workspace_skill_prompt_entry(&entry);
        assert!(prompt.contains("<name>xhs-note-creator</name>"));
        assert!(prompt.contains("<invoke_name>local-auto-redbook</invoke_name>"));
        assert!(prompt.contains("<description>Create Xiaohongshu content</description>"));
        assert!(prompt
            .contains("<location>E:\\workspace\\skills\\local-auto-redbook\\SKILL.md</location>"));
    }

    #[test]
    fn build_workspace_skills_prompt_wraps_available_skills_block() {
        let prompt = build_workspace_skills_prompt(&[WorkspaceSkillPromptEntry {
            skill_id: "builtin-general".to_string(),
            invoke_name: "builtin-general".to_string(),
            name: "General Assistant".to_string(),
            description: "Generic work".to_string(),
            skill_md_path: "E:\\workspace\\skills\\builtin-general\\SKILL.md".to_string(),
        }]);

        assert!(prompt.starts_with("<available_skills>"));
        assert!(prompt
            .contains("<location>E:\\workspace\\skills\\builtin-general\\SKILL.md</location>"));
        assert!(prompt.ends_with("</available_skills>"));
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_for_local_skill_uses_local_dir() {
        let manifest = SkillManifest {
            id: "local-auto-redbook".to_string(),
            name: "xhs-note-creator".to_string(),
            description: "Create Xiaohongshu notes".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "local-auto-redbook",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            "E:\\skills\\auto-redbook",
            "local",
        )
        .unwrap();

        assert_eq!(entry.projected_dir_name, "local-auto-redbook");
        match entry.content {
            WorkspaceSkillContent::LocalDir(path) => {
                assert_eq!(path, Path::new("E:\\skills\\auto-redbook"));
            }
            WorkspaceSkillContent::FileTree(_) => panic!("expected local dir content"),
        }
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_for_builtin_skill_creates_skill_md_file_tree() {
        let manifest = SkillManifest {
            id: "builtin-general".to_string(),
            name: "通用助手".to_string(),
            description: "Generic assistant".to_string(),
            version: "builtin".to_string(),
            author: "WorkClaw".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "builtin-general",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            "",
            "builtin",
        )
        .unwrap();

        match entry.content {
            WorkspaceSkillContent::FileTree(files) => {
                let skill_md = files
                    .get("SKILL.md")
                    .expect("builtin SKILL.md should exist");
                let text = String::from_utf8(skill_md.clone()).unwrap();
                assert!(text.contains("通用助手") || text.contains("通用任务智能体"));
            }
            WorkspaceSkillContent::LocalDir(_) => panic!("expected builtin file tree content"),
        }
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_for_encrypted_skill_uses_unpacked_files() {
        let tmp = tempdir().unwrap();
        let skill_dir = tmp.path().join("skill-src");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: encrypted-skill\ndescription: Encrypted skill\n---\n\n# Skill\nHello",
        )
        .unwrap();
        std::fs::write(skill_dir.join("scripts").join("hello.py"), "print('hello')").unwrap();

        let output = tmp.path().join("encrypted.skillpack");
        pack(&PackConfig {
            dir_path: skill_dir.to_string_lossy().to_string(),
            name: "encrypted-skill".to_string(),
            description: "Encrypted skill".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            username: "alice".to_string(),
            recommended_model: "gpt-4o".to_string(),
            output_path: output.to_string_lossy().to_string(),
        })
        .unwrap();

        let unpacked = skillpack_rs::verify_and_unpack(&output.to_string_lossy(), "alice").unwrap();
        let entry = resolve_workspace_skill_runtime_entry(
            &unpacked.manifest.id,
            &serde_json::to_string(&unpacked.manifest).unwrap(),
            "alice",
            &output.to_string_lossy(),
            "encrypted",
        )
        .unwrap();

        match entry.content {
            WorkspaceSkillContent::FileTree(files) => {
                assert!(files.contains_key("SKILL.md"));
                assert!(files.contains_key("scripts/hello.py"));
            }
            WorkspaceSkillContent::LocalDir(_) => panic!("expected encrypted file tree content"),
        }
    }

    #[test]
    fn sync_workspace_skills_to_directory_copies_local_skill_tree() {
        let tmp = tempdir().unwrap();
        let source_dir = tmp.path().join("local-skill");
        std::fs::create_dir_all(source_dir.join("scripts")).unwrap();
        std::fs::write(source_dir.join("SKILL.md"), "# Skill").unwrap();
        std::fs::write(source_dir.join("scripts").join("hello.py"), "print('hi')").unwrap();

        let work_dir = tmp.path().join("workspace");
        let entry = super::WorkspaceSkillRuntimeEntry {
            skill_id: "local-skill".to_string(),
            name: "Local Skill".to_string(),
            description: "Local".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "local-skill".to_string(),
            content: WorkspaceSkillContent::LocalDir(source_dir),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        assert!(work_dir
            .join("skills")
            .join("local-skill")
            .join("SKILL.md")
            .exists());
        assert!(work_dir
            .join("skills")
            .join("local-skill")
            .join("scripts")
            .join("hello.py")
            .exists());
    }

    #[test]
    fn sync_workspace_skills_to_directory_skips_git_metadata_for_local_skill() {
        let tmp = tempdir().unwrap();
        let source_dir = tmp.path().join("local-skill");
        std::fs::create_dir_all(source_dir.join(".git").join("objects")).unwrap();
        std::fs::create_dir_all(source_dir.join("scripts")).unwrap();
        std::fs::write(source_dir.join("SKILL.md"), "# Skill").unwrap();
        std::fs::write(source_dir.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();
        std::fs::write(source_dir.join("scripts").join("hello.py"), "print('hi')").unwrap();

        let work_dir = tmp.path().join("workspace");
        let entry = super::WorkspaceSkillRuntimeEntry {
            skill_id: "local-skill".to_string(),
            name: "Local Skill".to_string(),
            description: "Local".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "local-skill".to_string(),
            content: WorkspaceSkillContent::LocalDir(source_dir),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        let projected = work_dir.join("skills").join("local-skill");
        assert!(projected.join("SKILL.md").exists());
        assert!(projected.join("scripts").join("hello.py").exists());
        assert!(!projected.join(".git").exists());
    }

    #[test]
    fn sync_workspace_skills_to_directory_writes_file_tree_entries() {
        let tmp = tempdir().unwrap();
        let work_dir = tmp.path().join("workspace");
        let mut files = std::collections::HashMap::new();
        files.insert("SKILL.md".to_string(), b"# Builtin".to_vec());
        files.insert("assets/template.txt".to_string(), b"hello".to_vec());

        let entry = super::WorkspaceSkillRuntimeEntry {
            skill_id: "builtin-general".to_string(),
            name: "Builtin".to_string(),
            description: "Builtin".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "builtin-general".to_string(),
            content: WorkspaceSkillContent::FileTree(files),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        assert!(work_dir
            .join("skills")
            .join("builtin-general")
            .join("SKILL.md")
            .exists());
        assert!(work_dir
            .join("skills")
            .join("builtin-general")
            .join("assets")
            .join("template.txt")
            .exists());
    }

    #[test]
    fn sync_workspace_skills_to_directory_rebuilds_skills_root() {
        let tmp = tempdir().unwrap();
        let work_dir = tmp.path().join("workspace");
        let stale_dir = work_dir.join("skills").join("stale-skill");
        std::fs::create_dir_all(&stale_dir).unwrap();
        std::fs::write(stale_dir.join("old.txt"), "stale").unwrap();

        let mut files = std::collections::HashMap::new();
        files.insert("SKILL.md".to_string(), b"# Fresh".to_vec());
        let entry = super::WorkspaceSkillRuntimeEntry {
            skill_id: "fresh-skill".to_string(),
            name: "Fresh".to_string(),
            description: "Fresh".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "fresh-skill".to_string(),
            content: WorkspaceSkillContent::FileTree(files),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        assert!(!stale_dir.exists());
        assert!(work_dir
            .join("skills")
            .join("fresh-skill")
            .join("SKILL.md")
            .exists());
    }

    #[test]
    fn build_workspace_skill_prompt_entries_use_projected_skill_paths() {
        let work_dir = Path::new("E:\\workspace\\session");
        let entries = vec![super::WorkspaceSkillRuntimeEntry {
            skill_id: "builtin-general".to_string(),
            name: "General".to_string(),
            description: "Generic".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "builtin-general".to_string(),
            content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
        }];

        let prompt_entries = build_workspace_skill_prompt_entries(work_dir, &entries);
        assert_eq!(prompt_entries.len(), 1);
        assert_eq!(
            prompt_entries[0].skill_md_path,
            "E:\\workspace\\session\\skills\\builtin-general\\SKILL.md"
        );
    }

    #[test]
    fn prepare_workspace_skills_prompt_syncs_and_returns_available_skills_block() {
        let tmp = tempdir().unwrap();
        let work_dir = tmp.path().join("workspace");
        let mut files = std::collections::HashMap::new();
        files.insert("SKILL.md".to_string(), b"# Fresh".to_vec());
        let entry = super::WorkspaceSkillRuntimeEntry {
            skill_id: "fresh-skill".to_string(),
            name: "Fresh".to_string(),
            description: "Fresh description".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "fresh-skill".to_string(),
            content: WorkspaceSkillContent::FileTree(files),
        };

        let prompt = prepare_workspace_skills_prompt(&work_dir, &[entry]).unwrap();

        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("<name>Fresh</name>"));
        assert!(prompt.contains("<description>Fresh description</description>"));
        let projected_skill_md = work_dir
            .join("skills")
            .join("fresh-skill")
            .join("SKILL.md")
            .to_string_lossy()
            .to_string();
        assert!(prompt.contains(&projected_skill_md));
        assert!(work_dir
            .join("skills")
            .join("fresh-skill")
            .join("SKILL.md")
            .exists());
    }

    #[tokio::test]
    async fn load_workspace_skill_runtime_entries_with_pool_reads_local_and_builtin_skills() {
        let tmp = tempdir().unwrap();
        let db_path = tmp.path().join("skills.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE installed_skills (
                id TEXT PRIMARY KEY,
                manifest TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                last_used_at TEXT,
                username TEXT NOT NULL,
                pack_path TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'encrypted'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        let local_skill_dir = tmp.path().join("local-skill");
        std::fs::create_dir_all(local_skill_dir.join("scripts")).unwrap();
        std::fs::write(local_skill_dir.join("SKILL.md"), "# Local Skill").unwrap();
        std::fs::write(
            local_skill_dir.join("scripts").join("hello.py"),
            "print('hi')",
        )
        .unwrap();

        let local_manifest = SkillManifest {
            id: "local-auto-redbook".to_string(),
            name: "xhs-note-creator".to_string(),
            description: "Create Xiaohongshu notes".to_string(),
            version: "local".to_string(),
            author: "tester".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };
        let builtin_manifest = SkillManifest {
            id: "builtin-general".to_string(),
            name: "通用助手".to_string(),
            description: "Generic assistant".to_string(),
            version: "builtin".to_string(),
            author: "WorkClaw".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("local-auto-redbook")
        .bind(serde_json::to_string(&local_manifest).unwrap())
        .bind(Utc::now().to_rfc3339())
        .bind("")
        .bind(local_skill_dir.to_string_lossy().to_string())
        .bind("local")
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("builtin-general")
        .bind(serde_json::to_string(&builtin_manifest).unwrap())
        .bind(Utc::now().to_rfc3339())
        .bind("")
        .bind("")
        .bind("builtin")
        .execute(&pool)
        .await
        .unwrap();

        let entries = super::load_workspace_skill_runtime_entries_with_pool(&pool)
            .await
            .unwrap();

        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| {
            entry.skill_id == "local-auto-redbook"
                && matches!(entry.content, WorkspaceSkillContent::LocalDir(_))
        }));
        assert!(entries.iter().any(|entry| {
            entry.skill_id == "builtin-general"
                && matches!(entry.content, WorkspaceSkillContent::FileTree(_))
        }));
    }

    #[test]
    fn sync_workspace_skills_to_directory_preserves_auto_redbook_style_layout() {
        let tmp = tempdir().unwrap();
        let source_dir = tmp.path().join("auto-redbook-skill");
        std::fs::create_dir_all(source_dir.join("scripts")).unwrap();
        std::fs::create_dir_all(source_dir.join("assets")).unwrap();
        std::fs::create_dir_all(source_dir.join("references")).unwrap();
        std::fs::write(source_dir.join("SKILL.md"), "# Auto Redbook").unwrap();
        std::fs::write(
            source_dir.join("scripts").join("publish_xhs.py"),
            "print('publish')",
        )
        .unwrap();
        std::fs::write(
            source_dir.join("assets").join("cover.html"),
            "<html></html>",
        )
        .unwrap();
        std::fs::write(source_dir.join("references").join("params.md"), "# params").unwrap();

        let work_dir = tmp.path().join("workspace");
        let entry = super::WorkspaceSkillRuntimeEntry {
            skill_id: "local-auto-redbook".to_string(),
            name: "xhs-note-creator".to_string(),
            description: "Create Xiaohongshu notes".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "local-auto-redbook".to_string(),
            content: WorkspaceSkillContent::LocalDir(source_dir),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        let projected = work_dir.join("skills").join("local-auto-redbook");
        assert!(projected.join("SKILL.md").exists());
        assert!(projected.join("scripts").join("publish_xhs.py").exists());
        assert!(projected.join("assets").join("cover.html").exists());
        assert!(projected.join("references").join("params.md").exists());
    }
}

pub(crate) fn extract_new_messages_after_reconstructed_history<'a>(
    final_messages: &'a [Value],
    reconstructed_history_len: usize,
) -> Vec<&'a Value> {
    final_messages
        .iter()
        .skip(reconstructed_history_len)
        .collect()
}

pub(crate) fn reconstruct_history_messages(
    history: &[(String, String, Option<String>)],
    api_format: &str,
) -> Vec<Value> {
    history
        .iter()
        .flat_map(|(role, content, content_json)| {
            if role == "assistant" {
                if let Ok(parsed) = serde_json::from_str::<Value>(content) {
                    if parsed.get("text").is_some() && parsed.get("items").is_some() {
                        return reconstruct_llm_messages(&parsed, api_format);
                    }
                }
            }
            if role == "user" {
                if let Some(content_json) = content_json {
                    if let Ok(parts) = serde_json::from_str::<Value>(content_json) {
                        if let Some(parts_array) = parts.as_array() {
                            if let Some(message) =
                                super::chat_send_message_flow::build_current_turn_message(
                                    api_format,
                                    parts_array,
                                )
                            {
                                return vec![message];
                            }
                        }
                    }
                }
            }
            vec![json!({"role": role, "content": content})]
        })
        .collect()
}

pub(crate) fn build_assistant_content_from_final_messages(
    final_messages: &[Value],
    reconstructed_history_len: usize,
) -> (String, bool, String) {
    let new_messages =
        extract_new_messages_after_reconstructed_history(final_messages, reconstructed_history_len);
    let mut ordered_items: Vec<Value> = Vec::new();
    let mut final_text = String::new();

    for msg in &new_messages {
        let role = msg["role"].as_str().unwrap_or("");

        if role == "assistant" {
            if let Some(content_arr) = msg["content"].as_array() {
                for block in content_arr {
                    match block["type"].as_str() {
                        Some("text") => {
                            let text = block["text"].as_str().unwrap_or("");
                            if !text.is_empty() {
                                ordered_items.push(json!({"type": "text", "content": text}));
                            }
                        }
                        Some("tool_use") => {
                            ordered_items.push(json!({
                                "type": "tool_call",
                                "toolCall": {
                                    "id": block["id"],
                                    "name": block["name"],
                                    "input": block["input"],
                                    "status": "completed"
                                }
                            }));
                        }
                        _ => {}
                    }
                }
            } else if let Some(text) = msg["content"].as_str() {
                if !text.is_empty() {
                    final_text = text.to_string();
                    ordered_items.push(json!({
                        "type": "text",
                        "content": text
                    }));
                }
            }
            if let Some(tool_calls_arr) = msg["tool_calls"].as_array() {
                for tc in tool_calls_arr {
                    let func = &tc["function"];
                    let input_val =
                        serde_json::from_str::<Value>(func["arguments"].as_str().unwrap_or("{}"))
                            .unwrap_or(json!({}));
                    ordered_items.push(json!({
                        "type": "tool_call",
                        "toolCall": {
                            "id": tc["id"],
                            "name": func["name"],
                            "input": input_val,
                            "status": "completed"
                        }
                    }));
                }
            }
        }

        if role == "user" {
            if let Some(content_arr) = msg["content"].as_array() {
                for block in content_arr {
                    if block["type"].as_str() == Some("tool_result") {
                        let tool_use_id = block["tool_use_id"].as_str().unwrap_or("");
                        let output = block["content"].as_str().unwrap_or("");
                        for item in ordered_items.iter_mut().rev() {
                            if item["type"].as_str() == Some("tool_call") {
                                let tc = &item["toolCall"];
                                if tc["id"].as_str() == Some(tool_use_id)
                                    && tc.get("output").map_or(true, |v| v.is_null())
                                {
                                    item["toolCall"]["output"] = Value::String(output.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        if role == "tool" {
            let tool_call_id = msg["tool_call_id"].as_str().unwrap_or("");
            let output = msg["content"].as_str().unwrap_or("");
            for item in ordered_items.iter_mut().rev() {
                if item["type"].as_str() == Some("tool_call") {
                    let tc = &item["toolCall"];
                    if tc["id"].as_str() == Some(tool_call_id)
                        && tc.get("output").map_or(true, |v| v.is_null())
                    {
                        item["toolCall"]["output"] = Value::String(output.to_string());
                        break;
                    }
                }
            }
        }
    }

    let has_tool_calls = ordered_items
        .iter()
        .any(|item| item["type"].as_str() == Some("tool_call"));
    let content = if has_tool_calls {
        serde_json::to_string(&json!({
            "text": final_text,
            "items": ordered_items,
        }))
        .unwrap_or(final_text.clone())
    } else {
        final_text.clone()
    };

    (final_text, has_tool_calls, content)
}

pub(crate) fn build_assistant_content_with_stream_fallback(
    final_messages: &[Value],
    reconstructed_history_len: usize,
    streamed_text: &str,
) -> (String, bool, String) {
    let (mut final_text, has_tool_calls, mut content) =
        build_assistant_content_from_final_messages(final_messages, reconstructed_history_len);
    let fallback_text = streamed_text.trim();

    if final_text.trim().is_empty() && !fallback_text.is_empty() {
        final_text = streamed_text.to_string();
        content = if has_tool_calls {
            let parsed = serde_json::from_str::<Value>(&content).unwrap_or_else(|_| {
                json!({
                    "text": "",
                    "items": [],
                })
            });
            let items = parsed
                .get("items")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            serde_json::to_string(&json!({
                "text": final_text,
                "items": items,
            }))
            .unwrap_or_else(|_| final_text.clone())
        } else {
            final_text.clone()
        };
    }

    (final_text, has_tool_calls, content)
}

#[cfg(test)]
mod tests {
    use super::{
        build_assistant_content_from_final_messages, build_assistant_content_with_stream_fallback,
        extract_assistant_text_content, reconstruct_history_messages,
    };
    use serde_json::{json, Value};

    #[test]
    fn stream_fallback_restores_empty_text_response() {
        let final_messages = vec![json!({
            "role": "assistant",
            "content": ""
        })];

        let (final_text, has_tool_calls, content) =
            build_assistant_content_with_stream_fallback(&final_messages, 0, "你好，我在。");

        assert_eq!(final_text, "你好，我在。");
        assert!(!has_tool_calls);
        assert_eq!(content, "你好，我在。");
    }

    #[test]
    fn stream_fallback_preserves_tool_calls_when_text_missing() {
        let final_messages = vec![
            json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [
                    {
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "search",
                            "arguments": "{\"q\":\"minimax\"}"
                        }
                    }
                ]
            }),
            json!({
                "role": "tool",
                "tool_call_id": "call-1",
                "content": "ok"
            }),
        ];

        let (_, has_tool_calls_before, content_before) =
            build_assistant_content_from_final_messages(&final_messages, 0);
        assert!(has_tool_calls_before);

        let (final_text, has_tool_calls, content) =
            build_assistant_content_with_stream_fallback(&final_messages, 0, "我查到了结果");

        assert_eq!(final_text, "我查到了结果");
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured content");
        assert_eq!(parsed["text"].as_str(), Some("我查到了结果"));
        assert_eq!(parsed["items"].as_array().map(|items| items.len()), Some(1));
        assert_eq!(
            parsed["items"][0]["toolCall"]["name"].as_str(),
            Some("search")
        );

        let parsed_before: Value =
            serde_json::from_str(&content_before).expect("structured content before fallback");
        assert_eq!(parsed_before["text"].as_str(), Some(""));
    }

    #[test]
    fn build_assistant_content_from_final_messages_does_not_duplicate_text_when_tool_calls_exist() {
        let final_messages = vec![json!({
            "role": "assistant",
            "content": "让我先检查正确的目录路径。",
            "tool_calls": [
                {
                    "id": "call-1",
                    "type": "function",
                    "function": {
                        "name": "list_dir",
                        "arguments": "{\"path\":\".\"}"
                    }
                }
            ]
        })];

        let (final_text, has_tool_calls, content) =
            build_assistant_content_from_final_messages(&final_messages, 0);

        assert_eq!(final_text, "让我先检查正确的目录路径。");
        assert!(has_tool_calls);

        let parsed: Value = serde_json::from_str(&content).expect("structured content");
        let items = parsed["items"].as_array().expect("items array");

        assert_eq!(parsed["text"].as_str(), Some("让我先检查正确的目录路径。"));
        assert_eq!(
            items
                .iter()
                .filter(|item| item["type"].as_str() == Some("text"))
                .count(),
            1
        );
        assert_eq!(items[0]["content"].as_str(), Some("让我先检查正确的目录路径。"));
        assert_eq!(items[1]["toolCall"]["name"].as_str(), Some("list_dir"));
    }

    #[test]
    fn extract_assistant_text_content_prefers_text_field() {
        let content =
            r#"{"text":"最终答案","reasoning":{"status":"completed","content":"内部思考"}}"#;
        assert_eq!(extract_assistant_text_content(content), "最终答案");
    }

    #[test]
    fn extract_assistant_text_content_falls_back_for_plain_text() {
        assert_eq!(extract_assistant_text_content("普通文本"), "普通文本");
    }

    #[test]
    fn reconstruct_history_messages_restores_user_multimodal_parts() {
        let history = vec![(
            "user".to_string(),
            "[图片 1 张] [文本文件 1 个]".to_string(),
            Some(
                serde_json::to_string(&json!([
                    { "type": "text", "text": "请分析这些附件" },
                    {
                        "type": "image",
                        "name": "screen.png",
                        "mimeType": "image/png",
                        "data": "data:image/png;base64,aGVsbG8="
                    },
                    {
                        "type": "file_text",
                        "name": "debug.ts",
                        "mimeType": "text/plain",
                        "text": "console.log('hi')"
                    }
                ]))
                .expect("serialize parts"),
            ),
        )];

        let messages = reconstruct_history_messages(&history, "openai");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"].as_str(), Some("user"));
        let content = messages[0]["content"].as_array().expect("content array");
        assert_eq!(content[0]["type"].as_str(), Some("text"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("请分析这些附件"));
        assert!(content[0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("debug.ts"));
        assert_eq!(content[1]["type"].as_str(), Some("image_url"));
    }
}

#[cfg(test)]
mod run_guard_persistence_tests {
    use super::{
        append_run_guard_warning_with_pool, append_run_started_with_pool,
        append_run_stopped_with_pool,
    };
    use crate::agent::run_guard::RunStopReason;
    use crate::session_journal::SessionJournalStore;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn setup_run_event_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE session_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_message_id TEXT NOT NULL DEFAULT '',
                assistant_message_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'queued',
                buffered_text TEXT NOT NULL DEFAULT '',
                error_kind TEXT NOT NULL DEFAULT '',
                error_message TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_runs table");

        sqlx::query(
            "CREATE TABLE session_run_events (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create session_run_events table");

        pool
    }

    #[tokio::test]
    async fn append_run_stopped_event_persists_loop_detected_reason() {
        let pool = setup_run_event_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());
        let stop_reason =
            RunStopReason::loop_detected("工具 browser_snapshot 已连续 6 次返回相同结果。")
                .with_last_completed_step("已填写封面标题");

        append_run_started_with_pool(&pool, &journal, "session-1", "run-1", "user-1")
            .await
            .expect("append run started");
        append_run_stopped_with_pool(&pool, &journal, "session-1", "run-1", &stop_reason)
            .await
            .expect("append run stopped");

        let (event_type, payload_json): (String, String) = sqlx::query_as(
            "SELECT event_type, payload_json
             FROM session_run_events
             WHERE run_id = 'run-1' AND event_type = 'run_stopped'
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("query run_stopped event");
        assert_eq!(event_type, "run_stopped");
        assert!(payload_json.contains("\"kind\":\"loop_detected\""));
        assert!(payload_json.contains("\"last_completed_step\":\"已填写封面标题\""));

        let (status, error_kind, error_message): (String, String, String) = sqlx::query_as(
            "SELECT status, error_kind, error_message
             FROM session_runs
             WHERE id = 'run-1'",
        )
        .fetch_one(&pool)
        .await
        .expect("query session run projection");
        assert_eq!(status, "failed");
        assert_eq!(error_kind, "loop_detected");
        assert!(error_message.contains("最后完成步骤：已填写封面标题"));
    }

    #[tokio::test]
    async fn append_run_guard_warning_event_persists_warning_payload() {
        let pool = setup_run_event_pool().await;
        let journal_root = tempdir().expect("journal tempdir");
        let journal = SessionJournalStore::new(journal_root.path().to_path_buf());

        append_run_started_with_pool(&pool, &journal, "session-2", "run-2", "user-2")
            .await
            .expect("append run started");
        append_run_guard_warning_with_pool(
            &pool,
            &journal,
            "session-2",
            "run-2",
            "loop_detected",
            "任务可能即将卡住",
            "系统检测到连续重复步骤，若继续无变化将自动停止。",
            Some("工具 browser_snapshot 已连续 5 次使用相同输入执行。"),
            Some("已填写封面标题"),
        )
        .await
        .expect("append run guard warning");

        let (event_type, payload_json): (String, String) = sqlx::query_as(
            "SELECT event_type, payload_json
             FROM session_run_events
             WHERE run_id = 'run-2' AND event_type = 'run_guard_warning'
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("query run_guard_warning event");

        assert_eq!(event_type, "run_guard_warning");
        assert!(payload_json.contains("\"warning_kind\":\"loop_detected\""));
        assert!(payload_json.contains("\"last_completed_step\":\"已填写封面标题\""));
    }
}
