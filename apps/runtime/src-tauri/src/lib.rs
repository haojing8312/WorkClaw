mod adapters;
pub mod agent;
mod builtin_skills;
pub mod commands;
mod db;
mod diagnostics;
pub mod im;
pub mod providers;
pub mod session_journal;
pub mod sidecar;
pub mod team_templates;

use agent::tools::new_responder;
use agent::tools::search_providers::cache::SearchCache;
use agent::{AgentExecutor, ToolRegistry};
use commands::chat::{
    AskUserState, CancelFlagState, SearchCacheState, ToolConfirmResponder, ToolConfirmState,
};
use commands::feishu_gateway::FeishuEventRelayState;
use commands::skills::DbState;
use diagnostics::{DiagnosticsState, ManagedDiagnosticsState};
use session_journal::{SessionJournalStateHandle, SessionJournalStore};
use sidecar::SidecarManager;
use std::sync::Arc;
use tauri::Manager;

struct DiagnosticsStateHandle(Arc<DiagnosticsState>);

impl Drop for DiagnosticsStateHandle {
    fn drop(&mut self) {
        let _ = diagnostics::mark_clean_exit(self.0.as_ref());
    }
}

struct ManagedRuntimeHandles {
    registry: Arc<ToolRegistry>,
    sidecar_manager: Arc<SidecarManager>,
    feishu_relay_state: FeishuEventRelayState,
}

fn initialize_runtime_state(app: &mut tauri::App, pool: sqlx::SqlitePool) -> ManagedRuntimeHandles {
    app.manage(DbState(pool.clone()));

    let registry = Arc::new(ToolRegistry::with_standard_tools());
    let agent_executor = Arc::new(AgentExecutor::new(Arc::clone(&registry)));
    app.manage(agent_executor);
    app.manage(Arc::clone(&registry));

    let ask_user_responder = new_responder();
    app.manage(AskUserState(ask_user_responder));
    let tool_confirm_responder: ToolConfirmResponder =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    app.manage(ToolConfirmState(tool_confirm_responder));

    let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    app.manage(CancelFlagState(cancel_flag));

    let search_cache = Arc::new(SearchCache::new(std::time::Duration::from_secs(900), 100));
    app.manage(SearchCacheState(search_cache));

    let journal_root = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("workclaw"))
        .join("sessions");
    let journal_store = Arc::new(SessionJournalStore::new(journal_root));
    app.manage(SessionJournalStateHandle(journal_store));

    let sidecar_manager = Arc::new(SidecarManager::new());
    app.manage(sidecar_manager.clone());

    let feishu_relay_state = FeishuEventRelayState::default();
    app.manage(feishu_relay_state.clone());

    ManagedRuntimeHandles {
        registry,
        sidecar_manager,
        feishu_relay_state,
    }
}

fn apply_startup_preferences(app: &mut tauri::App, pool: &sqlx::SqlitePool) {
    let startup_prefs = tauri::async_runtime::block_on(
        commands::runtime_preferences::get_runtime_preferences_with_pool(pool),
    )
    .ok();
    if let Some(prefs) = startup_prefs {
        let _ = commands::runtime_preferences::sync_launch_at_login(
            app.handle(),
            prefs.launch_at_login,
        );
        if prefs.launch_minimized {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.minimize();
            }
        }
    }
}

