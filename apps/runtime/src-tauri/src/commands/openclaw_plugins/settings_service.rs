use crate::commands::feishu_gateway::{
    get_app_setting, list_enabled_employee_feishu_connections_with_pool,
    list_feishu_pairing_allow_from_with_pool, set_app_setting,
};
use sqlx::SqlitePool;

use super::{
    merge_pairing_allow_from, OpenClawPluginFeishuAdvancedSettings,
};

pub(crate) fn app_setting_string_or_default(value: Option<String>, default: &str) -> String {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn parse_app_setting_bool(value: Option<String>, default: bool) -> bool {
    match value
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_ascii_lowercase())
        .as_deref()
    {
        Some("1") | Some("true") | Some("yes") | Some("on") => true,
        Some("0") | Some("false") | Some("no") | Some("off") => false,
        _ => default,
    }
}

fn parse_app_setting_i64(value: Option<String>) -> Option<i64> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .and_then(|entry| entry.parse::<i64>().ok())
}

fn parse_app_setting_string_list(value: Option<String>) -> serde_json::Value {
    let Some(raw) = value
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    else {
        return serde_json::json!([]);
    };

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(entries) = parsed.as_array() {
            let normalized = entries
                .iter()
                .filter_map(|entry| match entry {
                    serde_json::Value::String(value) => Some(value.trim().to_string()),
                    serde_json::Value::Number(value) => Some(value.to_string()),
                    _ => None,
                })
                .filter(|entry| !entry.is_empty())
                .map(serde_json::Value::String)
                .collect::<Vec<_>>();
            return serde_json::Value::Array(normalized);
        }
    }

    serde_json::Value::Array(
        raw.split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| serde_json::Value::String(entry.to_string()))
            .collect::<Vec<_>>(),
    )
}

