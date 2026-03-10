mod helpers;

use runtime_lib::commands::im_routing::{
    list_im_routing_bindings_with_pool, upsert_im_routing_binding_with_pool,
    UpsertImRoutingBindingInput,
};

#[tokio::test]
async fn upsert_and_list_routing_bindings() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    upsert_im_routing_binding_with_pool(
        &pool,
        UpsertImRoutingBindingInput {
            id: None,
            agent_id: "main".to_string(),
            channel: "feishu".to_string(),
            account_id: "*".to_string(),
            peer_kind: "".to_string(),
            peer_id: "".to_string(),
            guild_id: "".to_string(),
            team_id: "".to_string(),
            role_ids: vec![],
            connector_meta: serde_json::json!({}),
            priority: 100,
            enabled: true,
        },
    )
    .await
    .expect("upsert");

    let rows = list_im_routing_bindings_with_pool(&pool).await.expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].channel, "feishu");
    assert_eq!(rows[0].connector_meta, serde_json::json!({}));
}

#[tokio::test]
async fn upsert_and_list_routing_bindings_preserve_connector_metadata() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    upsert_im_routing_binding_with_pool(
        &pool,
        UpsertImRoutingBindingInput {
            id: None,
            agent_id: "discord-agent".to_string(),
            channel: "Discord".to_string(),
            account_id: "guild-1".to_string(),
            peer_kind: "thread".to_string(),
            peer_id: "room-9".to_string(),
            guild_id: "guild-1".to_string(),
            team_id: "".to_string(),
            role_ids: vec!["admin".to_string()],
            connector_meta: serde_json::json!({
                "workspace_id": "guild-1",
                "connector_id": "discord-primary"
            }),
            priority: 90,
            enabled: true,
        },
    )
    .await
    .expect("upsert");

    let rows = list_im_routing_bindings_with_pool(&pool).await.expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].channel, "discord");
    assert_eq!(rows[0].connector_meta["workspace_id"], "guild-1");
    assert_eq!(rows[0].connector_meta["connector_id"], "discord-primary");
}
