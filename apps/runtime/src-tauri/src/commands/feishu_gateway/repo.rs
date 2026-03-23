use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

pub async fn get_app_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>, String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = ? LIMIT 1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(row.map(|(v,)| v))
}

pub async fn set_app_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<(), String> {
    sqlx::query("INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn resolve_feishu_sidecar_base_url(
    pool: &SqlitePool,
    input: Option<String>,
) -> Result<Option<String>, String> {
    if let Some(v) = input {
        if !v.trim().is_empty() {
            return Ok(Some(v));
        }
    }
    let feishu_specific = get_app_setting(pool, "feishu_sidecar_base_url").await?;
    if feishu_specific
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        return Ok(feishu_specific);
    }
    Ok(get_app_setting(pool, "im_sidecar_base_url").await?)
}

pub async fn resolve_feishu_app_credentials(
    pool: &SqlitePool,
    app_id: Option<String>,
    app_secret: Option<String>,
) -> Result<(Option<String>, Option<String>), String> {
    if let (Some(id), Some(secret)) = (app_id.clone(), app_secret.clone()) {
        if !id.trim().is_empty() && !secret.trim().is_empty() {
            return Ok((Some(id), Some(secret)));
        }
    }

    let employee_creds = sqlx::query_as::<_, (String, String)>(
        "SELECT feishu_app_id, feishu_app_secret
         FROM agent_employees
         WHERE enabled = 1
           AND TRIM(feishu_app_id) <> ''
           AND TRIM(feishu_app_secret) <> ''
         ORDER BY is_default DESC, updated_at DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((id, secret)) = employee_creds {
        return Ok((Some(id), Some(secret)));
    }

    // Backward compatibility: legacy global settings fallback.
    let resolved_app_id = get_app_setting(pool, "feishu_app_id").await?;
    let resolved_app_secret = get_app_setting(pool, "feishu_app_secret").await?;
    Ok((resolved_app_id, resolved_app_secret))
}

pub fn calculate_feishu_signature(
    timestamp: &str,
    nonce: &str,
    encrypt_key: &str,
    body: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}{}{}{}", timestamp, nonce, encrypt_key, body));
    let digest = hasher.finalize();
    format!("{:x}", digest)
}