fn parse_app_setting_json_object(value: Option<String>) -> serde_json::Value {
    let Some(raw) = value.as_deref().map(str::trim).filter(|entry| !entry.is_empty()) else {
        return serde_json::json!({});
    };

    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn build_feishu_markdown_projection(mode: &str, table_mode: &str) -> serde_json::Value {
    let mode = mode.trim();
    let table_mode = table_mode.trim();
    let mut markdown = serde_json::Map::new();
    if !mode.is_empty() {
        markdown.insert(
            "mode".to_string(),
            serde_json::Value::String(mode.to_string()),
        );
    }
    if !table_mode.is_empty() {
        markdown.insert(
            "tableMode".to_string(),
            serde_json::Value::String(table_mode.to_string()),
        );
    }
    serde_json::Value::Object(markdown)
}

fn build_feishu_heartbeat_projection(
    visibility: &str,
    interval_ms: Option<i64>,
) -> serde_json::Value {
    let mut heartbeat = serde_json::Map::new();
    if !visibility.trim().is_empty() {
        heartbeat.insert(
            "visibility".to_string(),
            serde_json::Value::String(visibility.trim().to_string()),
        );
    }
    if let Some(interval_ms) = interval_ms {
        heartbeat.insert("intervalMs".to_string(), serde_json::json!(interval_ms));
    }
    serde_json::Value::Object(heartbeat)
}

fn build_feishu_block_streaming_coalesce_projection(
    enabled: bool,
    min_delay_ms: Option<i64>,
    max_delay_ms: Option<i64>,
) -> serde_json::Value {
    let mut config = serde_json::Map::new();
    if enabled {
        config.insert("enabled".to_string(), serde_json::json!(true));
    }
    if let Some(min_delay_ms) = min_delay_ms {
        config.insert("minDelayMs".to_string(), serde_json::json!(min_delay_ms));
    }
    if let Some(max_delay_ms) = max_delay_ms {
        config.insert("maxDelayMs".to_string(), serde_json::json!(max_delay_ms));
    }
    serde_json::Value::Object(config)
}

fn build_feishu_dynamic_agent_creation_projection(
    enabled: bool,
    workspace_template: &str,
    agent_dir_template: &str,
    max_agents: Option<i64>,
) -> serde_json::Value {
    let mut config = serde_json::Map::new();
    if enabled {
        config.insert("enabled".to_string(), serde_json::json!(true));
    }
    if !workspace_template.trim().is_empty() {
        config.insert(
            "workspaceTemplate".to_string(),
            serde_json::Value::String(workspace_template.trim().to_string()),
        );
    }
    if !agent_dir_template.trim().is_empty() {
        config.insert(
            "agentDirTemplate".to_string(),
            serde_json::Value::String(agent_dir_template.trim().to_string()),
        );
    }
    if let Some(max_agents) = max_agents {
        config.insert("maxAgents".to_string(), serde_json::json!(max_agents));
    }
    serde_json::Value::Object(config)
}

fn build_feishu_default_groups_projection(
    require_mention: bool,
    group_session_scope: &str,
    topic_session_mode: &str,
    reply_in_thread: &str,
    allow_from: serde_json::Value,
    skills: serde_json::Value,
    system_prompt: &str,
    tools: serde_json::Value,
    overrides: serde_json::Value,
) -> serde_json::Value {
    let mut group = serde_json::Map::new();
    group.insert("enabled".to_string(), serde_json::json!(true));
    group.insert(
        "requireMention".to_string(),
        serde_json::json!(require_mention),
    );
    group.insert(
        "groupSessionScope".to_string(),
        serde_json::json!(group_session_scope),
    );
    group.insert(
        "topicSessionMode".to_string(),
        serde_json::json!(topic_session_mode),
    );
    group.insert(
        "replyInThread".to_string(),
        serde_json::json!(reply_in_thread),
    );
    if allow_from.as_array().is_some_and(|items| !items.is_empty()) {
        group.insert("allowFrom".to_string(), allow_from);
    }
    if skills.as_array().is_some_and(|items| !items.is_empty()) {
        group.insert("skills".to_string(), skills);
    }
    if !system_prompt.trim().is_empty() {
        group.insert(
            "systemPrompt".to_string(),
            serde_json::Value::String(system_prompt.trim().to_string()),
        );
    }
    if tools.as_object().is_some_and(|items| !items.is_empty()) {
        group.insert("tools".to_string(), tools);
    }

    let mut groups = serde_json::Map::new();
    groups.insert("*".to_string(), serde_json::Value::Object(group));

    if let Some(entries) = overrides.as_object() {
        for (group_id, value) in entries {
            if group_id.trim().is_empty() {
                continue;
            }
            if value.is_object() {
                groups.insert(group_id.to_string(), value.clone());
            }
        }
    }

    serde_json::Value::Object(groups)
}

fn merge_json_value(target: &mut serde_json::Value, override_value: serde_json::Value) {
    match (target, override_value) {
        (serde_json::Value::Object(target_map), serde_json::Value::Object(override_map)) => {
            for (key, value) in override_map {
                match target_map.get_mut(&key) {
                    Some(existing) => merge_json_value(existing, value),
                    None => {
                        target_map.insert(key, value);
                    }
                }
            }
        }
        (target_slot, override_value) => {
            *target_slot = override_value;
        }
    }
}

pub(crate) async fn build_feishu_openclaw_config_with_pool(
    pool: &SqlitePool,
) -> Result<serde_json::Value, String> {
    let app_id = get_app_setting(pool, "feishu_app_id")
        .await?
        .unwrap_or_default();
    let app_secret = get_app_setting(pool, "feishu_app_secret")
        .await?
        .unwrap_or_default();
    let ingress_token = get_app_setting(pool, "feishu_ingress_token")
        .await?
        .unwrap_or_default();
    let encrypt_key = get_app_setting(pool, "feishu_encrypt_key")
        .await?
        .unwrap_or_default();
    let employee_connections = list_enabled_employee_feishu_connections_with_pool(pool).await?;
    let default_pairing_allow_from = list_feishu_pairing_allow_from_with_pool(pool, "default")
        .await
        .unwrap_or_default();
    let enabled = !app_id.trim().is_empty() || !employee_connections.is_empty();
    let default_account = if enabled {
        Some("default".to_string())
    } else {
        None
    };
    let default_domain = "feishu";
    let default_connection_mode = "websocket";
    let default_webhook_path = "/feishu/events";
    let default_dm_policy = "pairing";
    let default_group_policy = "allowlist";
    let default_reaction_notifications = "own";
    let default_require_mention = true;
    let default_typing_indicator = true;
    let default_resolve_sender_names = true;
    let default_render_mode = get_app_setting(pool, "feishu_render_mode")
        .await?
        .unwrap_or_else(|| "auto".to_string());
    let default_text_chunk_limit =
        parse_app_setting_i64(get_app_setting(pool, "feishu_text_chunk_limit").await?)
            .unwrap_or(4000);
    let default_chunk_mode = get_app_setting(pool, "feishu_chunk_mode")
        .await?
        .unwrap_or_else(|| "length".to_string());
    let default_markdown_mode = get_app_setting(pool, "feishu_markdown_mode")
        .await?
        .unwrap_or_default();
    let default_markdown_table_mode = get_app_setting(pool, "feishu_markdown_table_mode")
        .await?
        .unwrap_or_default();
    let default_dms = parse_app_setting_json_object(get_app_setting(pool, "feishu_dms").await?);
    let default_footer =
        parse_app_setting_json_object(get_app_setting(pool, "feishu_footer").await?);
    let default_history_limit =
        parse_app_setting_i64(get_app_setting(pool, "feishu_history_limit").await?);
    let default_dm_history_limit =
        parse_app_setting_i64(get_app_setting(pool, "feishu_dm_history_limit").await?);
    let default_group_allow_from =
        parse_app_setting_string_list(get_app_setting(pool, "feishu_group_allow_from").await?);
    let default_group_sender_allow_from = parse_app_setting_string_list(
        get_app_setting(pool, "feishu_group_sender_allow_from").await?,
    );
    let default_group_default_allow_from = parse_app_setting_string_list(
        get_app_setting(pool, "feishu_group_default_allow_from").await?,
    );
    let default_group_default_skills =
        parse_app_setting_string_list(get_app_setting(pool, "feishu_group_default_skills").await?);
    let default_group_default_system_prompt =
        get_app_setting(pool, "feishu_group_default_system_prompt")
            .await?
            .unwrap_or_default();
    let default_group_default_tools =
        parse_app_setting_json_object(get_app_setting(pool, "feishu_group_default_tools").await?);
    let default_group_overrides =
        parse_app_setting_json_object(get_app_setting(pool, "feishu_groups").await?);
    let account_overrides =
        parse_app_setting_json_object(get_app_setting(pool, "feishu_account_overrides").await?);
    let default_streaming =
        parse_app_setting_bool(get_app_setting(pool, "feishu_streaming").await?, false);
    let default_reply_in_thread = get_app_setting(pool, "feishu_reply_in_thread")
        .await?
        .unwrap_or_else(|| "disabled".to_string());
    let default_group_session_scope = get_app_setting(pool, "feishu_group_session_scope")
        .await?
        .unwrap_or_else(|| "group".to_string());
    let default_topic_session_mode = get_app_setting(pool, "feishu_topic_session_mode")
        .await?
        .unwrap_or_else(|| "disabled".to_string());
    let default_webhook_host = get_app_setting(pool, "feishu_webhook_host")
        .await?
        .unwrap_or_default();
    let default_webhook_port =
        parse_app_setting_i64(get_app_setting(pool, "feishu_webhook_port").await?);
    let default_media_max_mb =
        parse_app_setting_i64(get_app_setting(pool, "feishu_media_max_mb").await?);
    let default_http_timeout_ms =
        parse_app_setting_i64(get_app_setting(pool, "feishu_http_timeout_ms").await?);
    let default_config_writes =
        parse_app_setting_bool(get_app_setting(pool, "feishu_config_writes").await?, false);
    let default_actions_reactions = parse_app_setting_bool(
        get_app_setting(pool, "feishu_actions_reactions").await?,
        false,
    );
    let default_block_streaming_coalesce_enabled = parse_app_setting_bool(
        get_app_setting(pool, "feishu_block_streaming_coalesce_enabled").await?,
        false,
    );
    let default_block_streaming_coalesce_min_delay_ms = parse_app_setting_i64(
        get_app_setting(pool, "feishu_block_streaming_coalesce_min_delay_ms").await?,
    );
    let default_block_streaming_coalesce_max_delay_ms = parse_app_setting_i64(
        get_app_setting(pool, "feishu_block_streaming_coalesce_max_delay_ms").await?,
    );
    let default_heartbeat_visibility = get_app_setting(pool, "feishu_heartbeat_visibility")
        .await?
        .unwrap_or_default();
    let default_heartbeat_interval_ms =
        parse_app_setting_i64(get_app_setting(pool, "feishu_heartbeat_interval_ms").await?);
    let default_dynamic_agent_creation_enabled = parse_app_setting_bool(
        get_app_setting(pool, "feishu_dynamic_agent_creation_enabled").await?,
        false,
    );
    let default_dynamic_agent_creation_workspace_template =
        get_app_setting(pool, "feishu_dynamic_agent_creation_workspace_template")
            .await?
            .unwrap_or_default();
    let default_dynamic_agent_creation_agent_dir_template =
        get_app_setting(pool, "feishu_dynamic_agent_creation_agent_dir_template")
            .await?
            .unwrap_or_default();
    let default_dynamic_agent_creation_max_agents = parse_app_setting_i64(
        get_app_setting(pool, "feishu_dynamic_agent_creation_max_agents").await?,
    );
    let default_tools = serde_json::json!({
        "doc": true,
        "chat": true,
        "wiki": true,
        "drive": true,
        "perm": false,
        "scopes": true
    });
    let default_markdown =
        build_feishu_markdown_projection(&default_markdown_mode, &default_markdown_table_mode);
    let default_heartbeat = build_feishu_heartbeat_projection(
        &default_heartbeat_visibility,
        default_heartbeat_interval_ms,
    );
    let default_block_streaming_coalesce = build_feishu_block_streaming_coalesce_projection(
        default_block_streaming_coalesce_enabled,
        default_block_streaming_coalesce_min_delay_ms,
        default_block_streaming_coalesce_max_delay_ms,
    );
    let default_dynamic_agent_creation = build_feishu_dynamic_agent_creation_projection(
        default_dynamic_agent_creation_enabled,
        &default_dynamic_agent_creation_workspace_template,
        &default_dynamic_agent_creation_agent_dir_template,
        default_dynamic_agent_creation_max_agents,
    );
    let default_groups = build_feishu_default_groups_projection(
        default_require_mention,
        &default_group_session_scope,
        &default_topic_session_mode,
        &default_reply_in_thread,
        default_group_default_allow_from,
        default_group_default_skills,
        &default_group_default_system_prompt,
        default_group_default_tools,
        default_group_overrides,
    );

    let mut accounts = serde_json::Map::new();
    for connection in employee_connections {
        let account_pairing_allow_from =
            list_feishu_pairing_allow_from_with_pool(pool, &connection.employee_id)
                .await
                .unwrap_or_default();
        let account_id = connection.employee_id.clone();
        let mut account_config = serde_json::json!({
            "name": connection.name,
            "appId": connection.app_id,
            "appSecret": connection.app_secret,
            "enabled": true,
            "domain": default_domain,
            "connectionMode": default_connection_mode,
            "webhookPath": default_webhook_path,
            "verificationToken": ingress_token,
            "encryptKey": encrypt_key,
            "webhookHost": default_webhook_host,
            "webhookPort": default_webhook_port,
            "configWrites": default_config_writes,
            "dmPolicy": default_dm_policy,
            "groupPolicy": default_group_policy,
            "groupAllowFrom": default_group_allow_from,
            "groupSenderAllowFrom": default_group_sender_allow_from,
            "requireMention": default_require_mention,
            "groups": default_groups,
            "dms": default_dms,
            "footer": default_footer,
            "markdown": default_markdown,
            "renderMode": default_render_mode,
            "reactionNotifications": default_reaction_notifications,
            "typingIndicator": default_typing_indicator,
            "resolveSenderNames": default_resolve_sender_names,
            "streaming": default_streaming,
            "replyInThread": default_reply_in_thread,
            "historyLimit": default_history_limit,
            "dmHistoryLimit": default_dm_history_limit,
            "groupSessionScope": default_group_session_scope,
            "topicSessionMode": default_topic_session_mode,
            "textChunkLimit": default_text_chunk_limit,
            "chunkMode": default_chunk_mode,
            "blockStreamingCoalesce": default_block_streaming_coalesce,
            "mediaMaxMb": default_media_max_mb,
            "httpTimeoutMs": default_http_timeout_ms,
            "heartbeat": default_heartbeat,
            "dynamicAgentCreation": default_dynamic_agent_creation,
            "tools": default_tools,
            "actions": {
                "reactions": default_actions_reactions
            },
            "allowFrom": merge_pairing_allow_from(None, account_pairing_allow_from),
        });

        if let Some(overrides) = account_overrides
            .as_object()
            .and_then(|entries| entries.get(&account_id))
        {
            merge_json_value(&mut account_config, overrides.clone());
        }

        accounts.insert(account_id, account_config);
    }

    let mut feishu_channel = serde_json::Map::new();
    feishu_channel.insert("enabled".to_string(), serde_json::json!(enabled));
    feishu_channel.insert(
        "defaultAccount".to_string(),
        serde_json::json!(default_account),
    );
    feishu_channel.insert("appId".to_string(), serde_json::json!(app_id));
    feishu_channel.insert("appSecret".to_string(), serde_json::json!(app_secret));
    feishu_channel.insert(
        "verificationToken".to_string(),
        serde_json::json!(ingress_token),
    );
    feishu_channel.insert("encryptKey".to_string(), serde_json::json!(encrypt_key));
    feishu_channel.insert(
        "webhookHost".to_string(),
        serde_json::json!(default_webhook_host),
    );
    feishu_channel.insert(
        "webhookPort".to_string(),
        serde_json::json!(default_webhook_port),
    );
    feishu_channel.insert(
        "configWrites".to_string(),
        serde_json::json!(default_config_writes),
    );
    feishu_channel.insert("domain".to_string(), serde_json::json!(default_domain));
    feishu_channel.insert(
        "connectionMode".to_string(),
        serde_json::json!(default_connection_mode),
    );
    feishu_channel.insert(
        "webhookPath".to_string(),
        serde_json::json!(default_webhook_path),
    );
    feishu_channel.insert("dmPolicy".to_string(), serde_json::json!(default_dm_policy));
    feishu_channel.insert(
        "groupPolicy".to_string(),
        serde_json::json!(default_group_policy),
    );
    feishu_channel.insert("groupAllowFrom".to_string(), default_group_allow_from);
    feishu_channel.insert(
        "groupSenderAllowFrom".to_string(),
        default_group_sender_allow_from,
    );
    feishu_channel.insert(
        "requireMention".to_string(),
        serde_json::json!(default_require_mention),
    );
    feishu_channel.insert("groups".to_string(), default_groups);
    feishu_channel.insert("dms".to_string(), default_dms);
    feishu_channel.insert("footer".to_string(), default_footer);
    feishu_channel.insert("markdown".to_string(), default_markdown);
    feishu_channel.insert(
        "renderMode".to_string(),
        serde_json::json!(default_render_mode),
    );
    feishu_channel.insert(
        "reactionNotifications".to_string(),
        serde_json::json!(default_reaction_notifications),
    );
    feishu_channel.insert(
        "typingIndicator".to_string(),
        serde_json::json!(default_typing_indicator),
    );
    feishu_channel.insert(
        "resolveSenderNames".to_string(),
        serde_json::json!(default_resolve_sender_names),
    );
    feishu_channel.insert(
        "streaming".to_string(),
        serde_json::json!(default_streaming),
    );
    feishu_channel.insert(
        "replyInThread".to_string(),
        serde_json::json!(default_reply_in_thread),
    );
    feishu_channel.insert(
        "historyLimit".to_string(),
        serde_json::json!(default_history_limit),
    );
    feishu_channel.insert(
        "dmHistoryLimit".to_string(),
        serde_json::json!(default_dm_history_limit),
    );
    feishu_channel.insert(
        "groupSessionScope".to_string(),
        serde_json::json!(default_group_session_scope),
    );
    feishu_channel.insert(
        "topicSessionMode".to_string(),
        serde_json::json!(default_topic_session_mode),
    );
    feishu_channel.insert(
        "textChunkLimit".to_string(),
        serde_json::json!(default_text_chunk_limit),
    );
    feishu_channel.insert(
        "chunkMode".to_string(),
        serde_json::json!(default_chunk_mode),
    );
    feishu_channel.insert(
        "blockStreamingCoalesce".to_string(),
        default_block_streaming_coalesce,
    );
    feishu_channel.insert(
        "mediaMaxMb".to_string(),
        serde_json::json!(default_media_max_mb),
    );
    feishu_channel.insert(
        "httpTimeoutMs".to_string(),
        serde_json::json!(default_http_timeout_ms),
    );
    feishu_channel.insert("heartbeat".to_string(), default_heartbeat);
    feishu_channel.insert(
        "dynamicAgentCreation".to_string(),
        default_dynamic_agent_creation,
    );
    feishu_channel.insert("tools".to_string(), default_tools);
    feishu_channel.insert(
        "actions".to_string(),
        serde_json::json!({
            "reactions": default_actions_reactions
        }),
    );
    feishu_channel.insert(
        "allowFrom".to_string(),
        merge_pairing_allow_from(None, default_pairing_allow_from),
    );
    feishu_channel.insert("accounts".to_string(), serde_json::Value::Object(accounts));

    Ok(serde_json::json!({
        "channels": {
            "feishu": feishu_channel
        },
        "plugins": {
            "entries": {}
        },
        "tools": {
            "profile": "default"
        }
    }))
}

pub(crate) async fn get_openclaw_plugin_feishu_advanced_settings_with_pool(
    pool: &SqlitePool,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    Ok(OpenClawPluginFeishuAdvancedSettings {
        groups_json: get_app_setting(pool, "feishu_groups")
            .await?
            .unwrap_or_default(),
        dms_json: get_app_setting(pool, "feishu_dms")
            .await?
            .unwrap_or_default(),
        footer_json: get_app_setting(pool, "feishu_footer")
            .await?
            .unwrap_or_default(),
        account_overrides_json: get_app_setting(pool, "feishu_account_overrides")
            .await?
            .unwrap_or_default(),
        render_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_render_mode").await?,
            "auto",
        ),
        streaming: app_setting_string_or_default(
            get_app_setting(pool, "feishu_streaming").await?,
            "false",
        ),
        text_chunk_limit: app_setting_string_or_default(
            get_app_setting(pool, "feishu_text_chunk_limit").await?,
            "4000",
        ),
        chunk_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_chunk_mode").await?,
            "length",
        ),
        reply_in_thread: app_setting_string_or_default(
            get_app_setting(pool, "feishu_reply_in_thread").await?,
            "disabled",
        ),
        group_session_scope: app_setting_string_or_default(
            get_app_setting(pool, "feishu_group_session_scope").await?,
            "group",
        ),
        topic_session_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_topic_session_mode").await?,
            "disabled",
        ),
        markdown_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_markdown_mode").await?,
            "native",
        ),
        markdown_table_mode: app_setting_string_or_default(
            get_app_setting(pool, "feishu_markdown_table_mode").await?,
            "native",
        ),
        heartbeat_visibility: app_setting_string_or_default(
            get_app_setting(pool, "feishu_heartbeat_visibility").await?,
            "visible",
        ),
        heartbeat_interval_ms: app_setting_string_or_default(
            get_app_setting(pool, "feishu_heartbeat_interval_ms").await?,
            "30000",
        ),
        media_max_mb: app_setting_string_or_default(
            get_app_setting(pool, "feishu_media_max_mb").await?,
            "20",
        ),
        http_timeout_ms: app_setting_string_or_default(
            get_app_setting(pool, "feishu_http_timeout_ms").await?,
            "60000",
        ),
        config_writes: app_setting_string_or_default(
            get_app_setting(pool, "feishu_config_writes").await?,
            "false",
        ),
        webhook_host: get_app_setting(pool, "feishu_webhook_host")
            .await?
            .unwrap_or_default(),
        webhook_port: get_app_setting(pool, "feishu_webhook_port")
            .await?
            .unwrap_or_default(),
        dynamic_agent_creation_enabled: get_app_setting(
            pool,
            "feishu_dynamic_agent_creation_enabled",
        )
        .await?
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .unwrap_or_else(|| "false".to_string()),
        dynamic_agent_creation_workspace_template: get_app_setting(
            pool,
            "feishu_dynamic_agent_creation_workspace_template",
        )
        .await?
        .unwrap_or_default(),
        dynamic_agent_creation_agent_dir_template: get_app_setting(
            pool,
            "feishu_dynamic_agent_creation_agent_dir_template",
        )
        .await?
        .unwrap_or_default(),
        dynamic_agent_creation_max_agents: get_app_setting(
            pool,
            "feishu_dynamic_agent_creation_max_agents",
        )
        .await?
        .unwrap_or_default(),
    })
}

