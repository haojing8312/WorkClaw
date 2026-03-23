use super::extract_assistant_text_content;

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
