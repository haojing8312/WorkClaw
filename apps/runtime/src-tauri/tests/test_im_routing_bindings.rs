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
            priority: 100,
            enabled: true,
        },
    )
    .await
    .expect("upsert");

    let rows = list_im_routing_bindings_with_pool(&pool).await.expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].channel, "feishu");
}
