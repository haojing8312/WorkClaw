use super::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::commands::chat_runtime_io::{
    derive_meaningful_session_title_from_messages, extract_assistant_text_content,
    is_generic_session_title,
};
use crate::session_journal::{
    SessionJournalState, SessionJournalStore, SessionRunEvent, SessionRunStatus,
};
use chrono::Utc;
use runtime_chat_app::{ChatPreparationService, SessionCreationRequest};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone)]
struct ExportToolCall {
    call_id: String,
    name: String,
    input: Value,
    output: String,
    status: String,
}

pub(crate) async fn create_session_with_pool(
    pool: &sqlx::SqlitePool,
    skill_id: String,
    model_id: String,
    work_dir: Option<String>,
    employee_id: Option<String>,
    title: Option<String>,
    permission_mode: Option<String>,
    session_mode: Option<String>,
    team_id: Option<String>,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let prepared = ChatPreparationService::new().prepare_session_creation(SessionCreationRequest {
        permission_mode,
        session_mode,
        team_id,
        title,
        work_dir,
        employee_id,
    });
    let resolved_work_dir = if prepared.normalized_work_dir.is_empty() {
        resolve_default_work_dir_with_pool(pool).await?
    } else {
        prepared.normalized_work_dir
    };
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&session_id)
    .bind(&skill_id)
    .bind(&prepared.normalized_title)
    .bind(&now)
    .bind(&model_id)
    .bind(&prepared.permission_mode_storage)
    .bind(&resolved_work_dir)
    .bind(&prepared.normalized_employee_id)
    .bind(&prepared.session_mode_storage)
    .bind(&prepared.normalized_team_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(session_id)
}

pub(crate) async fn get_messages_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<Vec<Value>, String> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            String,
            Option<String>,
        ),
    >(
        "SELECT
            m.id,
            m.role,
            m.content,
            m.content_json,
            m.created_at,
            NULLIF(sr.id, '') AS run_id
         FROM messages m
         LEFT JOIN session_runs sr ON sr.assistant_message_id = m.id
         WHERE m.session_id = ?
         ORDER BY m.created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|(id, role, content, content_json, created_at, run_id)| {
            if role == "assistant" {
                if let Ok(parsed) = serde_json::from_str::<Value>(content) {
                    if let Some(text) = parsed.get("text") {
                        let reasoning = parsed.get("reasoning").cloned().unwrap_or(Value::Null);
                        if let Some(items) = parsed.get("items") {
                            let normalized = normalize_stream_items(items);
                            return json!({
                                "id": id,
                                "role": role,
                                "content": text,
                                "created_at": created_at,
                                "runId": run_id,
                                "reasoning": reasoning,
                                "streamItems": normalized,
                            });
                        }
                        let tool_calls = parsed.get("tool_calls").cloned().unwrap_or(Value::Null);
                        return json!({
                            "id": id,
                            "role": role,
                            "content": text,
                            "created_at": created_at,
                            "runId": run_id,
                            "reasoning": reasoning,
                            "tool_calls": tool_calls,
                        });
                    }
                }
            }
            let mut message = json!({
                "id": id,
                "role": role,
                "content": content,
                "created_at": created_at,
                "runId": run_id,
            });
            if let Some(parts) = content_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .filter(|value| value.is_array())
            {
                message["contentParts"] = parts;
            }
            message
        })
        .collect())
}

pub(crate) fn resolve_im_session_source(channel: Option<&str>) -> (String, String) {
    match channel.unwrap_or("").trim() {
        "wecom" => ("wecom".to_string(), "企业微信".to_string()),
        "feishu" => ("feishu".to_string(), "飞书".to_string()),
        other if other.is_empty() => ("local".to_string(), String::new()),
        other => (other.to_string(), other.to_string()),
    }
}

