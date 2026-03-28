use super::extract_assistant_text_content;

async fn maybe_self_heal_builtin_skill_source_with_pool(
    pool: &sqlx::SqlitePool,
    skill_id: &str,
    username: &str,
    pack_path: &str,
    source_type: &str,
) -> Result<(String, String, String), String> {
    if source_type != "builtin" {
        return Ok((
            username.to_string(),
            pack_path.to_string(),
            source_type.to_string(),
        ));
    }

    let pack_root = std::path::Path::new(pack_path);
    if pack_path.trim().is_empty() || !pack_root.exists() {
        return Ok((
            username.to_string(),
            pack_path.to_string(),
            source_type.to_string(),
        ));
    }

    sqlx::query(
        "UPDATE installed_skills
         SET username = '', source_type = 'vendored'
         WHERE id = ? AND COALESCE(source_type, 'encrypted') = 'builtin'",
    )
    .bind(skill_id)
    .execute(pool)
    .await
    .map_err(|e| format!("自愈 legacy builtin skill 失败 (skill_id={skill_id}): {e}"))?;

    Ok(("".to_string(), pack_path.to_string(), "vendored".to_string()))
}

pub(crate) async fn load_session_runtime_inputs_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<(String, String, String, String, String), String> {
    let (skill_id, model_id, permission_mode, work_dir, employee_id) =
        sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT skill_id, model_id, permission_mode, COALESCE(work_dir, ''), COALESCE(employee_id, '') FROM sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("会话不存在 (session_id={session_id}): {e}"))?;

    let current_model_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(1) FROM model_configs WHERE id = ?",
    )
    .bind(&model_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("校验会话模型失败 (session_id={session_id}, model_id={model_id}): {e}"))?
        > 0;

    if current_model_exists {
        return Ok((skill_id, model_id, permission_mode, work_dir, employee_id));
    }

    let fallback_model_id = sqlx::query_scalar::<_, String>(
        "SELECT id
         FROM model_configs
         WHERE api_format NOT LIKE 'search_%' AND is_default = 1
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("读取默认模型失败 (session_id={session_id}): {e}"))?;
    let fallback_model_id = match fallback_model_id {
        Some(model_id) => model_id,
        None => sqlx::query_scalar::<_, String>(
            "SELECT id
             FROM model_configs
             WHERE api_format NOT LIKE 'search_%'
             ORDER BY rowid ASC
             LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("读取兜底模型失败 (session_id={session_id}): {e}"))?
        .ok_or_else(|| {
        format!(
            "会话模型不存在且没有可回退的默认模型 (session_id={session_id}, model_id={model_id})"
        )
        })?,
    };

    sqlx::query("UPDATE sessions SET model_id = ? WHERE id = ?")
        .bind(&fallback_model_id)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| format!("自愈会话模型失败 (session_id={session_id}): {e}"))?;

    Ok((
        skill_id,
        fallback_model_id,
        permission_mode,
        work_dir,
        employee_id,
    ))
}