pub(crate) async fn set_openclaw_plugin_feishu_advanced_settings_with_pool(
    pool: &SqlitePool,
    settings: &OpenClawPluginFeishuAdvancedSettings,
) -> Result<OpenClawPluginFeishuAdvancedSettings, String> {
    set_app_setting(pool, "feishu_groups", settings.groups_json.trim()).await?;
    set_app_setting(pool, "feishu_dms", settings.dms_json.trim()).await?;
    set_app_setting(pool, "feishu_footer", settings.footer_json.trim()).await?;
    set_app_setting(
        pool,
        "feishu_account_overrides",
        settings.account_overrides_json.trim(),
    )
    .await?;
    set_app_setting(pool, "feishu_render_mode", settings.render_mode.trim()).await?;
    set_app_setting(pool, "feishu_streaming", settings.streaming.trim()).await?;
    set_app_setting(
        pool,
        "feishu_text_chunk_limit",
        settings.text_chunk_limit.trim(),
    )
    .await?;
    set_app_setting(pool, "feishu_chunk_mode", settings.chunk_mode.trim()).await?;
    set_app_setting(
        pool,
        "feishu_reply_in_thread",
        settings.reply_in_thread.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_group_session_scope",
        settings.group_session_scope.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_topic_session_mode",
        settings.topic_session_mode.trim(),
    )
    .await?;
    set_app_setting(pool, "feishu_markdown_mode", settings.markdown_mode.trim()).await?;
    set_app_setting(
        pool,
        "feishu_markdown_table_mode",
        settings.markdown_table_mode.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_heartbeat_visibility",
        settings.heartbeat_visibility.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_heartbeat_interval_ms",
        settings.heartbeat_interval_ms.trim(),
    )
    .await?;
    set_app_setting(pool, "feishu_media_max_mb", settings.media_max_mb.trim()).await?;
    set_app_setting(
        pool,
        "feishu_http_timeout_ms",
        settings.http_timeout_ms.trim(),
    )
    .await?;
    set_app_setting(pool, "feishu_config_writes", settings.config_writes.trim()).await?;
    set_app_setting(pool, "feishu_webhook_host", settings.webhook_host.trim()).await?;
    set_app_setting(pool, "feishu_webhook_port", settings.webhook_port.trim()).await?;
    set_app_setting(
        pool,
        "feishu_dynamic_agent_creation_enabled",
        settings.dynamic_agent_creation_enabled.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_dynamic_agent_creation_workspace_template",
        settings.dynamic_agent_creation_workspace_template.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_dynamic_agent_creation_agent_dir_template",
        settings.dynamic_agent_creation_agent_dir_template.trim(),
    )
    .await?;
    set_app_setting(
        pool,
        "feishu_dynamic_agent_creation_max_agents",
        settings.dynamic_agent_creation_max_agents.trim(),
    )
    .await?;
    get_openclaw_plugin_feishu_advanced_settings_with_pool(pool).await
}