pub(crate) async fn list_sessions_with_pool(
    pool: &sqlx::SqlitePool,
    permission_mode_label_for_display: fn(&str) -> &'static str,
) -> Result<Vec<Value>, String> {
    let runtime_status_rows = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT
            s.id,
            (
                SELECT CASE
                    WHEN sr.status = 'waiting_approval' THEN 'waiting_approval'
                    WHEN sr.status IN ('thinking', 'tool_calling', 'waiting_user') THEN 'running'
                    WHEN sr.status = 'completed' THEN 'completed'
                    WHEN sr.status IN ('failed', 'cancelled') THEN 'failed'
                    ELSE NULL
                END
                FROM session_runs sr
                WHERE sr.session_id = s.id
                ORDER BY
                    CASE
                        WHEN sr.status = 'waiting_approval' THEN 0
                        WHEN sr.status IN ('thinking', 'tool_calling', 'waiting_user') THEN 1
                        ELSE 2
                    END,
                    sr.updated_at DESC,
                    sr.created_at DESC,
                    sr.id DESC
                LIMIT 1
            ) AS runtime_status
         FROM sessions s",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let runtime_status_by_session_id = runtime_status_rows
        .into_iter()
        .map(|(session_id, runtime_status)| (session_id, runtime_status))
        .collect::<std::collections::HashMap<_, _>>();

    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT
            s.id,
            COALESCE(s.skill_id, ''),
            s.title,
            s.created_at,
            s.model_id,
            COALESCE(s.work_dir, ''),
            COALESCE(s.employee_id, ''),
            COALESCE(s.permission_mode, 'standard'),
            COALESCE(s.session_mode, 'general'),
            COALESCE(s.team_id, ''),
            COALESCE((
                SELECT ts.channel
                FROM im_thread_sessions ts
                WHERE ts.session_id = s.id
                ORDER BY ts.updated_at DESC, ts.created_at DESC
                LIMIT 1
            ), '') AS im_source_channel
         FROM sessions s
         ORDER BY s.created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut employee_name_by_code = std::collections::HashMap::<String, String>::new();
    let employee_rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT COALESCE(employee_id, ''), COALESCE(role_id, ''), COALESCE(name, '') FROM agent_employees",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for (employee_id, role_id, name) in employee_rows {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            continue;
        }
        let display_name = trimmed_name.to_string();
        if !employee_id.trim().is_empty() {
            employee_name_by_code.insert(employee_id.trim().to_string(), display_name.clone());
        }
        if !role_id.trim().is_empty() {
            employee_name_by_code.insert(role_id.trim().to_string(), display_name);
        }
    }

    let team_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT COALESCE(id, ''), COALESCE(name, '') FROM employee_groups",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let team_name_by_id = team_rows
        .into_iter()
        .filter_map(|(id, name)| {
            let id = id.trim().to_string();
            let name = name.trim().to_string();
            if id.is_empty() || name.is_empty() {
                None
            } else {
                Some((id, name))
            }
        })
        .collect::<std::collections::HashMap<_, _>>();

    let mut sessions = Vec::with_capacity(rows.len());
    for (
        id,
        skill_id,
        title,
        created_at,
        model_id,
        work_dir,
        employee_id,
        permission_mode,
        session_mode,
        team_id,
        im_source_channel,
    ) in rows
    {
        let title = title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("New Chat")
            .to_string();
        let created_at = created_at.unwrap_or_default();
        let model_id = model_id.unwrap_or_default();
        let work_dir = work_dir.unwrap_or_default();
        let employee_id = employee_id.unwrap_or_default();
        let permission_mode = permission_mode.unwrap_or_else(|| "standard".to_string());
        let session_mode = session_mode.unwrap_or_else(|| "general".to_string());
        let team_id = team_id.unwrap_or_default();
        let im_source_channel = im_source_channel.unwrap_or_default();
        let employee_name = employee_name_by_code
            .get(employee_id.trim())
            .cloned()
            .unwrap_or_default();
        let (source_channel, source_label) = resolve_im_session_source(Some(&im_source_channel));
        let runtime_status = runtime_status_by_session_id
            .get(&id)
            .cloned()
            .flatten();
        let display_title = derive_session_display_title_with_pool(
            pool,
            &id,
            &title,
            &session_mode,
            &employee_id,
            &team_id,
            &employee_name_by_code,
            &team_name_by_id,
        )
        .await;
        sessions.push(json!({
            "id": id,
            "skill_id": skill_id,
            "title": title,
            "display_title": display_title,
            "created_at": created_at,
            "model_id": model_id,
            "work_dir": work_dir,
            "employee_id": employee_id,
            "employee_name": employee_name,
            "permission_mode": permission_mode,
            "session_mode": session_mode,
            "team_id": team_id,
            "permission_mode_label": permission_mode_label_for_display(&permission_mode),
            "source_channel": source_channel,
            "source_label": source_label,
            "runtime_status": runtime_status,
        }));
    }

    Ok(sessions)
}