fn spawn_sidecar_bootstrap(sidecar_manager: Arc<SidecarManager>) {
    std::thread::spawn(move || {
        tauri::async_runtime::block_on(async move {
            for i in 0..20 {
                if sidecar_manager.health_check().await.is_ok() {
                    break;
                }
                if let Err(e) = sidecar_manager.start().await {
                    eprintln!("[sidecar] start attempt {} failed: {}", i + 1, e);
                } else {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    });
}

fn restore_saved_mcp_servers(pool: sqlx::SqlitePool, registry: Arc<ToolRegistry>) {
    tauri::async_runtime::spawn(async move {
        let servers = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT name, command, args, env FROM mcp_servers WHERE enabled = 1",
        )
        .fetch_all(&pool)
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
                            let tool_desc = tool["description"].as_str().unwrap_or_default();
                            let schema = tool
                                .get("inputSchema")
                                .cloned()
                                .unwrap_or(serde_json::json!({"type": "object", "properties": {}}));

                            let full_name = format!("mcp_{}_{}", name, tool_name);
                            registry.register(Arc::new(agent::tools::SidecarBridgeTool::new_mcp(
                                "http://localhost:8765".to_string(),
                                full_name,
                                tool_desc.to_string(),
                                schema,
                                name.clone(),
                                tool_name.to_string(),
                            )));
                        }
                        eprintln!("[mcp] 已恢复 MCP 服务器 {} 的工具注册", name);
                    }
                }
            }
        }
    });
}

