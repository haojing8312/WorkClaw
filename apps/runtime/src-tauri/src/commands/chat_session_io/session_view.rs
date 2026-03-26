use crate::commands::chat_runtime_io::{
    derive_meaningful_session_title_from_messages, is_generic_session_title,
};
use serde_json::{json, Value};
use std::collections::HashMap;

pub(crate) fn resolve_im_session_source(channel: Option<&str>) -> (String, String) {
    match channel.unwrap_or("").trim() {
        "wecom" => ("wecom".to_string(), "企业微信".to_string()),
        "feishu" => ("feishu".to_string(), "飞书".to_string()),
        other if other.is_empty() => ("local".to_string(), String::new()),
        other => (other.to_string(), other.to_string()),
    }
}

pub(crate) async fn im_thread_sessions_has_channel_column(pool: &sqlx::SqlitePool) -> bool {
    matches!(
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('im_thread_sessions') WHERE name = 'channel'",
        )
        .fetch_optional(pool)
        .await,
        Ok(Some(_))
    )
}

pub(crate) async fn derive_session_display_title_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    persisted_title: &str,
    session_mode: &str,
    employee_id: &str,
    team_id: &str,
    employee_name_by_code: &HashMap<String, String>,
    team_name_by_id: &HashMap<String, String>,
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

pub(crate) fn normalize_stream_items(items: &Value) -> Value {
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

pub(crate) fn render_user_content_parts(content_json: &str) -> Option<String> {
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
            "pdf_file" => {
                let name = part
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("attachment.pdf");
                let mime_type = part
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .unwrap_or("application/pdf");
                let text = part
                    .get("extractedText")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let truncated = part
                    .get("truncated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let note = if truncated { "\n[内容已截断]" } else { "" };
                sections.push(format!(
                    "[PDF 附件] {name} ({mime_type})\n```text\n{text}\n```{note}"
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

#[cfg(test)]
mod tests {
    use super::{render_user_content_parts, resolve_im_session_source};
    use serde_json::json;

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

    #[test]
    fn render_user_content_parts_formats_images_text_files_and_pdf_files() {
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
                },
                {
                    "type": "pdf_file",
                    "name": "brief.pdf",
                    "mimeType": "application/pdf",
                    "extractedText": "这是 PDF 正文",
                    "truncated": true
                }
            ]))
            .expect("serialize content parts"),
        )
        .expect("render content parts");

        assert!(rendered.contains("请结合附件分析"));
        assert!(rendered.contains("[图片] screen.png"));
        assert!(rendered.contains("[文本附件] debug.ts (text/plain)"));
        assert!(rendered.contains("[PDF 附件] brief.pdf (application/pdf)"));
        assert!(rendered.contains("这是 PDF 正文"));
        assert!(rendered.contains("[内容已截断]"));
    }
}