pub(crate) async fn load_installed_skill_source_with_pool(
    pool: &sqlx::SqlitePool,
    skill_id: &str,
) -> Result<(String, String, String, String), String> {
    let (manifest, username, pack_path, source_type) = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT manifest, username, pack_path, COALESCE(source_type, 'encrypted') FROM installed_skills WHERE id = ?",
    )
    .bind(skill_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Skill 不存在 (skill_id={skill_id}): {e}"))?;

    let (username, pack_path, source_type) = maybe_self_heal_builtin_skill_source_with_pool(
        pool,
        skill_id,
        &username,
        &pack_path,
        &source_type,
    )
    .await?;

    Ok((manifest, username, pack_path, source_type))
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

#[cfg(test)]
mod tests {
    use super::{load_installed_skill_source_with_pool, load_session_runtime_inputs_with_pool};
    use chrono::Utc;
    use skillpack_rs::SkillManifest;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn setup_memory_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

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
        .expect("create installed_skills table");

        sqlx::query(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                permission_mode TEXT NOT NULL DEFAULT 'standard',
                work_dir TEXT NOT NULL DEFAULT '',
                employee_id TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create sessions table");

        sqlx::query(
            "CREATE TABLE model_configs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                api_format TEXT NOT NULL,
                base_url TEXT NOT NULL,
                model_name TEXT NOT NULL,
                is_default INTEGER DEFAULT 0,
                api_key TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create model_configs table");

        pool
    }

    #[tokio::test]
    async fn load_installed_skill_source_self_heals_builtin_rows_with_existing_pack_path() {
        let pool = setup_memory_pool().await;
        let vendor_root = tempdir().expect("create vendor root");
        let skill_dir = vendor_root.path().join("builtin-general");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(skill_dir.join("SKILL.md"), "# Builtin").expect("write skill markdown");

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

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("builtin-general")
        .bind(serde_json::to_string(&manifest).unwrap())
        .bind(Utc::now().to_rfc3339())
        .bind("legacy-user")
        .bind(skill_dir.to_string_lossy().to_string())
        .bind("builtin")
        .execute(&pool)
        .await
        .expect("insert legacy builtin row");

        let (_, username, pack_path, source_type) =
            load_installed_skill_source_with_pool(&pool, "builtin-general")
                .await
                .expect("load builtin skill source");

        assert_eq!(username, "");
        assert_eq!(pack_path, skill_dir.to_string_lossy());
        assert_eq!(source_type, "vendored");

        let (stored_source_type, stored_username): (String, String) = sqlx::query_as(
            "SELECT source_type, username FROM installed_skills WHERE id = 'builtin-general'",
        )
        .fetch_one(&pool)
        .await
        .expect("query self-healed row");
        assert_eq!(stored_source_type, "vendored");
        assert_eq!(stored_username, "");
    }

    #[tokio::test]
    async fn load_installed_skill_source_keeps_legacy_builtin_rows_without_pack_path() {
        let pool = setup_memory_pool().await;
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

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("builtin-general")
        .bind(serde_json::to_string(&manifest).unwrap())
        .bind(Utc::now().to_rfc3339())
        .bind("legacy-user")
        .bind("")
        .bind("builtin")
        .execute(&pool)
        .await
        .expect("insert legacy builtin row");

        let (_, username, pack_path, source_type) =
            load_installed_skill_source_with_pool(&pool, "builtin-general")
                .await
                .expect("load builtin skill source");

        assert_eq!(username, "legacy-user");
        assert_eq!(pack_path, "");
        assert_eq!(source_type, "builtin");
    }

    #[tokio::test]
    async fn load_session_runtime_inputs_self_heals_missing_model_to_the_default_model() {
        let pool = setup_memory_pool().await;

        sqlx::query(
            "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
             VALUES ('model-default', 'Default', 'openai', 'https://example.com', 'gpt-test', 1, 'sk-default')",
        )
        .execute(&pool)
        .await
        .expect("insert default model");

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, model_id, permission_mode, work_dir, employee_id)
             VALUES ('session-a', 'builtin-general', 'model-missing', 'standard', '', '')",
        )
        .execute(&pool)
        .await
        .expect("insert session");

        let (_, model_id, _, _, _) = load_session_runtime_inputs_with_pool(&pool, "session-a")
            .await
            .expect("load session runtime inputs");

        assert_eq!(model_id, "model-default");

        let (stored_model_id,): (String,) =
            sqlx::query_as("SELECT model_id FROM sessions WHERE id = 'session-a'")
                .fetch_one(&pool)
                .await
                .expect("query healed session");
        assert_eq!(stored_model_id, "model-default");
    }

    #[tokio::test]
    async fn load_session_runtime_inputs_keeps_existing_model_when_it_is_still_valid() {
        let pool = setup_memory_pool().await;

        sqlx::query(
            "INSERT INTO model_configs (id, name, api_format, base_url, model_name, is_default, api_key)
             VALUES ('model-a', 'Model A', 'openai', 'https://example.com', 'gpt-test', 1, 'sk-a')",
        )
        .execute(&pool)
        .await
        .expect("insert model");

        sqlx::query(
            "INSERT INTO sessions (id, skill_id, model_id, permission_mode, work_dir, employee_id)
             VALUES ('session-a', 'builtin-general', 'model-a', 'standard', '', '')",
        )
        .execute(&pool)
        .await
        .expect("insert session");

        let (_, model_id, _, _, _) = load_session_runtime_inputs_with_pool(&pool, "session-a")
            .await
            .expect("load session runtime inputs");

        assert_eq!(model_id, "model-a");
    }
}
