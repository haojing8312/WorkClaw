use sqlx::SqlitePool;

use super::{normalize_required, OpenClawPluginInstallInput, OpenClawPluginInstallRecord};

type InstallRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);

fn normalize_manifest_json(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok("{}".to_string());
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|e| format!("manifest_json must be valid json: {e}"))?;
    Ok(trimmed.to_string())
}

fn record_from_row(row: InstallRow) -> OpenClawPluginInstallRecord {
    OpenClawPluginInstallRecord {
        plugin_id: row.0,
        npm_spec: row.1,
        version: row.2,
        install_path: row.3,
        source_type: row.4,
        manifest_json: row.5,
        installed_at: row.6,
        updated_at: row.7,
    }
}

pub async fn upsert_openclaw_plugin_install_with_pool(
    pool: &SqlitePool,
    input: OpenClawPluginInstallInput,
) -> Result<OpenClawPluginInstallRecord, String> {
    let plugin_id = normalize_required(&input.plugin_id, "plugin_id")?;
    let npm_spec = normalize_required(&input.npm_spec, "npm_spec")?;
    let version = normalize_required(&input.version, "version")?;
    let install_path = normalize_required(&input.install_path, "install_path")?;
    let source_type = normalize_required(&input.source_type, "source_type")?;
    let manifest_json = normalize_manifest_json(&input.manifest_json)?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO installed_openclaw_plugins (
            plugin_id,
            npm_spec,
            version,
            install_path,
            source_type,
            manifest_json,
            installed_at,
            updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(plugin_id) DO UPDATE SET
            npm_spec = excluded.npm_spec,
            version = excluded.version,
            install_path = excluded.install_path,
            source_type = excluded.source_type,
            manifest_json = excluded.manifest_json,
            updated_at = excluded.updated_at",
    )
    .bind(&plugin_id)
    .bind(&npm_spec)
    .bind(&version)
    .bind(&install_path)
    .bind(&source_type)
    .bind(&manifest_json)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    get_openclaw_plugin_install_by_id_with_pool(pool, &plugin_id).await
}

pub async fn list_openclaw_plugin_installs_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<OpenClawPluginInstallRecord>, String> {
    let rows = sqlx::query_as::<_, InstallRow>(
        "SELECT plugin_id, npm_spec, version, install_path, source_type, manifest_json, installed_at, updated_at
         FROM installed_openclaw_plugins
         ORDER BY installed_at DESC, plugin_id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(record_from_row).collect())
}

pub async fn delete_openclaw_plugin_install_with_pool(
    pool: &SqlitePool,
    plugin_id: &str,
) -> Result<(), String> {
    let normalized = normalize_required(plugin_id, "plugin_id")?;
    sqlx::query("DELETE FROM installed_openclaw_plugins WHERE plugin_id = ?")
        .bind(normalized)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn get_openclaw_plugin_install_by_id_with_pool(
    pool: &SqlitePool,
    plugin_id: &str,
) -> Result<OpenClawPluginInstallRecord, String> {
    let normalized = normalize_required(plugin_id, "plugin_id")?;
    let row = sqlx::query_as::<_, InstallRow>(
        "SELECT plugin_id, npm_spec, version, install_path, source_type, manifest_json, installed_at, updated_at
         FROM installed_openclaw_plugins
         WHERE plugin_id = ?
         LIMIT 1",
    )
    .bind(&normalized)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("openclaw plugin install not found: {normalized}"))?;

    Ok(record_from_row(row))
}
