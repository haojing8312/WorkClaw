use crate::agent::ToolRegistry;
use crate::commands::mcp::add_mcp_server_with_registry;
use crate::commands::skills::DbState;
use crate::content_providers::{
    build_agent_reach_source_status, detect_agent_reach_mcp_servers, detect_agent_reach_provider,
    ContentCapability, DetectedExternalMcpServer, DiagnosticsRunner,
    ExternalCapabilitySourceStatus, ProcessDiagnosticsRunner, ProviderAvailability, ProviderStatus,
};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tauri::State;

fn builtin_web_provider_status() -> ProviderStatus {
    ProviderStatus {
        provider_id: "builtin-web".to_string(),
        availability: ProviderAvailability::Available,
        capabilities: vec![ContentCapability::ReadUrl, ContentCapability::SearchContent],
        detail: Some("Built-in HTTP reader/search fallback".to_string()),
    }
}

pub fn list_content_providers_for(runner: &dyn DiagnosticsRunner) -> Vec<ProviderStatus> {
    vec![
        builtin_web_provider_status(),
        detect_agent_reach_provider(runner),
    ]
}

pub fn run_content_provider_diagnostics_for(
    provider_id: &str,
    runner: &dyn DiagnosticsRunner,
) -> Result<ProviderStatus, String> {
    match provider_id {
        "builtin-web" => Ok(builtin_web_provider_status()),
        "agent-reach" => Ok(detect_agent_reach_provider(runner)),
        _ => Err(format!("unknown content provider: {provider_id}")),
    }
}

pub fn list_external_capability_sources_for(
    runner: &dyn DiagnosticsRunner,
) -> Vec<ExternalCapabilitySourceStatus> {
    vec![build_agent_reach_source_status(runner)]
}

pub fn list_detected_external_mcp_servers_for(
    runner: &dyn DiagnosticsRunner,
) -> Vec<DetectedExternalMcpServer> {
    detect_agent_reach_mcp_servers(runner)
}

pub fn mark_imported_external_mcp_servers(
    detected: Vec<DetectedExternalMcpServer>,
    imported_keys: &HashSet<(String, String)>,
) -> Vec<DetectedExternalMcpServer> {
    detected
        .into_iter()
        .map(|mut server| {
            server.managed_by_workclaw =
                imported_keys.contains(&(server.source_id.clone(), server.channel.clone()));
            server
        })
        .collect()
}

async fn persist_external_capability_sources_to_pool(
    pool: &sqlx::SqlitePool,
    sources: &[ExternalCapabilitySourceStatus],
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    for source in sources {
        for channel in &source.channels {
            sqlx::query(
                "INSERT INTO external_capability_channels (source_id, channel, backend_type, backend_name, last_status, detail, last_checked_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(source_id, channel) DO UPDATE SET
                   backend_type = excluded.backend_type,
                   backend_name = excluded.backend_name,
                   last_status = excluded.last_status,
                   detail = excluded.detail,
                   last_checked_at = excluded.last_checked_at",
            )
            .bind(&source.source_id)
            .bind(&channel.channel)
            .bind(&channel.backend_type)
            .bind(&channel.backend_name)
            .bind(&channel.status)
            .bind(channel.detail.clone().unwrap_or_default())
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("保存外部能力快照失败: {e}"))?;
        }
    }
    Ok(())
}

async fn persist_external_mcp_import_mapping_to_pool(
    pool: &sqlx::SqlitePool,
    server: &DetectedExternalMcpServer,
    mcp_server_id: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let fingerprint = format!(
        "{}|{}|{}",
        server.command,
        server.args.join(" "),
        server.backend_name
    );
    sqlx::query(
        "INSERT INTO external_mcp_imports (source_id, channel, detected_server_name, mcp_server_id, template_fingerprint, import_mode, imported_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 'safe_template', ?, ?)
         ON CONFLICT(source_id, channel) DO UPDATE SET
           detected_server_name = excluded.detected_server_name,
           mcp_server_id = excluded.mcp_server_id,
           template_fingerprint = excluded.template_fingerprint,
           import_mode = excluded.import_mode,
           updated_at = excluded.updated_at",
    )
    .bind(&server.source_id)
    .bind(&server.channel)
    .bind(&server.server_name)
    .bind(mcp_server_id)
    .bind(fingerprint)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("保存外部 MCP 导入映射失败: {e}"))?;
    Ok(())
}

pub fn is_known_safe_external_mcp_template(server: &DetectedExternalMcpServer) -> bool {
    matches!(
        (server.backend_name.as_str(), server.command.as_str()),
        ("mcporter", "mcporter") | ("linkedin-mcp", "linkedin-mcp")
    )
}

#[tauri::command]
pub async fn list_content_providers() -> Result<Vec<ProviderStatus>, String> {
    let runner = ProcessDiagnosticsRunner;
    Ok(list_content_providers_for(&runner))
}

#[tauri::command]
pub async fn run_content_provider_diagnostics(
    provider_id: String,
    db: State<'_, DbState>,
) -> Result<ProviderStatus, String> {
    let runner = ProcessDiagnosticsRunner;
    let result = run_content_provider_diagnostics_for(&provider_id, &runner)?;
    let sources = list_external_capability_sources_for(&runner);
    let _ = persist_external_capability_sources_to_pool(&db.0, &sources).await;
    Ok(result)
}

#[tauri::command]
pub async fn list_external_capability_sources(
    db: State<'_, DbState>,
) -> Result<Vec<ExternalCapabilitySourceStatus>, String> {
    let runner = ProcessDiagnosticsRunner;
    let sources = list_external_capability_sources_for(&runner);
    let _ = persist_external_capability_sources_to_pool(&db.0, &sources).await;
    Ok(sources)
}

#[tauri::command]
pub async fn list_detected_external_mcp_servers(
    db: State<'_, DbState>,
) -> Result<Vec<DetectedExternalMcpServer>, String> {
    let runner = ProcessDiagnosticsRunner;
    let imported_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT imports.source_id, imports.channel
         FROM external_mcp_imports AS imports
         INNER JOIN mcp_servers AS servers
           ON servers.id = imports.mcp_server_id
         WHERE servers.enabled = 1",
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| format!("读取已导入 MCP 服务器映射失败: {e}"))?;
    let imported_keys = imported_rows.into_iter().collect::<HashSet<_>>();
    Ok(mark_imported_external_mcp_servers(
        list_detected_external_mcp_servers_for(&runner),
        &imported_keys,
    ))
}

#[tauri::command]
pub async fn import_detected_external_mcp_server(
    server: DetectedExternalMcpServer,
    db: State<'_, DbState>,
    registry: State<'_, Arc<ToolRegistry>>,
) -> Result<String, String> {
    if !is_known_safe_external_mcp_template(&server) {
        return Err("unsupported external MCP template".to_string());
    }

    let server_name = server.server_name.clone();
    let command = server.command.clone();
    let args = server.args.clone();
    let env: HashMap<String, String> = server
        .env
        .iter()
        .map(|key| (key.clone(), String::new()))
        .collect();

    let id = add_mcp_server_with_registry(
        &db.0,
        Arc::clone(&registry.inner()),
        server_name,
        command,
        args,
        env,
    )
    .await?;

    persist_external_mcp_import_mapping_to_pool(&db.0, &server, &id).await?;

    Ok(id)
}
