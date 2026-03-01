pub mod agent;
pub mod sidecar;
pub mod providers;
mod adapters;
mod builtin_skills;
pub mod commands;
mod db;

use agent::{AgentExecutor, ToolRegistry};
use agent::tools::new_responder;
use agent::tools::search_providers::cache::SearchCache;
use commands::chat::{AskUserState, CancelFlagState, ToolConfirmState, ToolConfirmResponder, SearchCacheState};
use commands::skills::DbState;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 初始化数据库
            let pool = tauri::async_runtime::block_on(db::init_db(app.handle()))
                .expect("failed to init db");
            let pool_for_mcp = pool.clone();
            app.manage(DbState(pool));

            // 初始化 AgentExecutor（包含标准工具集）
            let registry = Arc::new(ToolRegistry::with_standard_tools());
            let agent_executor = Arc::new(AgentExecutor::new(Arc::clone(&registry)));
            app.manage(agent_executor);
            app.manage(Arc::clone(&registry));

            // 创建全局的 AskUser 和 ToolConfirm 响应通道（只创建一次）
            let ask_user_responder = new_responder();
            app.manage(AskUserState(ask_user_responder));
            let tool_confirm_responder: ToolConfirmResponder =
                std::sync::Arc::new(std::sync::Mutex::new(None));
            app.manage(ToolConfirmState(tool_confirm_responder));

            // 创建全局取消标志
            let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
            app.manage(CancelFlagState(cancel_flag));

            // 创建全局搜索缓存（15 分钟 TTL，最多 100 条）
            let search_cache = Arc::new(SearchCache::new(
                std::time::Duration::from_secs(900),
                100,
            ));
            app.manage(SearchCacheState(search_cache));

            // 恢复已保存的 MCP 服务器连接
            let registry_for_mcp = Arc::clone(&registry);
            tauri::async_runtime::spawn(async move {
                let servers = sqlx::query_as::<_, (String, String, String, String)>(
                    "SELECT name, command, args, env FROM mcp_servers WHERE enabled = 1"
                )
                .fetch_all(&pool_for_mcp)
                .await
                .unwrap_or_default();

                if servers.is_empty() {
                    return;
                }

                let client = reqwest::Client::new();
                for (name, command, args_json, env_json) in servers {
                    let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();
                    let env: std::collections::HashMap<String, String> =
                        serde_json::from_str(&env_json).unwrap_or_default();

                    // 连接 MCP 服务器
                    let connect_result = client.post("http://localhost:8765/api/mcp/add-server")
                        .json(&serde_json::json!({
                            "name": name,
                            "config": { "command": command, "args": args, "env": env }
                        }))
                        .send()
                        .await;

                    if connect_result.is_err() {
                        eprintln!("[mcp] 连接 MCP 服务器 {} 失败（Sidecar 可能未启动）", name);
                        continue;
                    }

                    // 获取工具列表并注册
                    if let Ok(resp) = client.post("http://localhost:8765/api/mcp/list-tools")
                        .json(&serde_json::json!({ "serverName": name }))
                        .send()
                        .await
                    {
                        if let Ok(body) = resp.json::<serde_json::Value>().await {
                            if let Some(tool_list) = body["tools"].as_array() {
                                for tool in tool_list {
                                    let tool_name = tool["name"].as_str().unwrap_or_default();
                                    let tool_desc = tool["description"].as_str().unwrap_or_default();
                                    let schema = tool.get("inputSchema").cloned()
                                        .unwrap_or(serde_json::json!({"type": "object", "properties": {}}));

                                    let full_name = format!("mcp_{}_{}", name, tool_name);
                                    registry_for_mcp.register(Arc::new(
                                        agent::tools::SidecarBridgeTool::new_mcp(
                                            "http://localhost:8765".to_string(),
                                            full_name,
                                            tool_desc.to_string(),
                                            schema,
                                            name.clone(),
                                            tool_name.to_string(),
                                        )
                                    ));
                                }
                                eprintln!("[mcp] 已恢复 MCP 服务器 {} 的工具注册", name);
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::skills::install_skill,
            commands::skills::import_local_skill,
            commands::skills::refresh_local_skill,
            commands::skills::create_local_skill,
            commands::skills::render_local_skill_preview,
            commands::skills::list_skills,
            commands::skills::delete_skill,
            commands::models::save_model_config,
            commands::models::list_model_configs,
            commands::models::get_model_api_key,
            commands::models::delete_model_config,
            commands::models::test_connection_cmd,
            commands::models::save_provider_config,
            commands::models::list_provider_configs,
            commands::models::delete_provider_config,
            commands::models::set_chat_routing_policy,
            commands::models::get_chat_routing_policy,
            commands::models::set_capability_routing_policy,
            commands::models::get_capability_routing_policy,
            commands::models::test_provider_health,
            commands::models::test_all_provider_health,
            commands::models::list_provider_recommended_models,
            commands::models::list_provider_models,
            commands::models::list_capability_route_templates,
            commands::models::apply_capability_route_template,
            commands::models::list_recent_route_attempt_logs,
            commands::models::list_route_attempt_stats,
            commands::models::export_route_attempt_logs_csv,
            commands::models::list_search_configs,
            commands::models::test_search_connection,
            commands::models::set_default_search,
            commands::models::list_builtin_provider_plugins,
            commands::models::get_routing_settings,
            commands::models::set_routing_settings,
            commands::chat::create_session,
            commands::chat::send_message,
            commands::chat::get_messages,
            commands::chat::get_sessions,
            commands::chat::delete_session,
            commands::chat::update_session_workspace,
            commands::chat::search_sessions,
            commands::chat::export_session,
            commands::chat::write_export_file,
            commands::chat::answer_user_question,
            commands::chat::confirm_tool_execution,
            commands::chat::cancel_agent,
            commands::chat::compact_context,
            commands::mcp::add_mcp_server,
            commands::mcp::list_mcp_servers,
            commands::mcp::remove_mcp_server,
            commands::dialog::select_directory,
            commands::packaging::read_skill_dir,
            commands::packaging::pack_skill,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
