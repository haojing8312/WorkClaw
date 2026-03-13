use super::employee_agents::maybe_handle_team_entry_session_message_with_pool;
use super::session_runs::{
    append_session_run_event_with_pool, attach_assistant_message_to_run_with_pool,
};
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
) -> Result<String, String> {
    let msg_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&msg_id)
    .bind(session_id)
    .bind(role)
    .bind(content)
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
        let title: String = user_message.chars().take(20).collect();
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
        let persisted_content = attach_reasoning_to_content(content, final_text, has_tool_calls, reasoning_text, reasoning_duration_ms);
        let msg_id =
            insert_session_message_with_pool(pool, session_id, "assistant", &persisted_content).await?;
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
) -> Result<Vec<(String, String)>, String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|(role, content)| {
            if role == "assistant" {
                (role, extract_assistant_text_content(&content))
            } else {
                (role, content)
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
    history: &[(String, String)],
    api_format: &str,
) -> Vec<Value> {
    history
        .iter()
        .flat_map(|(role, content)| {
            if role == "assistant" {
                if let Ok(parsed) = serde_json::from_str::<Value>(content) {
                    if parsed.get("text").is_some() && parsed.get("items").is_some() {
                        return reconstruct_llm_messages(&parsed, api_format);
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
                if let Some(text) = msg["content"].as_str() {
                    if !text.is_empty() {
                        ordered_items.push(json!({"type": "text", "content": text}));
                    }
                }
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
        extract_assistant_text_content,
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
    fn extract_assistant_text_content_prefers_text_field() {
        let content = r#"{"text":"最终答案","reasoning":{"status":"completed","content":"内部思考"}}"#;
        assert_eq!(extract_assistant_text_content(content), "最终答案");
    }

    #[test]
    fn extract_assistant_text_content_falls_back_for_plain_text() {
        assert_eq!(extract_assistant_text_content("普通文本"), "普通文本");
    }
}