pub(crate) async fn update_session_workspace_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    workspace: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET work_dir = ? WHERE id = ?")
        .bind(workspace)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn delete_session_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub(crate) async fn search_sessions_global_with_pool(
    pool: &sqlx::SqlitePool,
    query: &str,
) -> Result<Vec<Value>, String> {
    let pattern = format!("%{}%", query);
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        ),
    >(
        "SELECT DISTINCT
            s.id,
            COALESCE(s.skill_id, ''),
            s.title,
            s.created_at,
            s.model_id,
            COALESCE(s.work_dir, ''),
            COALESCE(s.employee_id, ''),
            COALESCE((
                SELECT ts.channel
                FROM im_thread_sessions ts
                WHERE ts.session_id = s.id
                ORDER BY ts.updated_at DESC, ts.created_at DESC
                LIMIT 1
            ), '') AS im_source_channel
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
         WHERE (s.title LIKE ? OR m.content LIKE ?)
         ORDER BY s.created_at DESC",
    )
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(
            |(
                id,
                skill_id,
                title,
                created_at,
                model_id,
                work_dir,
                employee_id,
                im_source_channel,
            )| {
                let (source_channel, source_label) =
                    resolve_im_session_source(Some(im_source_channel));
                json!({
                    "id": id,
                    "skill_id": skill_id,
                    "title": title,
                    "display_title": title,
                    "created_at": created_at,
                    "model_id": model_id,
                    "work_dir": work_dir,
                    "employee_id": employee_id,
                    "source_channel": source_channel,
                    "source_label": source_label
                })
            },
        )
        .collect())
}

async fn derive_session_display_title_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    persisted_title: &str,
    session_mode: &str,
    employee_id: &str,
    team_id: &str,
    employee_name_by_code: &std::collections::HashMap<String, String>,
    team_name_by_id: &std::collections::HashMap<String, String>,
) -> String {
    if session_mode == "team_entry" {
        if let Some(team_name) = team_name_by_id.get(team_id.trim()) {
            return team_name.clone();
        }
    }

    if session_mode == "employee_direct" || !employee_id.trim().is_empty() {
        if let Some(employee_name) = employee_name_by_code.get(employee_id.trim()) {
            return employee_name.clone();
        }
    }

    if !is_generic_session_title(persisted_title) {
        return persisted_title.trim().to_string();
    }

    let user_messages = sqlx::query_as::<_, (String,)>(
        "SELECT content
         FROM messages
         WHERE session_id = ? AND role = 'user'
         ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    derive_meaningful_session_title_from_messages(
        user_messages.iter().map(|(content,)| content.as_str()),
    )
    .unwrap_or_else(|| persisted_title.trim().to_string())
}

pub(crate) async fn export_session_markdown_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    journal: Option<&SessionJournalStore>,
) -> Result<String, String> {
    let (title,): (String,) = sqlx::query_as("SELECT title FROM sessions WHERE id = ?")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    let messages = sqlx::query_as::<_, (String, String, Option<String>, String, Option<String>)>(
        "SELECT
            m.role,
            m.content,
            m.content_json,
            m.created_at,
            NULLIF(sr.id, '') AS run_id
         FROM messages m
         LEFT JOIN session_runs sr ON sr.assistant_message_id = m.id
         WHERE m.session_id = ?
         ORDER BY m.created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let tool_calls_by_run = load_export_tool_calls_with_pool(pool, session_id).await?;
    let assistant_run_ids_in_messages: HashSet<String> = messages
        .iter()
        .filter_map(|(role, _, _, _, run_id)| {
            if role == "assistant" {
                run_id.as_ref().map(|value| value.to_string())
            } else {
                None
            }
        })
        .collect();

    let mut md = format!("# {}\n\n", title);
    for (role, content, content_json, created_at, run_id) in &messages {
        let label = if role == "user" { "用户" } else { "助手" };
        let tool_calls = run_id
            .as_ref()
            .and_then(|value| tool_calls_by_run.get(value))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let rendered_content =
            render_export_message_content(role, content, content_json.as_deref(), tool_calls);
        md.push_str(&format!(
            "## {} ({})\n\n{}\n\n---\n\n",
            label, created_at, rendered_content
        ));
    }

    if let Some(journal_store) = journal {
        if let Ok(state) = journal_store.read_state(session_id).await {
            let recovered = render_recovered_run_sections(
                &messages,
                &state,
                &tool_calls_by_run,
                &assistant_run_ids_in_messages,
            );
            if !recovered.is_empty() {
                md.push_str("## 恢复的运行记录\n\n");
                md.push_str(&recovered);
            }
        }
    }

    Ok(md)
}

pub(crate) async fn load_compaction_inputs_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<(Vec<Value>, String, String, String, String), String> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT role, content, content_json FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let messages: Vec<Value> = rows
        .iter()
        .map(|(role, content, content_json)| {
            let normalized_content = if role == "assistant" {
                extract_assistant_text_content(content)
            } else if let Some(parts_json) = content_json {
                render_user_content_parts(parts_json).unwrap_or_else(|| content.clone())
            } else {
                content.clone()
            };
            json!({ "role": role, "content": normalized_content })
        })
        .collect();

    let (model_id,): (String,) = sqlx::query_as("SELECT model_id FROM sessions WHERE id = ?")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    let (api_format, base_url, api_key, model_name) =
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT api_format, base_url, api_key, model_name FROM model_configs WHERE id = ?",
        )
        .bind(&model_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok((messages, api_format, base_url, api_key, model_name))
}

