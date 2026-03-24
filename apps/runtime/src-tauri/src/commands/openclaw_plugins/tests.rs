    use super::*;
    use super::installer_session::{
        build_openclaw_lark_installer_command, build_openclaw_shim_script,
        derive_installer_auto_input, ensure_openclaw_cli_shim, infer_installer_prompt_hint,
        prepend_env_path,
    };
    use super::plugin_host_service::{
        apply_command_search_path, build_effective_path_entries,
        build_plugin_host_fixture_root_from_app_data_dir,
        collect_windows_node_command_candidates, derive_channel_capabilities,
        derive_feishu_plugin_environment_status, parse_windows_registry_path_output,
        resolve_windows_node_command_path,
    };
    use super::runtime_service::{
        handle_openclaw_plugin_feishu_runtime_command_error_event,
        handle_openclaw_plugin_feishu_runtime_send_result_event,
        matches_feishu_runtime_command_line, merge_feishu_runtime_status_event,
        parse_feishu_runtime_dispatch_event_with_pool,
        register_pending_feishu_runtime_outbound_send_waiter,
    };
    use super::setup_service::{
        derive_feishu_credentials_from_openclaw_state_config,
        derive_feishu_credentials_from_shim_snapshot, derive_feishu_setup_summary_state,
        parse_feishu_app_access_token_response, parse_feishu_bot_info_response,
        OpenClawShimRecordedCommand, OpenClawShimStateSnapshot,
    };
    use crate::commands::feishu_gateway::{get_app_setting, set_app_setting};
    use crate::im::types::ImEventType;
    use std::path::PathBuf;
    use std::process::Command;
    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::time::Duration;

    async fn setup_memory_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE installed_openclaw_plugins (
                plugin_id TEXT PRIMARY KEY,
                npm_spec TEXT NOT NULL,
                version TEXT NOT NULL,
                install_path TEXT NOT NULL,
                source_type TEXT NOT NULL DEFAULT 'npm',
                manifest_json TEXT NOT NULL DEFAULT '{}',
                installed_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create installed_openclaw_plugins table");

        sqlx::query(
            "CREATE TABLE installed_skills (
                id TEXT PRIMARY KEY,
                manifest TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                username TEXT NOT NULL,
                pack_path TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'encrypted'
            )",
        )
        .execute(&pool)
        .await
        .expect("create installed_skills table");

        sqlx::query(
            "CREATE TABLE app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create app_settings table");

        sqlx::query(
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL DEFAULT '',
                role_id TEXT NOT NULL DEFAULT '',
                persona TEXT NOT NULL DEFAULT '',
                feishu_open_id TEXT NOT NULL DEFAULT '',
                feishu_app_id TEXT NOT NULL DEFAULT '',
                feishu_app_secret TEXT NOT NULL DEFAULT '',
                primary_skill_id TEXT NOT NULL DEFAULT '',
                default_work_dir TEXT NOT NULL DEFAULT '',
                openclaw_agent_id TEXT NOT NULL DEFAULT '',
                routing_priority INTEGER NOT NULL DEFAULT 100,
                enabled_scopes_json TEXT NOT NULL DEFAULT '[\"app\"]',
                enabled INTEGER NOT NULL DEFAULT 1,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees table");

        sqlx::query(
            "CREATE TABLE agent_employee_skills (
                employee_id TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employee_skills table");

        sqlx::query(
            "CREATE TABLE feishu_pairing_requests (
                id TEXT PRIMARY KEY,
                channel TEXT NOT NULL,
                account_id TEXT NOT NULL DEFAULT 'default',
                sender_id TEXT NOT NULL,
                chat_id TEXT NOT NULL DEFAULT '',
                code TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                resolved_at TEXT,
                resolved_by_user TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("create feishu_pairing_requests table");

        sqlx::query(
            "CREATE TABLE feishu_pairing_allow_from (
                channel TEXT NOT NULL DEFAULT 'feishu',
                account_id TEXT NOT NULL DEFAULT 'default',
                sender_id TEXT NOT NULL,
                source_request_id TEXT NOT NULL DEFAULT '',
                approved_at TEXT NOT NULL,
                approved_by_user TEXT NOT NULL DEFAULT '',
                PRIMARY KEY(channel, account_id, sender_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create feishu_pairing_allow_from");

        sqlx::query(
            "CREATE TABLE im_routing_bindings (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                account_id TEXT NOT NULL DEFAULT '',
                peer_kind TEXT NOT NULL DEFAULT '',
                peer_id TEXT NOT NULL DEFAULT '',
                guild_id TEXT NOT NULL DEFAULT '',
                team_id TEXT NOT NULL DEFAULT '',
                role_ids_json TEXT NOT NULL DEFAULT '[]',
                connector_meta_json TEXT NOT NULL DEFAULT '{}',
                priority INTEGER NOT NULL DEFAULT 100,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(&pool)
        .await
        .expect("create im_routing_bindings");

        pool
    }

    #[tokio::test]
    async fn feishu_advanced_settings_round_trip_through_app_settings() {
        let pool = setup_memory_pool().await;
        let saved = set_openclaw_plugin_feishu_advanced_settings_with_pool(
            &pool,
            &OpenClawPluginFeishuAdvancedSettings {
                groups_json: "{\"oc_demo\":{\"enabled\":true}}".to_string(),
                dms_json: "{\"ou_demo\":{\"enabled\":true}}".to_string(),
                footer_json: "{\"status\":true}".to_string(),
                account_overrides_json: "{\"default\":{\"renderMode\":\"card\"}}".to_string(),
                render_mode: "card".to_string(),
                streaming: "true".to_string(),
                text_chunk_limit: "2400".to_string(),
                chunk_mode: "newline".to_string(),
                reply_in_thread: "enabled".to_string(),
                group_session_scope: "group_sender".to_string(),
                topic_session_mode: "enabled".to_string(),
                markdown_mode: "native".to_string(),
                markdown_table_mode: "native".to_string(),
                heartbeat_visibility: "visible".to_string(),
                heartbeat_interval_ms: "30000".to_string(),
                media_max_mb: "20".to_string(),
                http_timeout_ms: "60000".to_string(),
                config_writes: "true".to_string(),
                webhook_host: "127.0.0.1".to_string(),
                webhook_port: "8787".to_string(),
                dynamic_agent_creation_enabled: "true".to_string(),
                dynamic_agent_creation_workspace_template: "workspace/{sender_id}".to_string(),
                dynamic_agent_creation_agent_dir_template: "agents/{sender_id}".to_string(),
                dynamic_agent_creation_max_agents: "48".to_string(),
            },
        )
        .await
        .expect("save advanced settings");

        assert_eq!(
            saved,
            OpenClawPluginFeishuAdvancedSettings {
                groups_json: "{\"oc_demo\":{\"enabled\":true}}".to_string(),
                dms_json: "{\"ou_demo\":{\"enabled\":true}}".to_string(),
                footer_json: "{\"status\":true}".to_string(),
                account_overrides_json: "{\"default\":{\"renderMode\":\"card\"}}".to_string(),
                render_mode: "card".to_string(),
                streaming: "true".to_string(),
                text_chunk_limit: "2400".to_string(),
                chunk_mode: "newline".to_string(),
                reply_in_thread: "enabled".to_string(),
                group_session_scope: "group_sender".to_string(),
                topic_session_mode: "enabled".to_string(),
                markdown_mode: "native".to_string(),
                markdown_table_mode: "native".to_string(),
                heartbeat_visibility: "visible".to_string(),
                heartbeat_interval_ms: "30000".to_string(),
                media_max_mb: "20".to_string(),
                http_timeout_ms: "60000".to_string(),
                config_writes: "true".to_string(),
                webhook_host: "127.0.0.1".to_string(),
                webhook_port: "8787".to_string(),
                dynamic_agent_creation_enabled: "true".to_string(),
                dynamic_agent_creation_workspace_template: "workspace/{sender_id}".to_string(),
                dynamic_agent_creation_agent_dir_template: "agents/{sender_id}".to_string(),
                dynamic_agent_creation_max_agents: "48".to_string(),
            }
        );

        let loaded = get_openclaw_plugin_feishu_advanced_settings_with_pool(&pool)
            .await
            .expect("load advanced settings");
        assert_eq!(loaded, saved);
    }

    #[tokio::test]
    async fn feishu_advanced_settings_returns_projection_defaults_when_unset() {
        let pool = setup_memory_pool().await;

        let loaded = get_openclaw_plugin_feishu_advanced_settings_with_pool(&pool)
            .await
            .expect("load defaults");

        assert_eq!(loaded.render_mode, "auto");
        assert_eq!(loaded.streaming, "false");
        assert_eq!(loaded.text_chunk_limit, "4000");
        assert_eq!(loaded.chunk_mode, "length");
        assert_eq!(loaded.reply_in_thread, "disabled");
        assert_eq!(loaded.group_session_scope, "group");
        assert_eq!(loaded.topic_session_mode, "disabled");
        assert_eq!(loaded.markdown_mode, "native");
        assert_eq!(loaded.markdown_table_mode, "native");
        assert_eq!(loaded.heartbeat_visibility, "visible");
        assert_eq!(loaded.heartbeat_interval_ms, "30000");
        assert_eq!(loaded.media_max_mb, "20");
        assert_eq!(loaded.http_timeout_ms, "60000");
        assert_eq!(loaded.config_writes, "false");
        assert_eq!(loaded.dynamic_agent_creation_enabled, "false");
    }

    #[tokio::test]
    async fn feishu_advanced_settings_treats_blank_rows_as_unset_defaults() {
        let pool = setup_memory_pool().await;
        for key in [
            "feishu_markdown_mode",
            "feishu_markdown_table_mode",
            "feishu_heartbeat_visibility",
            "feishu_heartbeat_interval_ms",
            "feishu_media_max_mb",
            "feishu_http_timeout_ms",
            "feishu_config_writes",
            "feishu_dynamic_agent_creation_enabled",
        ] {
            set_app_setting(&pool, key, "").await.expect("set blank app setting");
        }

        let loaded = get_openclaw_plugin_feishu_advanced_settings_with_pool(&pool)
            .await
            .expect("load defaults from blank rows");

        assert_eq!(loaded.markdown_mode, "native");
        assert_eq!(loaded.markdown_table_mode, "native");
        assert_eq!(loaded.heartbeat_visibility, "visible");
        assert_eq!(loaded.heartbeat_interval_ms, "30000");
        assert_eq!(loaded.media_max_mb, "20");
        assert_eq!(loaded.http_timeout_ms, "60000");
        assert_eq!(loaded.config_writes, "false");
        assert_eq!(loaded.dynamic_agent_creation_enabled, "false");
    }

    #[tokio::test]
    async fn build_feishu_openclaw_config_projects_official_defaults() {
        let pool = setup_memory_pool().await;
        set_app_setting(&pool, "feishu_app_id", "cli_root")
            .await
            .expect("set app id");
        set_app_setting(&pool, "feishu_app_secret", "secret_root")
            .await
            .expect("set app secret");
        set_app_setting(&pool, "feishu_history_limit", "36")
            .await
            .expect("set history limit");
        set_app_setting(&pool, "feishu_dm_history_limit", "10")
            .await
            .expect("set dm history limit");
        set_app_setting(&pool, "feishu_media_max_mb", "20")
            .await
            .expect("set media max mb");
        set_app_setting(&pool, "feishu_http_timeout_ms", "60000")
            .await
            .expect("set http timeout");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_enabled", "true")
            .await
            .expect("set block coalesce enabled");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_min_delay_ms", "100")
            .await
            .expect("set block coalesce min delay");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_max_delay_ms", "300")
            .await
            .expect("set block coalesce max delay");
        set_app_setting(&pool, "feishu_heartbeat_visibility", "visible")
            .await
            .expect("set heartbeat visibility");
        set_app_setting(&pool, "feishu_heartbeat_interval_ms", "30000")
            .await
            .expect("set heartbeat interval");
        set_app_setting(&pool, "feishu_dynamic_agent_creation_enabled", "true")
            .await
            .expect("set dynamic agent creation enabled");
        set_app_setting(
            &pool,
            "feishu_dynamic_agent_creation_workspace_template",
            "workspace/{sender_id}",
        )
        .await
        .expect("set dynamic workspace template");
        set_app_setting(
            &pool,
            "feishu_dynamic_agent_creation_agent_dir_template",
            "agents/{sender_id}",
        )
        .await
        .expect("set dynamic agent dir template");
        set_app_setting(&pool, "feishu_dynamic_agent_creation_max_agents", "48")
            .await
            .expect("set dynamic max agents");
        set_app_setting(
            &pool,
            "feishu_dms",
            "{\"user:carla\":{\"enabled\":true,\"systemPrompt\":\"优先处理私聊任务\"}}",
        )
        .await
        .expect("set dms");
        set_app_setting(
            &pool,
            "feishu_footer",
            "{\"status\":true,\"elapsed\":true}",
        )
        .await
        .expect("set footer");
        set_app_setting(
            &pool,
            "feishu_groups",
            "{\"oc_demo\":{\"enabled\":true,\"requireMention\":false,\"systemPrompt\":\"只处理 demo 群\",\"tools\":{\"allow\":[\"search_web\"]}}}",
        )
        .await
        .expect("set specific groups");

        let config = build_feishu_openclaw_config_with_pool(&pool)
            .await
            .expect("build feishu openclaw config");
        let feishu = &config["channels"]["feishu"];

        assert_eq!(feishu["enabled"], serde_json::json!(true));
        assert_eq!(feishu["defaultAccount"], serde_json::json!("default"));
        assert_eq!(feishu["appId"], serde_json::json!("cli_root"));
        assert_eq!(feishu["appSecret"], serde_json::json!("secret_root"));
        assert_eq!(feishu["domain"], serde_json::json!("feishu"));
        assert_eq!(feishu["connectionMode"], serde_json::json!("websocket"));
        assert_eq!(feishu["webhookPath"], serde_json::json!("/feishu/events"));
        assert_eq!(feishu["dmPolicy"], serde_json::json!("pairing"));
        assert_eq!(feishu["groupPolicy"], serde_json::json!("allowlist"));
        assert_eq!(feishu["requireMention"], serde_json::json!(true));
        assert_eq!(feishu["reactionNotifications"], serde_json::json!("own"));
        assert_eq!(feishu["typingIndicator"], serde_json::json!(true));
        assert_eq!(feishu["resolveSenderNames"], serde_json::json!(true));
        assert_eq!(feishu["streaming"], serde_json::json!(false));
        assert_eq!(feishu["replyInThread"], serde_json::json!("disabled"));
        assert_eq!(feishu["groupSessionScope"], serde_json::json!("group"));
        assert_eq!(feishu["topicSessionMode"], serde_json::json!("disabled"));
        assert_eq!(feishu["groupAllowFrom"], serde_json::json!([]));
        assert_eq!(feishu["groupSenderAllowFrom"], serde_json::json!([]));
        assert_eq!(
            feishu["groups"]["*"],
            serde_json::json!({
                "enabled": true,
                "requireMention": true,
                "groupSessionScope": "group",
                "topicSessionMode": "disabled",
                "replyInThread": "disabled"
            })
        );
        assert_eq!(
            feishu["groups"]["oc_demo"],
            serde_json::json!({
                "enabled": true,
                "requireMention": false,
                "systemPrompt": "只处理 demo 群",
                "tools": {
                    "allow": ["search_web"]
                }
            })
        );
        assert_eq!(feishu["configWrites"], serde_json::json!(false));
        assert_eq!(feishu["webhookHost"], serde_json::json!(""));
        assert_eq!(feishu["webhookPort"], serde_json::Value::Null);
        assert_eq!(feishu["markdown"], serde_json::json!({}));
        assert_eq!(feishu["renderMode"], serde_json::json!("auto"));
        assert_eq!(feishu["textChunkLimit"], serde_json::json!(4000));
        assert_eq!(feishu["chunkMode"], serde_json::json!("length"));
        assert_eq!(
            feishu["blockStreamingCoalesce"],
            serde_json::json!({
                "enabled": true,
                "minDelayMs": 100,
                "maxDelayMs": 300
            })
        );
        assert_eq!(feishu["historyLimit"], serde_json::json!(36));
        assert_eq!(feishu["dmHistoryLimit"], serde_json::json!(10));
        assert_eq!(feishu["mediaMaxMb"], serde_json::json!(20));
        assert_eq!(feishu["httpTimeoutMs"], serde_json::json!(60000));
        assert_eq!(
            feishu["heartbeat"],
            serde_json::json!({
                "visibility": "visible",
                "intervalMs": 30000
            })
        );
        assert_eq!(
            feishu["dynamicAgentCreation"],
            serde_json::json!({
                "enabled": true,
                "workspaceTemplate": "workspace/{sender_id}",
                "agentDirTemplate": "agents/{sender_id}",
                "maxAgents": 48
            })
        );
        assert_eq!(
            feishu["dms"],
            serde_json::json!({
                "user:carla": {
                    "enabled": true,
                    "systemPrompt": "优先处理私聊任务"
                }
            })
        );
        assert_eq!(
            feishu["footer"],
            serde_json::json!({
                "status": true,
                "elapsed": true
            })
        );
        assert_eq!(feishu["actions"], serde_json::json!({ "reactions": false }));
        assert_eq!(
            feishu["tools"],
            serde_json::json!({
                "doc": true,
                "chat": true,
                "wiki": true,
                "drive": true,
                "perm": false,
                "scopes": true
            })
        );
        assert_eq!(feishu["allowFrom"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn build_feishu_openclaw_config_projects_employee_accounts_with_inherited_defaults() {
        let pool = setup_memory_pool().await;
        set_app_setting(&pool, "feishu_app_id", "cli_root")
            .await
            .expect("set app id");
        set_app_setting(&pool, "feishu_app_secret", "secret_root")
            .await
            .expect("set app secret");
        set_app_setting(&pool, "feishu_ingress_token", "verify_root")
            .await
            .expect("set verification token");
        set_app_setting(&pool, "feishu_encrypt_key", "encrypt_root")
            .await
            .expect("set encrypt key");
        set_app_setting(&pool, "feishu_streaming", "true")
            .await
            .expect("set streaming");
        set_app_setting(&pool, "feishu_reply_in_thread", "enabled")
            .await
            .expect("set reply in thread");
        set_app_setting(&pool, "feishu_group_session_scope", "group_sender")
            .await
            .expect("set group session scope");
        set_app_setting(&pool, "feishu_topic_session_mode", "enabled")
            .await
            .expect("set topic session mode");
        set_app_setting(&pool, "feishu_group_allow_from", "[\"ou_group_owner\"]")
            .await
            .expect("set group allow from");
        set_app_setting(
            &pool,
            "feishu_group_sender_allow_from",
            "ou_sender_a,ou_sender_b",
        )
        .await
        .expect("set group sender allow from");
        set_app_setting(&pool, "feishu_webhook_host", "127.0.0.1")
            .await
            .expect("set webhook host");
        set_app_setting(&pool, "feishu_webhook_port", "8787")
            .await
            .expect("set webhook port");
        set_app_setting(&pool, "feishu_config_writes", "true")
            .await
            .expect("set config writes");
        set_app_setting(&pool, "feishu_actions_reactions", "true")
            .await
            .expect("set actions reactions");
        set_app_setting(&pool, "feishu_render_mode", "card")
            .await
            .expect("set render mode");
        set_app_setting(&pool, "feishu_text_chunk_limit", "3200")
            .await
            .expect("set text chunk limit");
        set_app_setting(&pool, "feishu_chunk_mode", "newline")
            .await
            .expect("set chunk mode");
        set_app_setting(&pool, "feishu_markdown_mode", "native")
            .await
            .expect("set markdown mode");
        set_app_setting(&pool, "feishu_markdown_table_mode", "native")
            .await
            .expect("set markdown table mode");
        set_app_setting(&pool, "feishu_history_limit", "40")
            .await
            .expect("set history limit");
        set_app_setting(&pool, "feishu_dm_history_limit", "12")
            .await
            .expect("set dm history limit");
        set_app_setting(&pool, "feishu_media_max_mb", "25")
            .await
            .expect("set media max mb");
        set_app_setting(&pool, "feishu_http_timeout_ms", "45000")
            .await
            .expect("set http timeout ms");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_enabled", "true")
            .await
            .expect("set block coalesce enabled");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_min_delay_ms", "80")
            .await
            .expect("set block coalesce min delay");
        set_app_setting(&pool, "feishu_block_streaming_coalesce_max_delay_ms", "240")
            .await
            .expect("set block coalesce max delay");
        set_app_setting(&pool, "feishu_heartbeat_visibility", "hidden")
            .await
            .expect("set heartbeat visibility");
        set_app_setting(&pool, "feishu_heartbeat_interval_ms", "15000")
            .await
            .expect("set heartbeat interval ms");
        set_app_setting(&pool, "feishu_dynamic_agent_creation_enabled", "true")
            .await
            .expect("set dynamic agent creation enabled");
        set_app_setting(
            &pool,
            "feishu_dynamic_agent_creation_workspace_template",
            "employees/{sender_id}",
        )
        .await
        .expect("set dynamic workspace template");
        set_app_setting(
            &pool,
            "feishu_dynamic_agent_creation_agent_dir_template",
            "agents/{sender_id}",
        )
        .await
        .expect("set dynamic agent dir template");
        set_app_setting(&pool, "feishu_dynamic_agent_creation_max_agents", "24")
            .await
            .expect("set dynamic max agents");
        set_app_setting(
            &pool,
            "feishu_dms",
            "{\"ou_dm_vip\":{\"enabled\":true,\"systemPrompt\":\"仅处理 VIP 私聊\"}}",
        )
        .await
        .expect("set dms");
        set_app_setting(
            &pool,
            "feishu_footer",
            "{\"status\":true,\"elapsed\":false}",
        )
        .await
        .expect("set footer");
        set_app_setting(
            &pool,
            "feishu_groups",
            "{\"oc_ops\":{\"enabled\":true,\"requireMention\":true,\"skills\":[\"ops\"],\"replyInThread\":\"enabled\"}}",
        )
        .await
        .expect("set specific groups");
        set_app_setting(
            &pool,
            "feishu_account_overrides",
            "{\"taizi\":{\"enabled\":false,\"verificationToken\":\"verify_override\",\"renderMode\":\"raw\",\"footer\":{\"status\":false,\"elapsed\":true},\"groups\":{\"oc_ops\":{\"requireMention\":false}}}}",
        )
        .await
        .expect("set account overrides");
        set_app_setting(
            &pool,
            "feishu_group_default_allow_from",
            "[\"ou_group_only\"]",
        )
        .await
        .expect("set group default allowFrom");
        set_app_setting(
            &pool,
            "feishu_group_default_skills",
            "[\"briefing\", \"planner\"]",
        )
        .await
        .expect("set group default skills");
        set_app_setting(
            &pool,
            "feishu_group_default_system_prompt",
            "只处理群内任务分发",
        )
        .await
        .expect("set group default system prompt");
        set_app_setting(
            &pool,
            "feishu_group_default_tools",
            "{\"allow\":[\"read_file\",\"search_web\"]}",
        )
        .await
        .expect("set group default tools");

        sqlx::query(
            "INSERT INTO agent_employees (
                id, employee_id, name, role_id, feishu_app_id, feishu_app_secret, enabled, is_default, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, 1, 1, ?)",
        )
        .bind("emp_1")
        .bind("taizi")
        .bind("太子")
        .bind("taizi")
        .bind("cli_taizi")
        .bind("secret_taizi")
        .bind("2026-03-20T00:00:00Z")
        .execute(&pool)
        .await
        .expect("insert employee account");

        sqlx::query(
            "INSERT INTO feishu_pairing_allow_from (
                channel, account_id, sender_id, source_request_id, approved_at, approved_by_user
             ) VALUES ('feishu', ?, ?, ?, ?, ?)",
        )
        .bind("taizi")
        .bind("ou_allowed")
        .bind("req_1")
        .bind("2026-03-20T00:00:00Z")
        .bind("tester")
        .execute(&pool)
        .await
        .expect("insert approved sender");

        let config = build_feishu_openclaw_config_with_pool(&pool)
            .await
            .expect("build feishu openclaw config");
        let default_account = &config["channels"]["feishu"];
        let account = &config["channels"]["feishu"]["accounts"]["taizi"];

        assert_eq!(account["enabled"], serde_json::json!(false));
        assert_eq!(account["name"], serde_json::json!("太子"));
        assert_eq!(account["appId"], serde_json::json!("cli_taizi"));
        assert_eq!(account["appSecret"], serde_json::json!("secret_taizi"));
        assert_eq!(account["domain"], serde_json::json!("feishu"));
        assert_eq!(account["connectionMode"], serde_json::json!("websocket"));
        assert_eq!(account["webhookPath"], serde_json::json!("/feishu/events"));
        assert_eq!(
            account["verificationToken"],
            serde_json::json!("verify_override")
        );
        assert_eq!(account["encryptKey"], serde_json::json!("encrypt_root"));
        assert_eq!(account["encryptKey"], default_account["encryptKey"]);
        assert_eq!(account["dmPolicy"], default_account["dmPolicy"]);
        assert_eq!(account["groupPolicy"], default_account["groupPolicy"]);
        assert_eq!(account["dmPolicy"], serde_json::json!("pairing"));
        assert_eq!(account["groupPolicy"], serde_json::json!("allowlist"));
        assert_eq!(account["requireMention"], serde_json::json!(true));
        assert_eq!(account["reactionNotifications"], serde_json::json!("own"));
        assert_eq!(account["typingIndicator"], serde_json::json!(true));
        assert_eq!(account["resolveSenderNames"], serde_json::json!(true));
        assert_eq!(account["streaming"], serde_json::json!(true));
        assert_eq!(account["replyInThread"], serde_json::json!("enabled"));
        assert_eq!(
            account["groupSessionScope"],
            serde_json::json!("group_sender")
        );
        assert_eq!(account["topicSessionMode"], serde_json::json!("enabled"));
        assert_eq!(
            account["groupAllowFrom"],
            serde_json::json!(["ou_group_owner"])
        );
        assert_eq!(
            account["groupSenderAllowFrom"],
            serde_json::json!(["ou_sender_a", "ou_sender_b"])
        );
        assert_eq!(
            account["groups"]["*"],
            serde_json::json!({
                "enabled": true,
                "allowFrom": ["ou_group_only"],
                "requireMention": true,
                "skills": ["briefing", "planner"],
                "systemPrompt": "只处理群内任务分发",
                "tools": {
                    "allow": ["read_file", "search_web"]
                },
                "groupSessionScope": "group_sender",
                "topicSessionMode": "enabled",
                "replyInThread": "enabled"
            })
        );
        assert_eq!(
            account["groups"]["oc_ops"],
            serde_json::json!({
                "enabled": true,
                "requireMention": false,
                "skills": ["ops"],
                "replyInThread": "enabled"
            })
        );
        assert_eq!(account["configWrites"], serde_json::json!(true));
        assert_eq!(account["webhookHost"], serde_json::json!("127.0.0.1"));
        assert_eq!(account["webhookPort"], serde_json::json!(8787));
        assert_eq!(
            account["markdown"],
            serde_json::json!({
                "mode": "native",
                "tableMode": "native"
            })
        );
        assert_eq!(account["renderMode"], serde_json::json!("raw"));
        assert_eq!(account["textChunkLimit"], serde_json::json!(3200));
        assert_eq!(account["chunkMode"], serde_json::json!("newline"));
        assert_eq!(
            account["blockStreamingCoalesce"],
            serde_json::json!({
                "enabled": true,
                "minDelayMs": 80,
                "maxDelayMs": 240
            })
        );
        assert_eq!(account["historyLimit"], serde_json::json!(40));
        assert_eq!(account["dmHistoryLimit"], serde_json::json!(12));
        assert_eq!(account["mediaMaxMb"], serde_json::json!(25));
        assert_eq!(account["httpTimeoutMs"], serde_json::json!(45000));
        assert_eq!(
            account["heartbeat"],
            serde_json::json!({
                "visibility": "hidden",
                "intervalMs": 15000
            })
        );
        assert_eq!(
            account["dynamicAgentCreation"],
            serde_json::json!({
                "enabled": true,
                "workspaceTemplate": "employees/{sender_id}",
                "agentDirTemplate": "agents/{sender_id}",
                "maxAgents": 24
            })
        );
        assert_eq!(account["dms"], default_account["dms"]);
        assert_ne!(account["footer"], default_account["footer"]);
        assert_eq!(
            account["dms"],
            serde_json::json!({
                "ou_dm_vip": {
                    "enabled": true,
                    "systemPrompt": "仅处理 VIP 私聊"
                }
            })
        );
        assert_eq!(
            account["footer"],
            serde_json::json!({
                "status": false,
                "elapsed": true
            })
        );
        assert_eq!(
            account["groups"]["oc_ops"],
            serde_json::json!({
                "enabled": true,
                "requireMention": false,
                "skills": ["ops"],
                "replyInThread": "enabled"
            })
        );
        assert_eq!(account["actions"], serde_json::json!({ "reactions": true }));
        assert_eq!(
            account["tools"],
            serde_json::json!({
                "doc": true,
                "chat": true,
                "wiki": true,
                "drive": true,
                "perm": false,
                "scopes": true
            })
        );
        assert_eq!(account["allowFrom"], serde_json::json!(["ou_allowed"]));
    }

    #[test]
    fn installer_auto_input_selects_create_mode_by_default() {
        let mut auto = OpenClawLarkInstallerAutoInputState::default();
        let payload = derive_installer_auto_input(
            &OpenClawLarkInstallerMode::Create,
            None,
            None,
            "What would you like to do (请选择操作):",
            &mut auto,
        );
        assert_eq!(payload.as_deref(), Some("\r"));
        assert!(auto.selection_sent);
    }

    #[test]
    fn installer_auto_input_selects_link_mode_and_sends_credentials() {
        let mut auto = OpenClawLarkInstallerAutoInputState::default();
        let select_payload = derive_installer_auto_input(
            &OpenClawLarkInstallerMode::Link,
            Some("cli_app"),
            Some("secret"),
            "What would you like to do (请选择操作):",
            &mut auto,
        );
        assert_eq!(select_payload.as_deref(), Some("\u{1b}[B\r"));

        let app_id_payload = derive_installer_auto_input(
            &OpenClawLarkInstallerMode::Link,
            Some("cli_app"),
            Some("secret"),
            "Enter your App ID (请输入 App ID):",
            &mut auto,
        );
        assert_eq!(app_id_payload.as_deref(), Some("cli_app\r"));

        let app_secret_payload = derive_installer_auto_input(
            &OpenClawLarkInstallerMode::Link,
            Some("cli_app"),
            Some("secret"),
            "Enter your App Secret [press Enter to confirm] (请输入 App Secret [按回车确认]):",
            &mut auto,
        );
        assert_eq!(app_secret_payload.as_deref(), Some("secret\r"));
    }

    #[test]
    fn installer_prompt_hint_explains_poll_waiting_states() {
        assert_eq!(
            infer_installer_prompt_hint(
                "Fetching configuration results (正在获取你的机器人配置结果)..."
            )
            .as_deref(),
            Some("正在等待飞书官方接口返回机器人 App ID / App Secret，请稍候。")
        );
        assert_eq!(
            infer_installer_prompt_hint(
                "[DEBUG] Poll result: {\"error\":\"authorization_pending\"}"
            )
            .as_deref(),
            Some("飞书官方接口仍在等待这次扫码配置完成回传结果（authorization_pending）。")
        );
    }

    #[test]
    fn derives_environment_status_when_node_and_npm_are_available() {
        let status = derive_feishu_plugin_environment_status(
            Ok(Some("v22.0.0".to_string())),
            Ok(Some("10.8.0".to_string())),
            true,
        );

        assert!(status.node_available);
        assert!(status.npm_available);
        assert_eq!(status.node_version.as_deref(), Some("v22.0.0"));
        assert_eq!(status.npm_version.as_deref(), Some("10.8.0"));
        assert!(status.can_install_plugin);
        assert!(status.can_start_runtime);
        assert_eq!(status.error, None);
    }

    #[test]
    fn derives_environment_status_when_node_is_missing() {
        let status = derive_feishu_plugin_environment_status(
            Ok(None),
            Ok(Some("10.8.0".to_string())),
            true,
        );

        assert!(!status.node_available);
        assert!(status.npm_available);
        assert!(!status.can_install_plugin);
        assert!(!status.can_start_runtime);
        assert_eq!(status.error.as_deref(), Some("未检测到 Node.js"));
    }

    #[test]
    fn derives_environment_status_when_npm_is_missing() {
        let status = derive_feishu_plugin_environment_status(
            Ok(Some("v22.0.0".to_string())),
            Ok(None),
            true,
        );

        assert!(status.node_available);
        assert!(!status.npm_available);
        assert!(status.can_start_runtime);
        assert!(!status.can_install_plugin);
        assert_eq!(status.error.as_deref(), Some("未检测到 npm"));
    }

    #[test]
    fn derives_environment_status_when_runtime_script_is_missing() {
        let status = derive_feishu_plugin_environment_status(
            Ok(Some("v22.0.0".to_string())),
            Ok(Some("10.8.0".to_string())),
            false,
        );

        assert!(status.node_available);
        assert!(status.npm_available);
        assert!(status.can_install_plugin);
        assert!(!status.can_start_runtime);
        assert_eq!(status.error.as_deref(), Some("飞书插件运行脚本缺失"));
    }

    #[test]
    fn derives_setup_summary_state_for_missing_environment() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus::default(),
            false,
            false,
            false,
            None,
            "unknown",
            0,
            None,
            0,
        );
        assert_eq!(summary, "env_missing");
    }

    #[test]
    fn derives_setup_summary_state_for_missing_plugin_install_before_credentials() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            false,
            false,
            false,
            None,
            "unknown",
            0,
            None,
            0,
        );
        assert_eq!(summary, "plugin_not_installed");
    }

    #[test]
    fn derives_setup_summary_state_for_missing_credentials_after_plugin_install() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            false,
            true,
            false,
            None,
            "unknown",
            0,
            None,
            0,
        );
        assert_eq!(summary, "ready_to_bind");
    }

    #[test]
    fn derives_setup_summary_state_for_pending_auth() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            true,
            true,
            true,
            None,
            "pending",
            0,
            None,
            0,
        );
        assert_eq!(summary, "awaiting_auth");
    }

    #[test]
    fn derives_setup_summary_state_for_pending_pairing_approval() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            true,
            true,
            true,
            None,
            "pending",
            1,
            None,
            0,
        );
        assert_eq!(summary, "awaiting_pairing_approval");
    }

    #[test]
    fn derives_setup_summary_state_for_ready_for_routing() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            true,
            true,
            true,
            None,
            "approved",
            0,
            None,
            0,
        );
        assert_eq!(summary, "ready_for_routing");
    }

    #[test]
    fn derives_setup_summary_state_for_fully_ready_flow() {
        let summary = derive_feishu_setup_summary_state(
            &FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            true,
            true,
            true,
            None,
            "approved",
            0,
            Some("财务刚"),
            1,
        );
        assert_eq!(summary, "ready");
    }

    #[test]
    fn auto_restore_feishu_runtime_when_previous_connection_was_fully_approved() {
        let progress = FeishuSetupProgress {
            environment: FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            credentials_configured: true,
            plugin_installed: true,
            plugin_version: Some("1.0.0".to_string()),
            runtime_running: false,
            runtime_last_error: None,
            auth_status: "approved".to_string(),
            pending_pairings: 0,
            default_routing_employee_name: Some("太子".to_string()),
            scoped_routing_count: 0,
            summary_state: "plugin_starting".to_string(),
        };

        assert!(should_auto_restore_feishu_runtime(&progress));
    }

    #[test]
    fn does_not_auto_restore_feishu_runtime_before_authorization_is_complete() {
        let progress = FeishuSetupProgress {
            environment: FeishuPluginEnvironmentStatus {
                node_available: true,
                npm_available: true,
                node_version: Some("v22".to_string()),
                npm_version: Some("10".to_string()),
                can_install_plugin: true,
                can_start_runtime: true,
                error: None,
            },
            credentials_configured: true,
            plugin_installed: true,
            plugin_version: Some("1.0.0".to_string()),
            runtime_running: false,
            runtime_last_error: None,
            auth_status: "pending".to_string(),
            pending_pairings: 0,
            default_routing_employee_name: None,
            scoped_routing_count: 0,
            summary_state: "awaiting_auth".to_string(),
        };

        assert!(!should_auto_restore_feishu_runtime(&progress));
    }

    #[test]
    fn openclaw_shim_script_supports_minimal_installer_commands() {
        let script = build_openclaw_shim_script(Path::new("C:\\temp\\state.json"));
        assert!(script.contains("args[0] === \"config\" && args[1] === \"get\""));
        assert!(script.contains("args[0] === \"config\" && args[1] === \"set\""));
        assert!(
            script.contains(
                "args[0] === \"gateway\" && (args[1] === \"restart\" || args[1] === \"start\" || args[1] === \"stop\")"
            )
        );
        assert!(
            script.contains(
                "(args[0] === \"plugins\" || args[0] === \"plugin\") && (args[1] === \"install\" || args[1] === \"uninstall\")"
            )
        );
        assert!(script.contains("plugin ${args[1]} satisfied via WorkClaw shim"));
        assert!(script.contains("args[0] === \"pairing\" && args[1] === \"approve\""));
        assert!(script.contains(OPENCLAW_SHIM_VERSION));
    }

    #[test]
    fn ensure_openclaw_cli_shim_creates_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let shim_dir = ensure_openclaw_cli_shim(temp.path()).expect("create shim");
        assert!(shim_dir.join("openclaw-shim.mjs").exists());
        assert!(shim_dir.join("state.json").exists());
        #[cfg(windows)]
        assert!(shim_dir.join("openclaw.cmd").exists());
        #[cfg(not(windows))]
        assert!(shim_dir.join("openclaw").exists());
    }

    #[test]
    fn installer_command_prefers_installed_bin_script_when_present() {
        let temp = tempfile::tempdir().expect("tempdir");
        let plugin_dir = temp.path().join("openclaw-lark");
        let installer_script = plugin_dir.join("bin").join("openclaw-lark.js");
        std::fs::create_dir_all(installer_script.parent().expect("installer script parent"))
            .expect("create installer script parent");
        std::fs::write(&installer_script, "#!/usr/bin/env node\n").expect("write installer script");

        let command = build_openclaw_lark_installer_command(&plugin_dir)
            .expect("build official installer command");
        let args: Vec<String> = command
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect();

        assert_eq!(args.first().map(String::as_str), Some(&*installer_script.to_string_lossy()));
        assert_eq!(args.get(1).map(String::as_str), Some("install"));
        assert_eq!(args.get(2).map(String::as_str), Some("--debug"));
        assert!(
            !args.iter().any(|value| value.contains("@larksuite/openclaw-lark")),
            "expected direct script execution instead of npm exec fallback"
        );
    }

    #[test]
    fn installer_command_prefers_local_tools_bin_when_present() {
        let temp = tempfile::tempdir().expect("tempdir");
        let plugin_dir = temp
            .path()
            .join("workspace")
            .join("node_modules")
            .join("@larksuite")
            .join("openclaw-lark");
        let tools_bin = temp
            .path()
            .join("workspace")
            .join("installer-tools")
            .join("node_modules")
            .join(".bin")
            .join(if cfg!(windows) {
                "feishu-plugin-onboard.cmd"
            } else {
                "feishu-plugin-onboard"
            });
        std::fs::create_dir_all(plugin_dir.join("bin")).expect("create plugin dir");
        std::fs::create_dir_all(tools_bin.parent().expect("tools bin parent"))
            .expect("create tools bin parent");
        std::fs::write(&tools_bin, "@echo off\r\n").expect("write tools bin");

        let command = build_openclaw_lark_installer_command(&plugin_dir)
            .expect("build official installer command");
        let args: Vec<String> = command
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect();

        assert_eq!(
            command.get_program().to_string_lossy().to_string(),
            tools_bin.to_string_lossy().to_string()
        );
        assert_eq!(args, vec!["install", "--debug", "--skip-version-check"]);
    }

    #[test]
    fn derive_feishu_credentials_from_shim_snapshot_reads_config_projection() {
        let snapshot = OpenClawShimStateSnapshot {
            config: serde_json::json!({
                "channels": {
                    "feishu": {
                        "appId": "cli_created",
                        "appSecret": "secret_created"
                    }
                }
            }),
            commands: vec![],
        };

        let credentials =
            derive_feishu_credentials_from_shim_snapshot(&snapshot).expect("credentials from config");

        assert_eq!(credentials.0, "cli_created");
        assert_eq!(credentials.1, "secret_created");
    }

    #[test]
    fn derive_feishu_credentials_from_shim_snapshot_falls_back_to_recorded_commands() {
        let snapshot = OpenClawShimStateSnapshot {
            config: serde_json::json!({}),
            commands: vec![
                OpenClawShimRecordedCommand {
                    args: vec![
                        "config".to_string(),
                        "set".to_string(),
                        "channels.feishu.appId".to_string(),
                        "cli_from_command".to_string(),
                    ],
                },
                OpenClawShimRecordedCommand {
                    args: vec![
                        "config".to_string(),
                        "set".to_string(),
                        "channels.feishu.appSecret".to_string(),
                        "secret_from_command".to_string(),
                    ],
                },
            ],
        };

        let credentials =
            derive_feishu_credentials_from_shim_snapshot(&snapshot).expect("credentials from commands");

        assert_eq!(credentials.0, "cli_from_command");
        assert_eq!(credentials.1, "secret_from_command");
    }

    #[tokio::test]
    async fn sync_feishu_gateway_credentials_from_shim_updates_app_settings() {
        let pool = setup_memory_pool().await;
        let temp = tempfile::tempdir().expect("tempdir");
        let shim_root = temp.path().join("openclaw-cli-shim");
        std::fs::create_dir_all(&shim_root).expect("create shim root");
        std::fs::write(
            build_openclaw_shim_state_file_path(&shim_root),
            serde_json::json!({
                "config": {
                    "channels": {
                        "feishu": {
                            "appId": "cli_synced",
                            "appSecret": "secret_synced"
                        }
                    }
                },
                "commands": []
            })
            .to_string(),
        )
        .expect("write shim state");

        let updated = sync_feishu_gateway_credentials_from_shim_with_pool(&pool, &shim_root)
            .await
            .expect("sync shim credentials");

        assert!(updated);
        assert_eq!(
            get_app_setting(&pool, "feishu_app_id")
                .await
                .expect("load app id")
                .as_deref(),
            Some("cli_synced")
        );
        assert_eq!(
            get_app_setting(&pool, "feishu_app_secret")
                .await
                .expect("load app secret")
                .as_deref(),
            Some("secret_synced")
        );
    }

    #[test]
    fn derive_feishu_credentials_from_openclaw_state_config_reads_plaintext_credentials() {
        let state_root = Path::new("C:\\workclaw\\openclaw-state");
        let config = serde_json::json!({
            "channels": {
                "feishu": {
                    "appId": "cli_created_from_state",
                    "appSecret": "secret_created_from_state"
                }
            }
        });

        let credentials = derive_feishu_credentials_from_openclaw_state_config(&config, state_root)
            .expect("credentials from controlled state config");

        assert_eq!(credentials.0, "cli_created_from_state");
        assert_eq!(credentials.1, "secret_created_from_state");
    }

    #[tokio::test]
    async fn sync_feishu_gateway_credentials_from_controlled_state_reads_env_secret() {
        let pool = setup_memory_pool().await;
        let temp = tempfile::tempdir().expect("tempdir");
        let state_root = temp.path().join("openclaw-state");
        std::fs::create_dir_all(&state_root).expect("create state root");
        std::fs::write(
            state_root.join(".env"),
            "LARK_APP_SECRET=secret_from_env\n",
        )
        .expect("write env file");
        std::fs::write(
            state_root.join("openclaw.json"),
            serde_json::json!({
                "channels": {
                    "feishu": {
                        "appId": "cli_from_state",
                        "appSecret": {
                            "source": "env",
                            "provider": "default",
                            "id": "LARK_APP_SECRET"
                        }
                    }
                }
            })
            .to_string(),
        )
        .expect("write controlled state config");

        let updated = sync_feishu_gateway_credentials_from_openclaw_state_with_pool(&pool, &state_root)
            .await
            .expect("sync credentials from controlled state");

        assert!(updated);
        assert_eq!(
            get_app_setting(&pool, "feishu_app_id")
                .await
                .expect("load app id")
                .as_deref(),
            Some("cli_from_state")
        );
        assert_eq!(
            get_app_setting(&pool, "feishu_app_secret")
                .await
                .expect("load app secret")
                .as_deref(),
            Some("secret_from_env")
        );
    }

    #[test]
    fn resolve_plugin_host_dir_finds_packaged_up_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let exe_dir = temp.path().join("runtime-bin");
        let up_plugin_host = exe_dir.join("_up_").join("plugin-host");
        std::fs::create_dir_all(&up_plugin_host).expect("create packaged plugin host");
        std::fs::write(up_plugin_host.join("marker.txt"), "ok").expect("write marker");

        let candidates = [
            exe_dir.join("resources").join("plugin-host"),
            exe_dir.join("_up_").join("plugin-host"),
            exe_dir.join("plugin-host"),
        ];
        let resolved = candidates
            .into_iter()
            .find(|candidate| candidate.exists())
            .expect("resolved packaged plugin host");
        assert_eq!(resolved, up_plugin_host);
    }

    #[test]
    fn build_plugin_host_fixture_root_uses_app_data_dir() {
        let app_data_dir = Path::new(r"C:\Users\Alice\AppData\Roaming\dev.workclaw.runtime");
        let fixture_root = build_plugin_host_fixture_root_from_app_data_dir(app_data_dir);
        assert_eq!(
            fixture_root,
            PathBuf::from(r"C:\Users\Alice\AppData\Roaming\dev.workclaw.runtime\plugin-host-fixtures")
        );
    }

    #[test]
    fn prepend_env_path_places_shim_first() {
        let mut command = Command::new("node");
        let shim_dir = Path::new("C:\\shim");
        prepend_env_path(&mut command, shim_dir);
        let env_path = command
            .get_envs()
            .find_map(|(key, value)| (key == "PATH").then(|| value))
            .flatten()
            .expect("PATH env");
        let first = std::env::split_paths(env_path)
            .next()
            .expect("first PATH segment");
        assert_eq!(first, shim_dir);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn build_effective_path_entries_keeps_prepend_and_adds_registry_paths() {
        let prepend = vec![PathBuf::from(r"C:\shim")];
        let current_path = std::ffi::OsString::from(r"C:\gui-node;C:\common");
        let extra_entries = vec![PathBuf::from(r"C:\user-node"), PathBuf::from(r"C:\common")];

        let paths = build_effective_path_entries(Some(&current_path), &prepend, &extra_entries);

        assert_eq!(paths.first(), Some(&PathBuf::from(r"C:\shim")));
        assert!(paths.contains(&PathBuf::from(r"C:\gui-node")));
        assert!(paths.contains(&PathBuf::from(r"C:\user-node")));
        assert_eq!(
            paths
                .iter()
                .filter(|entry| entry == &&PathBuf::from(r"C:\common"))
                .count(),
            1
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parse_windows_registry_path_output_expands_env_segments() {
        std::env::set_var("LOCALAPPDATA", r"C:\Users\Alice\AppData\Local");
        let parsed = parse_windows_registry_path_output(
            "HKEY_CURRENT_USER\\Environment\n    Path    REG_EXPAND_SZ    %LOCALAPPDATA%\\Programs\\nodejs;C:\\Tools\\Node\n",
        );

        assert_eq!(
            parsed,
            vec![
                PathBuf::from(r"C:\Users\Alice\AppData\Local\Programs\nodejs"),
                PathBuf::from(r"C:\Tools\Node"),
            ]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_node_candidates_include_nvm_and_common_install_locations() {
        let temp = tempfile::tempdir().expect("tempdir");
        let nvm_link = temp.path().join("nvm-link");
        let nvm_home = temp.path().join("nvm-home");
        std::fs::create_dir_all(&nvm_link).expect("create nvm_link");
        std::fs::create_dir_all(&nvm_home).expect("create nvm_home");
        std::env::set_var("NVM_SYMLINK", &nvm_link);
        std::env::set_var("NVM_HOME", &nvm_home);

        let candidates = collect_windows_node_command_candidates();
        assert!(candidates.iter().any(|path| path.ends_with(Path::new("node.exe"))));
        assert!(candidates.iter().any(|path| path == &nvm_link.join("node.exe")));
        assert!(candidates.iter().any(|path| path == &nvm_home.join("node.exe")));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_node_candidates_are_deduped_case_insensitively() {
        std::env::set_var("PATH", r"C:\Node;C:\node");
        let candidates = collect_windows_node_command_candidates();
        let lowered: std::collections::HashSet<String> = candidates
            .iter()
            .map(|candidate| candidate.to_string_lossy().to_lowercase())
            .collect();
        assert_eq!(lowered.len(), candidates.len());
    }

    #[tokio::test]
    async fn outbound_send_writes_command_and_receives_structured_send_result() {
        use std::collections::HashMap;
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        use std::sync::{Arc, Mutex};

        let temp = tempfile::tempdir().expect("tempdir");
        let script_path = temp.path().join("echo-send-result.mjs");
        std::fs::write(
            &script_path,
            r#"
import readline from 'node:readline';
const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on('line', (line) => {
  const payload = JSON.parse(line);
  process.stdout.write(JSON.stringify({
    event: 'send_result',
    requestId: payload.requestId,
    request: payload,
    result: {
      delivered: true,
      channel: 'feishu',
      accountId: payload.accountId,
      target: payload.target,
      threadId: payload.threadId,
      text: payload.text,
      mode: payload.mode,
      messageId: 'om_outbound_1',
      chatId: payload.target,
      sequence: 1,
    },
  }) + '\n');
});
"#,
        )
        .expect("write echo runtime script");

        #[cfg(target_os = "windows")]
        let mut child = {
            let mut command =
                Command::new(resolve_windows_node_command_path().expect("resolve node path"));
            command
                .arg(&script_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            apply_command_search_path(&mut command, &[]);
            command.spawn().expect("spawn echo runtime")
        };
        #[cfg(not(target_os = "windows"))]
        let mut child = {
            let mut command = Command::new("node");
            command
                .arg(&script_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            apply_command_search_path(&mut command, &[]);
            command.spawn().expect("spawn echo runtime")
        };
        let stdout = child.stdout.take().expect("runtime stdout");
        let runtime_stdin = Arc::new(Mutex::new(child.stdin.take().expect("runtime stdin")));
        let state = OpenClawPluginFeishuRuntimeState(Arc::new(Mutex::new(
            OpenClawPluginFeishuRuntimeStore {
                process: Some(Arc::new(Mutex::new(Some(child)))),
                stdin: Some(runtime_stdin.clone()),
                status: OpenClawPluginFeishuRuntimeStatus {
                    running: true,
                    ..Default::default()
                },
                pending_outbound_send_results: HashMap::new(),
            },
        )));

        let state_clone = state.clone();
        let stdout_thread = std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                    continue;
                };
                let _ = handle_openclaw_plugin_feishu_runtime_send_result_event(
                    &state_clone,
                    &value,
                );
            }
        });

        let result = send_openclaw_plugin_feishu_runtime_outbound_message_in_state(
            &state,
            OpenClawPluginFeishuOutboundSendRequest {
                request_id: "request-1".to_string(),
                account_id: "default".to_string(),
            target: "oc_chat_123".to_string(),
            thread_id: Some("oc_chat_123".to_string()),
            text: "你好".to_string(),
            mode: "text".to_string(),
        },
        )
        .expect("send outbound message");

        assert_eq!(result.request_id, "request-1");
        assert_eq!(result.request.account_id, "default");
        assert_eq!(result.request.target, "oc_chat_123");
        assert_eq!(result.result.delivered, true);
        assert_eq!(result.result.channel, "feishu");
        assert_eq!(result.result.message_id, "om_outbound_1");
        assert_eq!(result.result.chat_id, "oc_chat_123");

        {
            let mut guard = state.0.lock().expect("runtime state lock");
            guard.stdin = None;
        }
        drop(runtime_stdin);
        {
            let guard = state.0.lock().expect("runtime state lock");
            if let Some(slot) = guard.process.as_ref() {
                if let Ok(mut child_guard) = slot.lock() {
                    if let Some(mut child) = child_guard.take() {
                        let _ = child.wait();
                    }
                }
            }
        }

        stdout_thread.join().expect("stdout reader");
    }

    #[test]
    fn outbound_command_error_fails_pending_request_immediately() {
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};

        let request_id = "request-command-error";
        let state = OpenClawPluginFeishuRuntimeState(Arc::new(Mutex::new(
            OpenClawPluginFeishuRuntimeStore {
                process: None,
                stdin: None,
                status: OpenClawPluginFeishuRuntimeStatus {
                    running: true,
                    ..Default::default()
                },
                pending_outbound_send_results: HashMap::new(),
            },
        )));

        let receiver = register_pending_feishu_runtime_outbound_send_waiter(&state, request_id)
            .expect("register pending outbound waiter");

        let handled = handle_openclaw_plugin_feishu_runtime_command_error_event(
            &state,
            &serde_json::json!({
                "event": "command_error",
                "requestId": request_id,
                "command": "send_message",
                "error": "outbound target is required",
            }),
        );

        assert!(handled, "expected command_error event to resolve pending waiter");
        let result = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("receive command_error result");
        match result {
            Ok(_) => panic!("expected outbound command to fail"),
            Err(error) => assert!(
                error.contains("outbound target is required"),
                "unexpected command_error: {error}"
            ),
        }
    }

    #[test]
    fn merges_runtime_status_patch_events() {
        let mut status = OpenClawPluginFeishuRuntimeStatus::default();
        merge_feishu_runtime_status_event(
            &mut status,
            &serde_json::json!({
                "event": "status",
                "patch": {
                    "accountId": "workspace",
                    "port": 3100,
                    "lastError": ""
                }
            }),
        );

        assert_eq!(status.account_id, "workspace");
        assert_eq!(status.port, Some(3100));
        assert_eq!(status.last_error, None);
    }

    #[test]
    fn merges_runtime_fatal_events_into_last_error() {
        let mut status = OpenClawPluginFeishuRuntimeStatus::default();
        merge_feishu_runtime_status_event(
            &mut status,
            &serde_json::json!({
                "event": "fatal",
                "error": "runtime crashed"
            }),
        );

        assert_eq!(status.last_error.as_deref(), Some("runtime crashed"));
    }

    #[test]
    fn merges_runtime_log_events_into_recent_logs_and_error_state() {
        let mut status = OpenClawPluginFeishuRuntimeStatus::default();
        merge_feishu_runtime_status_event(
            &mut status,
            &serde_json::json!({
                "event": "log",
                "level": "error",
                "scope": "channel/monitor",
                "message": "failed to dispatch inbound message"
            }),
        );

        assert_eq!(
            status.last_error.as_deref(),
            Some("[error] channel/monitor: failed to dispatch inbound message")
        );
        assert_eq!(
            status.recent_logs.last().map(String::as_str),
            Some("[error] channel/monitor: failed to dispatch inbound message")
        );
        assert!(status.last_event_at.is_some());
    }

    #[test]
    fn matches_feishu_runtime_command_line_by_plugin_root_and_account() {
        let command_line = "\"node\" D:\\code\\WorkClaw\\apps\\runtime\\plugin-host\\scripts\\run-feishu-host.mjs --plugin-root C:\\Users\\36443\\AppData\\Roaming\\dev.workclaw.runtime\\openclaw-plugins\\openclaw-lark\\workspace\\node_modules\\@larksuite\\openclaw-lark --fixture-name openclaw-lark-runtime --account-id default --config-json {}";
        assert!(matches_feishu_runtime_command_line(
            command_line,
            "C:\\Users\\36443\\AppData\\Roaming\\dev.workclaw.runtime\\openclaw-plugins\\openclaw-lark\\workspace\\node_modules\\@larksuite\\openclaw-lark",
            "default"
        ));
        assert!(!matches_feishu_runtime_command_line(
            command_line,
            "C:\\Users\\36443\\AppData\\Roaming\\dev.workclaw.runtime\\openclaw-plugins\\other",
            "default"
        ));
        assert!(!matches_feishu_runtime_command_line(
            command_line,
            "C:\\Users\\36443\\AppData\\Roaming\\dev.workclaw.runtime\\openclaw-plugins\\openclaw-lark\\workspace\\node_modules\\@larksuite\\openclaw-lark",
            "workspace"
        ));
    }

    #[tokio::test]
    async fn parses_runtime_dispatch_events_into_im_events() {
        let pool = setup_memory_pool().await;
        let event = parse_feishu_runtime_dispatch_event_with_pool(
            &pool,
            &serde_json::json!({
                "threadId": "ou_sender",
                "chatId": "oc_chat_123",
                "accountId": "default",
                "senderId": "ou_sender",
                "messageId": "om_123",
                "text": "你好",
                "chatType": "direct"
            }),
        )
        .await
        .expect("parse dispatch event");

        assert_eq!(event.channel, "feishu");
        assert_eq!(event.event_type, ImEventType::MessageCreated);
        assert_eq!(event.thread_id, "oc_chat_123");
        assert_eq!(event.text.as_deref(), Some("你好"));
        assert_eq!(event.sender_id.as_deref(), Some("ou_sender"));
        assert_eq!(event.chat_type.as_deref(), Some("direct"));
    }

    #[tokio::test]
    async fn resolves_runtime_dispatch_thread_id_from_pairing_chat_id() {
        let pool = setup_memory_pool().await;
        let _ = sqlx::query(
            "INSERT INTO feishu_pairing_requests (
                id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
             ) VALUES (?, 'feishu', ?, ?, ?, ?, 'approved', ?, ?, ?, ?)",
        )
        .bind("req_1")
        .bind("default")
        .bind("ou_sender")
        .bind("oc_chat_123")
        .bind("PAIR1234")
        .bind("2026-03-19T00:00:00Z")
        .bind("2026-03-19T00:00:00Z")
        .bind("2026-03-19T00:00:00Z")
        .bind("tester")
        .execute(&pool)
        .await
        .expect("insert pairing request");

        let event = parse_feishu_runtime_dispatch_event_with_pool(
            &pool,
            &serde_json::json!({
                "threadId": "ou_sender",
                "accountId": "default",
                "senderId": "ou_sender",
                "messageId": "om_124",
                "text": "你好",
                "chatType": "direct"
            }),
        )
        .await
        .expect("parse dispatch event");

        assert_eq!(event.thread_id, "oc_chat_123");
    }

    #[tokio::test]
    async fn upsert_openclaw_plugin_install_records_plugin_metadata() {
        let pool = setup_memory_pool().await;

        let record = upsert_openclaw_plugin_install_with_pool(
            &pool,
            OpenClawPluginInstallInput {
                plugin_id: "openclaw-lark".to_string(),
                npm_spec: "@larksuite/openclaw-lark".to_string(),
                version: "2026.3.17".to_string(),
                install_path: "D:/plugins/openclaw-lark".to_string(),
                source_type: "npm".to_string(),
                manifest_json: "{\"id\":\"openclaw-lark\"}".to_string(),
            },
        )
        .await
        .expect("upsert plugin install");

        assert_eq!(record.plugin_id, "openclaw-lark");
        assert_eq!(record.npm_spec, "@larksuite/openclaw-lark");
        assert_eq!(record.version, "2026.3.17");
        assert_eq!(record.install_path, "D:/plugins/openclaw-lark");
        assert_eq!(record.source_type, "npm");
        assert_eq!(record.manifest_json, "{\"id\":\"openclaw-lark\"}");
        assert!(!record.installed_at.is_empty());
        assert!(!record.updated_at.is_empty());
    }

    #[tokio::test]
    async fn list_openclaw_plugin_installs_is_separate_from_local_skills() {
        let pool = setup_memory_pool().await;

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES ('local-brainstorming', '{}', '2026-03-19T00:00:00Z', '', 'D:/skills/brainstorming', 'local')",
        )
        .execute(&pool)
        .await
        .expect("seed installed skill");

        upsert_openclaw_plugin_install_with_pool(
            &pool,
            OpenClawPluginInstallInput {
                plugin_id: "openclaw-lark".to_string(),
                npm_spec: "@larksuite/openclaw-lark".to_string(),
                version: "2026.3.17".to_string(),
                install_path: "D:/plugins/openclaw-lark".to_string(),
                source_type: "npm".to_string(),
                manifest_json: "{\"id\":\"openclaw-lark\"}".to_string(),
            },
        )
        .await
        .expect("upsert plugin install");

        let records = list_openclaw_plugin_installs_with_pool(&pool)
            .await
            .expect("list plugin installs");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].plugin_id, "openclaw-lark");
    }

    #[tokio::test]
    async fn upsert_openclaw_plugin_install_updates_existing_record() {
        let pool = setup_memory_pool().await;

        upsert_openclaw_plugin_install_with_pool(
            &pool,
            OpenClawPluginInstallInput {
                plugin_id: "openclaw-lark".to_string(),
                npm_spec: "@larksuite/openclaw-lark".to_string(),
                version: "2026.3.15".to_string(),
                install_path: "D:/plugins/openclaw-lark-old".to_string(),
                source_type: "npm".to_string(),
                manifest_json: "{\"id\":\"openclaw-lark\",\"version\":\"2026.3.15\"}".to_string(),
            },
        )
        .await
        .expect("seed plugin install");

        let updated = upsert_openclaw_plugin_install_with_pool(
            &pool,
            OpenClawPluginInstallInput {
                plugin_id: "openclaw-lark".to_string(),
                npm_spec: "@larksuite/openclaw-lark".to_string(),
                version: "2026.3.17".to_string(),
                install_path: "D:/plugins/openclaw-lark".to_string(),
                source_type: "npm".to_string(),
                manifest_json: "{\"id\":\"openclaw-lark\",\"version\":\"2026.3.17\"}".to_string(),
            },
        )
        .await
        .expect("update plugin install");

        let records = list_openclaw_plugin_installs_with_pool(&pool)
            .await
            .expect("list plugin installs");

        assert_eq!(records.len(), 1);
        assert_eq!(updated.version, "2026.3.17");
        assert_eq!(records[0].install_path, "D:/plugins/openclaw-lark");
    }

    #[test]
    fn derive_channel_capabilities_flattens_runtime_flags() {
        let channel = OpenClawPluginChannelInspection {
            id: Some("feishu".to_string()),
            meta: None,
            capabilities: Some(serde_json::json!({
                "chatTypes": ["direct", "group"],
                "media": true,
                "reactions": true,
                "threads": true,
                "nativeCommands": true,
                "blockStreaming": true
            })),
            reload_config_prefixes: vec!["channels.feishu".to_string()],
            has_pairing: true,
            has_setup: true,
            has_onboarding: true,
            has_directory: true,
            has_outbound: true,
            has_threading: true,
            has_actions: true,
            has_status: true,
            target_hint: Some("<chatId|user:openId>".to_string()),
        };

        let capabilities = derive_channel_capabilities(&channel);

        assert!(capabilities.contains(&"chat_type:direct".to_string()));
        assert!(capabilities.contains(&"chat_type:group".to_string()));
        assert!(capabilities.contains(&"media".to_string()));
        assert!(capabilities.contains(&"reactions".to_string()));
        assert!(capabilities.contains(&"threads".to_string()));
        assert!(capabilities.contains(&"native_commands".to_string()));
        assert!(capabilities.contains(&"block_streaming".to_string()));
        assert!(capabilities.contains(&"pairing".to_string()));
        assert!(capabilities.contains(&"setup".to_string()));
        assert!(capabilities.contains(&"onboarding".to_string()));
        assert!(capabilities.contains(&"directory".to_string()));
        assert!(capabilities.contains(&"outbound".to_string()));
        assert!(capabilities.contains(&"threading".to_string()));
        assert!(capabilities.contains(&"actions".to_string()));
        assert!(capabilities.contains(&"status".to_string()));
    }

    #[test]
    fn parse_feishu_app_access_token_response_returns_token_on_success() {
        let token = parse_feishu_app_access_token_response(serde_json::json!({
            "code": 0,
            "msg": "success",
            "app_access_token": "token-123"
        }))
        .expect("token should parse");

        assert_eq!(token, "token-123");
    }

    #[test]
    fn parse_feishu_app_access_token_response_returns_api_error() {
        let error = parse_feishu_app_access_token_response(serde_json::json!({
            "code": 99991663,
            "msg": "invalid app credentials"
        }))
        .expect_err("invalid credentials should fail");

        assert_eq!(error, "API error: invalid app credentials");
    }

    #[test]
    fn parse_feishu_bot_info_response_extracts_identity() {
        let result = parse_feishu_bot_info_response(
            "cli_app",
            serde_json::json!({
                "code": 0,
                "msg": "success",
                "bot": {
                    "bot_name": "WorkClaw Bot",
                    "open_id": "ou_bot_open_id"
                }
            }),
        );

        assert!(result.ok);
        assert_eq!(result.app_id, "cli_app");
        assert_eq!(result.bot_name.as_deref(), Some("WorkClaw Bot"));
        assert_eq!(result.bot_open_id.as_deref(), Some("ou_bot_open_id"));
        assert_eq!(result.error, None);
    }
