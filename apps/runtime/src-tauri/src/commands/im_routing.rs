use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ImRoutingBinding {
    pub id: String,
    pub agent_id: String,
    pub channel: String,
    pub account_id: String,
    pub peer_kind: String,
    pub peer_id: String,
    pub guild_id: String,
    pub team_id: String,
    pub role_ids: Vec<String>,
    pub priority: i64,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct UpsertImRoutingBindingInput {
    pub id: Option<String>,
    pub agent_id: String,
    pub channel: String,
    pub account_id: String,
    pub peer_kind: String,
    pub peer_id: String,
    pub guild_id: String,
    pub team_id: String,
    pub role_ids: Vec<String>,
    pub priority: i64,
    pub enabled: bool,
}

fn normalize_role_ids(role_ids: &[String]) -> Vec<String> {
    role_ids
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .collect()
}

pub async fn list_im_routing_bindings_with_pool(
    pool: &SqlitePool,
) -> Result<Vec<ImRoutingBinding>, String> {
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
            i64,
            i64,
            String,
            String,
        ),
    >(
        "SELECT id, agent_id, channel, account_id, peer_kind, peer_id, guild_id, team_id, role_ids_json, priority, enabled, created_at, updated_at
         FROM im_routing_bindings
         ORDER BY priority ASC, updated_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(rows.len());
    for (
        id,
        agent_id,
        channel,
        account_id,
        peer_kind,
        peer_id,
        guild_id,
        team_id,
        role_ids_json,
        priority,
        enabled,
        created_at,
        updated_at,
    ) in rows
    {
        let role_ids =
            serde_json::from_str::<Vec<String>>(&role_ids_json).unwrap_or_else(|_| Vec::new());
        out.push(ImRoutingBinding {
            id,
            agent_id,
            channel,
            account_id,
            peer_kind,
            peer_id,
            guild_id,
            team_id,
            role_ids,
            priority,
            enabled: enabled != 0,
            created_at,
            updated_at,
        });
    }

    Ok(out)
}

pub async fn upsert_im_routing_binding_with_pool(
    pool: &SqlitePool,
    input: UpsertImRoutingBindingInput,
) -> Result<String, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agent_id is required".to_string());
    }
    let channel = input.channel.trim().to_lowercase();
    if channel.is_empty() {
        return Err("channel is required".to_string());
    }

    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = chrono::Utc::now().to_rfc3339();
    let role_ids_json =
        serde_json::to_string(&normalize_role_ids(&input.role_ids)).map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO im_routing_bindings (
            id, agent_id, channel, account_id, peer_kind, peer_id, guild_id, team_id,
            role_ids_json, priority, enabled, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            agent_id = excluded.agent_id,
            channel = excluded.channel,
            account_id = excluded.account_id,
            peer_kind = excluded.peer_kind,
            peer_id = excluded.peer_id,
            guild_id = excluded.guild_id,
            team_id = excluded.team_id,
            role_ids_json = excluded.role_ids_json,
            priority = excluded.priority,
            enabled = excluded.enabled,
            updated_at = excluded.updated_at",
    )
    .bind(&id)
    .bind(agent_id)
    .bind(&channel)
    .bind(input.account_id.trim())
    .bind(input.peer_kind.trim())
    .bind(input.peer_id.trim())
    .bind(input.guild_id.trim())
    .bind(input.team_id.trim())
    .bind(role_ids_json)
    .bind(input.priority)
    .bind(if input.enabled { 1 } else { 0 })
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(id)
}

pub async fn delete_im_routing_binding_with_pool(
    pool: &SqlitePool,
    id: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM im_routing_bindings WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_im_routing_bindings(
    db: State<'_, DbState>,
) -> Result<Vec<ImRoutingBinding>, String> {
    list_im_routing_bindings_with_pool(&db.0).await
}

#[tauri::command]
pub async fn upsert_im_routing_binding(
    input: UpsertImRoutingBindingInput,
    db: State<'_, DbState>,
) -> Result<String, String> {
    upsert_im_routing_binding_with_pool(&db.0, input).await
}

#[tauri::command]
pub async fn delete_im_routing_binding(id: String, db: State<'_, DbState>) -> Result<(), String> {
    delete_im_routing_binding_with_pool(&db.0, &id).await
}
