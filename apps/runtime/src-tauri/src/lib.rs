mod adapters;
pub mod agent;
mod builtin_skills;
pub mod commands;
mod db;
pub mod im;
pub mod providers;
pub mod sidecar;

use agent::tools::new_responder;
use agent::tools::search_providers::cache::SearchCache;
use agent::{AgentExecutor, ToolRegistry};
use commands::chat::{
    AskUserState, CancelFlagState, SearchCacheState, ToolConfirmResponder, ToolConfirmState,
};
use commands::skills::DbState;
use sidecar::SidecarManager;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            // 初始化数据库
            let pool = tauri::async_runtime::block_on(db::init_db(app.handle()))
                .expect("failed to init db");
            let pool_for_mcp = pool.clone();
            app.manage(DbState(pool.clone()));

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
            let search_cache = Arc::new(SearchCache::new(std::time::Duration::from_secs(900), 100));
            app.manage(SearchCacheState(search_cache));
            let sidecar_manager = Arc::new(SidecarManager::new());
            app.manage(sidecar_manager.clone());
            let feishu_relay_state = commands::feishu_gateway::FeishuEventRelayState::default();
            app.manage(feishu_relay_state.clone());

            // 启动 Sidecar（重试），确保后续 Feishu/MCP 调用有可用网关。
            let sidecar_for_boot = sidecar_manager.clone();
            std::thread::spawn(move || {
                tauri::async_runtime::block_on(async move {
                    for i in 0..20 {
                        if sidecar_for_boot.health_check().await.is_ok() {
                            break;
                        }
                        if let Err(e) = sidecar_for_boot.start().await {
                            eprintln!("[sidecar] start attempt {} failed: {}", i + 1, e);
                        } else {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                });
            });

            // 恢复已保存的 MCP 服务器连接
            let registry_for_mcp = Arc::clone(&registry);
            tauri::async_runtime::spawn(async move {
                let servers = sqlx::query_as::<_, (String, String, String, String)>(
                    "SELECT name, command, args, env FROM mcp_servers WHERE enabled = 1",
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
                    let connect_result = client
                        .post("http://localhost:8765/api/mcp/add-server")
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
                    if let Ok(resp) = client
                        .post("http://localhost:8765/api/mcp/list-tools")
                        .json(&serde_json::json!({ "serverName": name }))
                        .send()
                        .await
                    {
                        if let Ok(body) = resp.json::<serde_json::Value>().await {
                            if let Some(tool_list) = body["tools"].as_array() {
                                for tool in tool_list {
                                    let tool_name = tool["name"].as_str().unwrap_or_default();
                                    let tool_desc =
                                        tool["description"].as_str().unwrap_or_default();
                                    let schema = tool.get("inputSchema").cloned().unwrap_or(
                                        serde_json::json!({"type": "object", "properties": {}}),
                                    );

                                    let full_name = format!("mcp_{}_{}", name, tool_name);
                                    registry_for_mcp.register(Arc::new(
                                        agent::tools::SidecarBridgeTool::new_mcp(
                                            "http://localhost:8765".to_string(),
                                            full_name,
                                            tool_desc.to_string(),
                                            schema,
                                            name.clone(),
                                            tool_name.to_string(),
                                        ),
                                    ));
                                }
                                eprintln!("[mcp] 已恢复 MCP 服务器 {} 的工具注册", name);
                            }
                        }
                    }
                }
            });

            // 自动恢复飞书长连接与事件同步 relay（按员工配置对齐 + 周期健康检查）
            let pool_for_feishu = pool.clone();
            let relay_for_feishu = feishu_relay_state.clone();
            let app_for_feishu = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let mut backoff_secs = 2u64;
                for _ in 0..30 {
                    let has_connections =
                        match commands::feishu_gateway::reconcile_feishu_employee_connections_with_pool(
                            &pool_for_feishu,
                            None,
                        )
                        .await
                        {
                            Ok(summary) => !summary.items.is_empty(),
                            Err(_) => false,
                        };
                    if has_connections {
                        let _ = commands::feishu_gateway::start_feishu_event_relay_with_pool_and_app(
                            &pool_for_feishu,
                            relay_for_feishu.clone(),
                            Some(app_for_feishu.clone()),
                            None,
                            Some(1500),
                            Some(50),
                        )
                        .await;
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                    backoff_secs = (backoff_secs * 2).min(60);
                }

                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    let summary = match commands::feishu_gateway::reconcile_feishu_employee_connections_with_pool(
                        &pool_for_feishu,
                        None,
                    )
                    .await
                    {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    if summary.items.is_empty() {
                        continue;
                    }

                    let relay_status = commands::feishu_gateway::start_feishu_event_relay_with_pool_and_app(
                        &pool_for_feishu,
                        relay_for_feishu.clone(),
                        Some(app_for_feishu.clone()),
                        None,
                        Some(1500),
                        Some(50),
                    )
                    .await;
                    if relay_status.is_ok() {
                        continue;
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::skills::install_skill,
            commands::skills::import_local_skill,
            commands::skills::install_industry_bundle,
            commands::skills::check_industry_bundle_update,
            commands::skills::refresh_local_skill,
            commands::skills::create_local_skill,
            commands::skills::render_local_skill_preview,
            commands::skills::list_skills,
            commands::skills::delete_skill,
            commands::clawhub::search_clawhub_skills,
            commands::clawhub::recommend_clawhub_skills,
            commands::clawhub::list_clawhub_library,
            commands::clawhub::translate_clawhub_texts,
            commands::clawhub::install_clawhub_skill,
            commands::clawhub::check_clawhub_skill_update,
            commands::clawhub::update_clawhub_skill,
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
            commands::runtime_preferences::get_runtime_preferences,
            commands::runtime_preferences::set_runtime_preferences,
            commands::runtime_preferences::resolve_default_work_dir,
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
            commands::feishu_gateway::handle_feishu_event,
            commands::feishu_gateway::send_feishu_text_message,
            commands::feishu_gateway::list_feishu_chats,
            commands::feishu_gateway::push_role_summary_to_feishu,
            commands::feishu_gateway::set_feishu_gateway_settings,
            commands::feishu_gateway::get_feishu_gateway_settings,
            commands::feishu_gateway::start_feishu_long_connection,
            commands::feishu_gateway::stop_feishu_long_connection,
            commands::feishu_gateway::get_feishu_long_connection_status,
            commands::feishu_gateway::get_feishu_employee_connection_statuses,
            commands::feishu_gateway::sync_feishu_ws_events,
            commands::feishu_gateway::start_feishu_event_relay,
            commands::feishu_gateway::stop_feishu_event_relay,
            commands::feishu_gateway::get_feishu_event_relay_status,
            commands::openclaw_gateway::handle_openclaw_event,
            commands::openclaw_gateway::simulate_im_route,
            commands::im_gateway::handle_feishu_callback,
            commands::im_config::bind_thread_roles,
            commands::im_config::get_thread_role_config,
            commands::im_routing::list_im_routing_bindings,
            commands::im_routing::upsert_im_routing_binding,
            commands::im_routing::delete_im_routing_binding,
            commands::employee_agents::list_agent_employees,
            commands::employee_agents::upsert_agent_employee,
            commands::employee_agents::delete_agent_employee,
            commands::employee_agents::get_employee_memory_stats,
            commands::employee_agents::export_employee_memory,
            commands::employee_agents::clear_employee_memory,
            commands::agent_profile::generate_agent_profile_draft,
            commands::agent_profile::apply_agent_profile,
            commands::mcp::add_mcp_server,
            commands::mcp::list_mcp_servers,
            commands::mcp::remove_mcp_server,
            commands::dialog::select_directory,
            commands::packaging::read_skill_dir,
            commands::packaging::scan_workclaw_dirs,
            commands::packaging::update_skill_dir_tags,
            commands::packaging::pack_skill,
            commands::packaging::pack_industry_bundle,
            commands::packaging::read_industry_bundle_manifest,
            commands::packaging::unpack_industry_bundle,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