pub(crate) async fn replace_messages_with_compacted_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    compacted: &[Value],
) -> Result<(), String> {
    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    for msg in compacted {
        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, content_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(msg["role"].as_str().unwrap_or("user"))
        .bind(msg["content"].as_str().unwrap_or(""))
        .bind(Option::<String>::None)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub(crate) fn write_export_file_to_path(path: &str, content: &str) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| format!("写入失败: {}", e))
}

fn normalize_stream_items(items: &Value) -> Value {
    if let Some(arr) = items.as_array() {
        Value::Array(
            arr.iter()
                .map(|item| {
                    if item["type"].as_str() == Some("tool_call") && item.get("toolCall").is_none()
                    {
                        json!({
                            "type": "tool_call",
                            "toolCall": {
                                "id": item["id"],
                                "name": item["name"],
                                "input": item["input"],
                                "output": item["output"],
                                "status": item["status"]
                            }
                        })
                    } else {
                        item.clone()
                    }
                })
                .collect(),
        )
    } else {
        items.clone()
    }
}

async fn load_export_tool_calls_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<HashMap<String, Vec<ExportToolCall>>, String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT run_id, payload_json
         FROM session_run_events
         WHERE session_id = ? AND event_type IN ('tool_started', 'tool_completed')
         ORDER BY created_at ASC, id ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut by_run: HashMap<String, Vec<ExportToolCall>> = HashMap::new();
    for (run_id, payload_json) in rows {
        let Ok(event) = serde_json::from_str::<SessionRunEvent>(&payload_json) else {
            continue;
        };
        let entries = by_run.entry(run_id).or_default();
        match event {
            SessionRunEvent::ToolStarted {
                call_id,
                tool_name,
                input,
                ..
            } => {
                if let Some(existing) = entries.iter_mut().find(|entry| entry.call_id == call_id) {
                    existing.name = tool_name;
                    existing.input = input;
                    existing.status = "running".to_string();
                } else {
                    entries.push(ExportToolCall {
                        call_id,
                        name: tool_name,
                        input,
                        output: String::new(),
                        status: "running".to_string(),
                    });
                }
            }
            SessionRunEvent::ToolCompleted {
                call_id,
                tool_name,
                input,
                output,
                is_error,
                ..
            } => {
                let status = if is_error { "error" } else { "completed" }.to_string();
                if let Some(existing) = entries.iter_mut().find(|entry| entry.call_id == call_id) {
                    existing.name = tool_name;
                    existing.input = input;
                    existing.output = output;
                    existing.status = status;
                } else {
                    entries.push(ExportToolCall {
                        call_id,
                        name: tool_name,
                        input,
                        output,
                        status,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(by_run)
}

fn render_export_message_content(
    role: &str,
    content: &str,
    content_json: Option<&str>,
    supplemental_tool_calls: &[ExportToolCall],
) -> String {
    if role == "user" {
        return content_json
            .and_then(render_user_content_parts)
            .unwrap_or_else(|| content.to_string());
    }

    if role != "assistant" {
        return content.to_string();
    }

    let mut sections: Vec<String> = Vec::new();
    if let Ok(parsed) = serde_json::from_str::<Value>(content) {
        let final_text = parsed["text"].as_str().unwrap_or("").trim();
        if !final_text.is_empty() {
            sections.push(final_text.to_string());
        }

        if let Some(items) = parsed["items"].as_array() {
            for item in items {
                match item["type"].as_str() {
                    Some("text") => {
                        if let Some(text) = item["content"]
                            .as_str()
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                        {
                            if !sections.iter().any(|section| section.contains(text)) {
                                sections.push(text.to_string());
                            }
                        }
                    }
                    Some("tool_call") => {
                        if let Some(tool_section) = render_export_tool_call(item.get("toolCall")) {
                            push_unique_export_section(&mut sections, tool_section);
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(tool_calls) = parsed["tool_calls"].as_array() {
            for item in tool_calls {
                if let Some(tool_section) = render_export_tool_call(Some(item)) {
                    push_unique_export_section(&mut sections, tool_section);
                }
            }
        }
    } else if !content.trim().is_empty() {
        sections.push(content.trim().to_string());
    }

    for tool_call in supplemental_tool_calls {
        if let Some(tool_section) = render_export_tool_call_entry(tool_call) {
            push_unique_export_section(&mut sections, tool_section);
        }
    }

    if sections.is_empty() {
        content.to_string()
    } else {
        sections.join("\n\n")
    }
}

fn push_unique_export_section(sections: &mut Vec<String>, section: String) {
    if !section.trim().is_empty() && !sections.iter().any(|existing| existing == &section) {
        sections.push(section);
    }
}

fn render_export_tool_call(tool_call: Option<&Value>) -> Option<String> {
    let tool_call = tool_call?;

    let name = tool_call["name"]
        .as_str()
        .or_else(|| tool_call["function"]["name"].as_str())
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        return None;
    }

    let input = if tool_call["input"].is_object() {
        tool_call["input"].clone()
    } else if let Some(arguments) = tool_call["function"]["arguments"].as_str() {
        serde_json::from_str::<Value>(arguments).unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    let output = tool_call["output"].as_str().unwrap_or("").trim();
    let status = tool_call["status"].as_str().unwrap_or("").trim();

    let mut lines = vec![format!("**工具调用** `{}`", name)];
    if let Some(path) = read_tool_call_path(&input) {
        lines.push(format!("- 路径：`{}`", path));
    }
    if !status.is_empty() {
        lines.push(format!(
            "- 状态：{}",
            render_export_tool_status(status, output)
        ));
    }
    if !output.is_empty() {
        lines.push("```text".to_string());
        lines.push(output.to_string());
        lines.push("```".to_string());
    }

    Some(lines.join("\n"))
}

fn render_export_tool_call_entry(tool_call: &ExportToolCall) -> Option<String> {
    let tool_call_value = json!({
        "name": tool_call.name,
        "input": tool_call.input,
        "output": tool_call.output,
        "status": tool_call.status,
    });
    render_export_tool_call(Some(&tool_call_value))
}

fn read_tool_call_path(input: &Value) -> Option<&str> {
    input["path"]
        .as_str()
        .or_else(|| input["file_path"].as_str())
        .filter(|value| !value.trim().is_empty())
}

fn render_export_tool_status(status: &str, output: &str) -> &'static str {
    if status.eq_ignore_ascii_case("error")
        || output.contains("工具执行错误")
        || output.contains("工具参数错误")
        || output.contains("工具执行线程异常")
    {
        "错误"
    } else if status.eq_ignore_ascii_case("running") {
        "进行中"
    } else {
        "已完成"
    }
}

fn render_recovered_run_sections(
    messages: &[(String, String, Option<String>, String, Option<String>)],
    state: &SessionJournalState,
    tool_calls_by_run: &HashMap<String, Vec<ExportToolCall>>,
    assistant_run_ids_in_messages: &HashSet<String>,
) -> String {
    let assistant_contents: Vec<&str> = messages
        .iter()
        .filter_map(|(role, content, _, _, _)| (role == "assistant").then_some(content.as_str()))
        .collect();

    let mut sections = Vec::new();
    for run in &state.runs {
        let buffered = run.buffered_text.trim();
        let error_message = run.last_error_message.as_deref().unwrap_or("").trim();
        let tool_sections: Vec<String> = tool_calls_by_run
            .get(&run.run_id)
            .map(|tool_calls| {
                tool_calls
                    .iter()
                    .filter_map(render_export_tool_call_entry)
                    .collect()
            })
            .unwrap_or_default();
        let buffered_already_exported = !buffered.is_empty()
            && assistant_contents
                .iter()
                .any(|content| content.contains(buffered));
        let error_already_exported = !error_message.is_empty()
            && assistant_contents
                .iter()
                .any(|content| content.contains(error_message));
        let missing_assistant_message_for_run =
            !assistant_run_ids_in_messages.contains(&run.run_id);
        let should_recover = (!buffered.is_empty() && !buffered_already_exported)
            || (!error_message.is_empty() && !error_already_exported)
            || (missing_assistant_message_for_run && !tool_sections.is_empty())
            || matches!(
                &run.status,
                SessionRunStatus::Failed | SessionRunStatus::Cancelled
            );

        if !should_recover {
            continue;
        }

        sections.push(format!(
            "### Run {} ({})",
            run.run_id,
            export_status_label(&run.status)
        ));
        sections.push(String::new());
        if !buffered.is_empty() && !buffered_already_exported {
            sections.push("#### 已保留的部分输出".to_string());
            sections.push(String::new());
            sections.push(buffered.to_string());
            sections.push(String::new());
        }
        if let Some(error_kind) = &run.last_error_kind {
            if !error_kind.trim().is_empty() {
                sections.push(format!("- error_kind: {}", error_kind));
            }
        }
        if !error_message.is_empty() && !error_already_exported {
            sections.push(format!("- error_message: {}", error_message));
        }
        if missing_assistant_message_for_run {
            for tool_section in tool_sections {
                sections.push(String::new());
                sections.push(tool_section);
            }
        }
        sections.push("\n---\n".to_string());
    }

    sections.join("\n")
}

fn export_status_label(status: &SessionRunStatus) -> &'static str {
    match status {
        SessionRunStatus::Queued => "queued",
        SessionRunStatus::Thinking => "thinking",
        SessionRunStatus::ToolCalling => "tool_calling",
        SessionRunStatus::WaitingApproval => "waiting_approval",
        SessionRunStatus::WaitingUser => "waiting_user",
        SessionRunStatus::Completed => "completed",
        SessionRunStatus::Failed => "failed",
        SessionRunStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::{list_sessions_with_pool, render_user_content_parts, resolve_im_session_source};
    use crate::commands::chat_policy::permission_mode_label_for_display;
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn resolve_im_session_source_maps_wecom_and_feishu_labels() {
        assert_eq!(
            resolve_im_session_source(Some("wecom")),
            ("wecom".to_string(), "企业微信".to_string())
        );
        assert_eq!(
            resolve_im_session_source(Some("feishu")),
            ("feishu".to_string(), "飞书".to_string())
        );
        assert_eq!(
            resolve_im_session_source(Some("")),
            ("local".to_string(), String::new())
        );
        assert_eq!(
            resolve_im_session_source(None),
            ("local".to_string(), String::new())
        );
    }

    #[tokio::test]
    async fn list_sessions_with_pool_tolerates_null_titles() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                model_id TEXT NOT NULL,
                permission_mode TEXT NOT NULL DEFAULT 'standard',
                work_dir TEXT NOT NULL DEFAULT '',
                employee_id TEXT NOT NULL DEFAULT '',
                session_mode TEXT NOT NULL DEFAULT 'general',
                team_id TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create sessions table");

        sqlx::query(
            "CREATE TABLE im_thread_sessions (
                thread_id TEXT NOT NULL,
                employee_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                route_session_key TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                channel TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (thread_id, employee_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_thread_sessions table");

        sqlx::query(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                content_json TEXT,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create messages table");

        sqlx::query(
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                role_id TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        sqlx::query(
            "CREATE TABLE employee_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create employee_groups table");

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES
             ('session-null-title', 'skill-1', NULL, '2026-03-13T00:00:00Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-normal', 'skill-1', 'Visible Session', '2026-03-13T00:01:00Z', 'model-1', 'full_access', '', '', 'general', '')",
        )
        .execute(&pool)
        .await
        .expect("seed sessions");

        let sessions = list_sessions_with_pool(&pool, permission_mode_label_for_display)
            .await
            .expect("list sessions should succeed");

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0]["id"], "session-normal");
        assert_eq!(sessions[0]["title"], "Visible Session");
        assert_eq!(sessions[1]["id"], "session-null-title");
        assert_eq!(sessions[1]["title"], "New Chat");
    }

    #[tokio::test]
    async fn list_sessions_with_pool_derives_display_title_for_general_sessions() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                model_id TEXT NOT NULL,
                permission_mode TEXT NOT NULL DEFAULT 'standard',
                work_dir TEXT NOT NULL DEFAULT '',
                employee_id TEXT NOT NULL DEFAULT '',
                session_mode TEXT NOT NULL DEFAULT 'general',
                team_id TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create sessions table");

        sqlx::query(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                content_json TEXT,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create messages table");

        sqlx::query(
            "CREATE TABLE im_thread_sessions (
                thread_id TEXT NOT NULL,
                employee_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                route_session_key TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                channel TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (thread_id, employee_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_thread_sessions table");

        sqlx::query(
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                role_id TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        sqlx::query(
            "CREATE TABLE employee_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create employee_groups table");

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES
             ('session-general', 'skill-1', 'New Chat', '2026-03-14T00:00:00Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-general-generic-first', 'skill-1', 'New Chat', '2026-03-14T00:01:00Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-team', 'skill-1', 'New Chat', '2026-03-14T00:02:00Z', 'model-1', 'standard', '', '', 'team_entry', 'team-a'),
             ('session-employee', 'skill-1', 'New Chat', '2026-03-14T00:03:00Z', 'model-1', 'standard', '', 'emp-1', 'employee_direct', '')",
        )
        .execute(&pool)
        .await
        .expect("seed sessions");

        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at)
             VALUES
             ('msg-1', 'session-general', 'user', '帮我整理本周销售周报', '2026-03-14T00:00:01Z'),
             ('msg-2', 'session-general-generic-first', 'user', '你好', '2026-03-14T00:01:01Z'),
             ('msg-3', 'session-general-generic-first', 'user', '修复登录接口超时问题', '2026-03-14T00:01:02Z')",
        )
        .execute(&pool)
        .await
        .expect("seed messages");

        sqlx::query("INSERT INTO employee_groups (id, name) VALUES ('team-a', '市场协作')")
            .execute(&pool)
            .await
            .expect("seed employee_groups");

        sqlx::query(
            "INSERT INTO agent_employees (id, employee_id, name, role_id) VALUES ('employee-row-1', 'emp-1', '张三', 'role-1')",
        )
        .execute(&pool)
        .await
        .expect("seed agent_employees");

        let sessions = list_sessions_with_pool(&pool, permission_mode_label_for_display)
            .await
            .expect("list sessions should succeed");

        assert_eq!(sessions[0]["id"], "session-employee");
        assert_eq!(sessions[0]["display_title"], "张三");
        assert_eq!(sessions[0]["employee_name"], "张三");
        assert_eq!(sessions[1]["id"], "session-team");
        assert_eq!(sessions[1]["display_title"], "市场协作");
        assert_eq!(sessions[1]["employee_name"], "");
        assert_eq!(sessions[2]["id"], "session-general-generic-first");
        assert_eq!(sessions[2]["display_title"], "修复登录接口超时问题");
        assert_eq!(sessions[2]["employee_name"], "");
        assert_eq!(sessions[3]["id"], "session-general");
        assert_eq!(sessions[3]["display_title"], "帮我整理本周销售周报");
        assert_eq!(sessions[3]["employee_name"], "");
    }

    #[tokio::test]
    async fn list_sessions_with_pool_projects_runtime_status() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                model_id TEXT NOT NULL,
                permission_mode TEXT NOT NULL DEFAULT 'standard',
                work_dir TEXT NOT NULL DEFAULT '',
                employee_id TEXT NOT NULL DEFAULT '',
                session_mode TEXT NOT NULL DEFAULT 'general',
                team_id TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create sessions table");

        sqlx::query(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                content_json TEXT,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create messages table");

        sqlx::query(
            "CREATE TABLE im_thread_sessions (
                thread_id TEXT NOT NULL,
                employee_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                route_session_key TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                channel TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (thread_id, employee_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_thread_sessions table");

        sqlx::query(
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                role_id TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        sqlx::query(
            "CREATE TABLE employee_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create employee_groups table");

        sqlx::query(
            "CREATE TABLE session_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_message_id TEXT NOT NULL,
                assistant_message_id TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL,
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
            "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id)
             VALUES
             ('session-failed', 'skill-1', '失败会话', '2026-03-16T00:00:04Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-completed', 'skill-1', '完成会话', '2026-03-16T00:00:03Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-waiting', 'skill-1', '审批会话', '2026-03-16T00:00:02Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-running', 'skill-1', '运行会话', '2026-03-16T00:00:01Z', 'model-1', 'standard', '', '', 'general', ''),
             ('session-idle', 'skill-1', '空闲会话', '2026-03-16T00:00:00Z', 'model-1', 'standard', '', '', 'general', '')",
        )
        .execute(&pool)
        .await
        .expect("seed sessions");

        sqlx::query(
            "INSERT INTO session_runs (id, session_id, user_message_id, assistant_message_id, status, buffered_text, error_kind, error_message, created_at, updated_at)
             VALUES
             ('run-failed', 'session-failed', 'user-1', '', 'failed', '', 'billing', '额度不足', '2026-03-16T00:00:04Z', '2026-03-16T00:00:05Z'),
             ('run-completed', 'session-completed', 'user-2', 'assistant-2', 'completed', '已完成', '', '', '2026-03-16T00:00:03Z', '2026-03-16T00:00:04Z'),
             ('run-waiting', 'session-waiting', 'user-3', '', 'waiting_approval', '等待确认', '', '', '2026-03-16T00:00:02Z', '2026-03-16T00:00:06Z'),
             ('run-running', 'session-running', 'user-4', '', 'thinking', '执行中', '', '', '2026-03-16T00:00:01Z', '2026-03-16T00:00:07Z')",
        )
        .execute(&pool)
        .await
        .expect("seed session_runs");

        let sessions = list_sessions_with_pool(&pool, permission_mode_label_for_display)
            .await
            .expect("list sessions should succeed");

        assert_eq!(sessions[0]["id"], "session-failed");
        assert_eq!(sessions[0]["runtime_status"], "failed");
        assert_eq!(sessions[1]["id"], "session-completed");
        assert_eq!(sessions[1]["runtime_status"], "completed");
        assert_eq!(sessions[2]["id"], "session-waiting");
        assert_eq!(sessions[2]["runtime_status"], "waiting_approval");
        assert_eq!(sessions[3]["id"], "session-running");
        assert_eq!(sessions[3]["runtime_status"], "running");
        assert_eq!(sessions[4]["id"], "session-idle");
        assert!(sessions[4]["runtime_status"].is_null());
    }

    #[test]
    fn render_user_content_parts_formats_images_and_text_files() {
        let rendered = render_user_content_parts(
            &serde_json::to_string(&json!([
                { "type": "text", "text": "请结合附件分析" },
                { "type": "image", "name": "screen.png" },
                {
                    "type": "file_text",
                    "name": "debug.ts",
                    "mimeType": "text/plain",
                    "text": "console.log('hi')",
                    "truncated": true
                }
            ]))
            .expect("serialize content parts"),
        )
        .expect("render content parts");

        assert!(rendered.contains("请结合附件分析"));
        assert!(rendered.contains("[图片] screen.png"));
        assert!(rendered.contains("[文本附件] debug.ts (text/plain)"));
        assert!(rendered.contains("[内容已截断]"));
    }
}

fn render_user_content_parts(content_json: &str) -> Option<String> {
    let parts = serde_json::from_str::<Value>(content_json).ok()?;
    let items = parts.as_array()?;
    let mut sections = Vec::new();

    for part in items {
        match part.get("type").and_then(Value::as_str).unwrap_or_default() {
            "text" => {
                let text = part
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if !text.is_empty() {
                    sections.push(text.to_string());
                }
            }
            "image" => {
                let name = part.get("name").and_then(Value::as_str).unwrap_or("image");
                sections.push(format!("[图片] {name}"));
            }
            "file_text" => {
                let name = part
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("attachment.txt");
                let mime_type = part
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .unwrap_or("text/plain");
                let text = part.get("text").and_then(Value::as_str).unwrap_or("");
                let truncated = part
                    .get("truncated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let ext = name.rsplit('.').next().unwrap_or("txt");
                let note = if truncated { "\n[内容已截断]" } else { "" };
                sections.push(format!(
                    "[文本附件] {name} ({mime_type})\n```{ext}\n{text}\n```{note}"
                ));
            }
            _ => {}
        }
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}
