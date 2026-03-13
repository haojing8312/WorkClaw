use super::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::commands::chat_runtime_io::extract_assistant_text_content;
use crate::session_journal::{SessionJournalState, SessionJournalStore, SessionRunStatus};
use chrono::Utc;
use runtime_chat_app::{ChatPreparationService, SessionCreationRequest};
use serde_json::{json, Value};
use uuid::Uuid;

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
    let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
        "SELECT
            m.id,
            m.role,
            m.content,
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
        .map(|(id, role, content, created_at, run_id)| {
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
            json!({
                "id": id,
                "role": role,
                "content": content,
                "created_at": created_at,
                "runId": run_id,
            })
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
            String,
            String,
        ),
    >(
        "SELECT
            s.id,
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

    Ok(rows
        .iter()
        .map(
            |(
                id,
                title,
                created_at,
                model_id,
                work_dir,
                employee_id,
                permission_mode,
                session_mode,
                team_id,
                im_source_channel,
            )| {
                let (source_channel, source_label) =
                    resolve_im_session_source(Some(im_source_channel));
                json!({
                    "id": id,
                    "title": title,
                    "created_at": created_at,
                    "model_id": model_id,
                    "work_dir": work_dir,
                    "employee_id": employee_id,
                    "permission_mode": permission_mode,
                    "session_mode": session_mode,
                    "team_id": team_id,
                    "permission_mode_label": permission_mode_label_for_display(permission_mode),
                    "source_channel": source_channel,
                    "source_label": source_label,
                })
            },
        )
        .collect())
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
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT DISTINCT
            s.id,
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
            |(id, title, created_at, model_id, work_dir, employee_id, im_source_channel)| {
                let (source_channel, source_label) =
                    resolve_im_session_source(Some(im_source_channel));
                json!({
                    "id": id,
                    "title": title,
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

    let messages = sqlx::query_as::<_, (String, String, String)>(
        "SELECT role, content, created_at FROM messages WHERE session_id = ? ORDER BY created_at ASC"
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut md = format!("# {}\n\n", title);
    for (role, content, created_at) in &messages {
        let label = if role == "user" { "用户" } else { "助手" };
        let rendered_content = render_export_message_content(role, content);
        md.push_str(&format!(
            "## {} ({})\n\n{}\n\n---\n\n",
            label, created_at, rendered_content
        ));
    }

    if let Some(journal_store) = journal {
        if let Ok(state) = journal_store.read_state(session_id).await {
            let recovered = render_recovered_run_sections(&messages, &state);
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
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let messages: Vec<Value> = rows
        .iter()
        .map(|(role, content)| {
            let normalized_content = if role == "assistant" {
                extract_assistant_text_content(content)
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
            "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(msg["role"].as_str().unwrap_or("user"))
        .bind(msg["content"].as_str().unwrap_or(""))
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

fn render_export_message_content(role: &str, content: &str) -> String {
    if role != "assistant" {
        return content.to_string();
    }

    let Ok(parsed) = serde_json::from_str::<Value>(content) else {
        return content.to_string();
    };

    let mut sections: Vec<String> = Vec::new();
    let final_text = parsed["text"].as_str().unwrap_or("").trim();
    if !final_text.is_empty() {
        sections.push(final_text.to_string());
    }

    if let Some(items) = parsed["items"].as_array() {
        let item_text = items
            .iter()
            .filter_map(|item| {
                if item["type"].as_str() == Some("text") {
                    return item["content"]
                        .as_str()
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_string);
                }
                None
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !item_text.is_empty() && !sections.iter().any(|section| section.contains(&item_text)) {
            sections.push(item_text);
        }
    }

    if sections.is_empty() {
        content.to_string()
    } else {
        sections.join("\n\n")
    }
}

fn render_recovered_run_sections(
    messages: &[(String, String, String)],
    state: &SessionJournalState,
) -> String {
    let assistant_contents: Vec<&str> = messages
        .iter()
        .filter_map(|(role, content, _)| (role == "assistant").then_some(content.as_str()))
        .collect();

    let mut sections = Vec::new();
    for run in &state.runs {
        let buffered = run.buffered_text.trim();
        let error_message = run.last_error_message.as_deref().unwrap_or("").trim();
        let buffered_already_exported = !buffered.is_empty()
            && assistant_contents
                .iter()
                .any(|content| content.contains(buffered));
        let error_already_exported = !error_message.is_empty()
            && assistant_contents
                .iter()
                .any(|content| content.contains(error_message));
        let should_recover = (!buffered.is_empty() && !buffered_already_exported)
            || (!error_message.is_empty() && !error_already_exported)
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
        sections.push("\n---\n".to_string());
    }

    sections.join("\n")
}

fn export_status_label(status: &SessionRunStatus) -> &'static str {
    match status {
        SessionRunStatus::Queued => "queued",
        SessionRunStatus::Thinking => "thinking",
        SessionRunStatus::ToolCalling => "tool_calling",
        SessionRunStatus::WaitingUser => "waiting_user",
        SessionRunStatus::Completed => "completed",
        SessionRunStatus::Failed => "failed",
        SessionRunStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_im_session_source;

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
}
