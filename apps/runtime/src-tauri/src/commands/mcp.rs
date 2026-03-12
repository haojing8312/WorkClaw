use super::skills::DbState;
use crate::agent::tools::SidecarBridgeTool;
use crate::agent::ToolRegistry;
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

pub async fn add_mcp_server_with_registry(
    pool: &sqlx::SqlitePool,
    registry: Arc<ToolRegistry>,
    name: String,
    command: String,
    args: Vec<String>,
    env: std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // 保存到数据库
    sqlx::query(
        "INSERT INTO mcp_servers (id, name, command, args, env, enabled, created_at) VALUES (?, ?, ?, ?, ?, 1, ?)"
    )
    .bind(&id)
    .bind(&name)
    .bind(&command)
    .bind(serde_json::to_string(&args).unwrap_or_default())
    .bind(serde_json::to_string(&env).unwrap_or_default())
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 通知 Sidecar 连接 MCP 服务器
    let client = reqwest::Client::new();
    let connect_resp = client
        .post("http://localhost:8765/api/mcp/add-server")
        .json(&json!({
            "name": name,
            "config": {
                "command": command,
                "args": args,
                "env": env,
            }
        }))
        .send()
        .await
        .map_err(|e| format!("连接 Sidecar 失败: {}", e))?;

    if !connect_resp.status().is_success() {
        // 连接失败时回滚数据库记录
        let _ = sqlx::query("DELETE FROM mcp_servers WHERE id = ?")
            .bind(&id)
            .execute(pool)
            .await;
        return Err("MCP 服务器连接失败".to_string());
    }

    // 获取工具列表并注册
    let tools_resp = client
        .post("http://localhost:8765/api/mcp/list-tools")
        .json(&json!({ "serverName": name }))
        .send()
        .await
        .map_err(|e| format!("获取工具列表失败: {}", e))?;

    let tools_body: Value = tools_resp.json().await.map_err(|e| e.to_string())?;

    if let Some(tool_list) = tools_body["tools"].as_array() {
        for tool in tool_list {
            let tool_name = tool["name"].as_str().unwrap_or_default();
            let tool_desc = tool["description"].as_str().unwrap_or_default();
            let schema = tool
                .get("inputSchema")
                .cloned()
                .unwrap_or(json!({"type": "object", "properties": {}}));

            let full_name = format!("mcp_{}_{}", name, tool_name);
            registry.register(Arc::new(SidecarBridgeTool::new_mcp(
                "http://localhost:8765".to_string(),
                full_name,
                tool_desc.to_string(),
                schema,
                name.clone(),
                tool_name.to_string(),
            )));
        }
    }

    Ok(id)
}

#[tauri::command]
pub async fn add_mcp_server(
    name: String,
    command: String,
    args: Vec<String>,
    env: std::collections::HashMap<String, String>,
    db: State<'_, DbState>,
    registry: State<'_, Arc<ToolRegistry>>,
) -> Result<String, String> {
    add_mcp_server_with_registry(
        &db.0,
        Arc::clone(&registry.inner()),
        name,
        command,
        args,
        env,
    )
    .await
}

#[tauri::command]
pub async fn list_mcp_servers(db: State<'_, DbState>) -> Result<Vec<Value>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, i32, String)>(
        "SELECT id, name, command, args, env, enabled, created_at FROM mcp_servers ORDER BY created_at DESC"
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|(id, name, command, args, env, enabled, created_at)| {
            json!({
                "id": id,
                "name": name,
                "command": command,
                "args": serde_json::from_str::<Value>(args).unwrap_or(json!([])),
                "env": serde_json::from_str::<Value>(env).unwrap_or(json!({})),
                "enabled": enabled == &1,
                "created_at": created_at,
            })
        })
        .collect())
}

#[tauri::command]
pub async fn remove_mcp_server(
    id: String,
    db: State<'_, DbState>,
    registry: State<'_, Arc<ToolRegistry>>,
) -> Result<(), String> {
    // 获取 server name
    let (name,): (String,) = sqlx::query_as("SELECT name FROM mcp_servers WHERE id = ?")
        .bind(&id)
        .fetch_one(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    // 从 registry 反注册所有该服务器的工具
    let prefix = format!("mcp_{}_", name);
    let tool_names = registry.tools_with_prefix(&prefix);
    for tool_name in tool_names {
        registry.unregister(&tool_name);
    }

    // 从数据库删除
    sqlx::query("DELETE FROM mcp_servers WHERE id = ?")
        .bind(&id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