fn spawn_feishu_relay_bootstrap(
    pool: sqlx::SqlitePool,
    relay_state: FeishuEventRelayState,
    app_handle: tauri::AppHandle,
) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let mut backoff_secs = 2u64;
        for _ in 0..30 {
            let has_connections =
                match commands::feishu_gateway::reconcile_feishu_employee_connections_with_pool(
                    &pool, None,
                )
                .await
                {
                    Ok(summary) => !summary.items.is_empty(),
                    Err(_) => false,
                };
            if has_connections {
                let _ = commands::feishu_gateway::start_feishu_event_relay_with_pool_and_app(
                    &pool,
                    relay_state.clone(),
                    Some(app_handle.clone()),
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
            let summary =
                match commands::feishu_gateway::reconcile_feishu_employee_connections_with_pool(
                    &pool, None,
                )
                .await
                {
                    Ok(v) => v,
                    Err(_) => continue,
                };
            if summary.items.is_empty() {
                continue;
            }

            let relay_status =
                commands::feishu_gateway::start_feishu_event_relay_with_pool_and_app(
                    &pool,
                    relay_state.clone(),
                    Some(app_handle.clone()),
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
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|app, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = tauri::async_runtime::block_on(
                    commands::runtime_preferences::get_runtime_preferences_with_pool(
                        &app.state::<DbState>().0,
                    ),
                )
                .map(|prefs| prefs.close_to_tray)
                .unwrap_or(false);

                if close_to_tray {
                    api.prevent_close();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
            }
        })
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let diagnostics_state = diagnostics::initialize_for_app(
                app.handle(),
                app.package_info().version.to_string(),
            )
            .expect("failed to init diagnostics");
            let diagnostics_state = Arc::new(diagnostics_state);
            app.manage(DiagnosticsStateHandle(Arc::clone(&diagnostics_state)));
            app.manage(ManagedDiagnosticsState(Arc::clone(&diagnostics_state)));

            let pool = match tauri::async_runtime::block_on(db::init_db(app.handle())) {
                Ok(pool) => pool,
                Err(error) => {
                    let _ = diagnostics::write_log_record(
                        &diagnostics_state.paths,
                        diagnostics::LogLevel::Error,
                        "runtime",
                        "db_init_failed",
                        &error.to_string(),
                        None,
                    );
                    panic!("failed to init db: {error}");
                }
            };
            let handles = initialize_runtime_state(app, pool.clone());
            apply_startup_preferences(app, &pool);
            spawn_sidecar_bootstrap(handles.sidecar_manager.clone());
            restore_saved_mcp_servers(pool.clone(), Arc::clone(&handles.registry));
            spawn_feishu_relay_bootstrap(
                pool,
                handles.feishu_relay_state.clone(),
                app.handle().clone(),
            );

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
            commands::clawhub::get_clawhub_skill_detail,
            commands::clawhub::translate_texts_with_preferences,
            commands::clawhub::translate_clawhub_texts,
            commands::clawhub::install_clawhub_skill,
            commands::clawhub::install_github_skill_repo,
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
            commands::models::set_default_model,
            commands::models::set_default_search,
            commands::models::list_builtin_provider_plugins,
            commands::models::get_routing_settings,
            commands::models::set_routing_settings,
            commands::runtime_preferences::get_runtime_preferences,
            commands::runtime_preferences::set_runtime_preferences,
            commands::runtime_preferences::resolve_default_work_dir,
            commands::desktop_lifecycle::get_desktop_lifecycle_paths,
            commands::desktop_lifecycle::get_desktop_diagnostics_status,
            commands::desktop_lifecycle::open_desktop_path,
            commands::desktop_lifecycle::open_desktop_diagnostics_dir,
            commands::desktop_lifecycle::clear_desktop_cache_and_logs,
            commands::desktop_lifecycle::export_desktop_environment_summary,
            commands::desktop_lifecycle::export_desktop_diagnostics_bundle,
            commands::desktop_lifecycle::record_frontend_diagnostic_event,
            commands::chat::create_session,
            commands::chat::send_message,
            commands::chat_session_commands::get_messages,
            commands::chat_session_commands::list_sessions,
            commands::chat_session_commands::get_sessions,
            commands::chat_session_commands::delete_session,
            commands::chat_session_commands::update_session_workspace,
            commands::chat_session_commands::search_sessions_global,
            commands::chat_session_commands::search_sessions,
            commands::chat_session_commands::export_session,
            commands::chat_session_commands::write_export_file,
            commands::session_runs::list_session_runs,
            commands::chat_control::answer_user_question,
            commands::chat_control::confirm_tool_execution,
            commands::chat_control::cancel_agent,
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
            commands::wecom_gateway::set_wecom_gateway_settings,
            commands::wecom_gateway::get_wecom_gateway_settings,
            commands::wecom_gateway::start_wecom_connector,
            commands::wecom_gateway::get_wecom_connector_status,
            commands::wecom_gateway::send_wecom_text_message,
            commands::channel_connectors::list_channel_connectors,
            commands::channel_connectors::get_channel_connector_diagnostics,
            commands::channel_connectors::ack_channel_events,
            commands::channel_connectors::replay_channel_events,
            commands::openclaw_gateway::handle_openclaw_event,
            commands::openclaw_gateway::simulate_im_route,
            commands::im_gateway::handle_feishu_callback,
            commands::im_config::bind_thread_roles,
            commands::im_config::get_thread_role_config,
            commands::im_routing::list_im_routing_bindings,
            commands::im_routing::upsert_im_routing_binding,
            commands::im_routing::delete_im_routing_binding,
            commands::employee_agents::list_agent_employees,
            commands::employee_agents::create_employee_group,
            commands::employee_agents::create_employee_team,
            commands::employee_agents::clone_employee_group_template,
            commands::employee_agents::list_employee_groups,
            commands::employee_agents::list_employee_group_runs,
            commands::employee_agents::list_employee_group_rules,
            commands::employee_agents::delete_employee_group,
            commands::employee_agents::start_employee_group_run,
            commands::employee_agents::continue_employee_group_run,
            commands::employee_agents::run_group_step,
            commands::employee_agents::get_employee_group_run_snapshot,
            commands::employee_agents::cancel_employee_group_run,
            commands::employee_agents::retry_employee_group_run_failed_steps,
            commands::employee_agents::review_group_run_step,
            commands::employee_agents::pause_employee_group_run,
            commands::employee_agents::resume_employee_group_run,
            commands::employee_agents::reassign_group_run_step,
            commands::employee_agents::upsert_agent_employee,
            commands::employee_agents::delete_agent_employee,
            commands::employee_agents::get_employee_memory_stats,
            commands::employee_agents::export_employee_memory,
            commands::employee_agents::clear_employee_memory,
            commands::agent_profile::generate_agent_profile_draft,
            commands::agent_profile::apply_agent_profile,
            commands::agent_profile::get_agent_profile_files,
            commands::mcp::add_mcp_server,
            commands::mcp::list_mcp_servers,
            commands::mcp::remove_mcp_server,
            commands::dialog::select_directory,
            commands::dialog::open_external_url,
            commands::workspace_files::list_workspace_files,
            commands::workspace_files::read_workspace_file_preview,
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
