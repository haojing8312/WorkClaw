mod helpers;

use runtime_lib::commands::im_config::{
    bind_thread_roles_with_pool, get_thread_role_config_with_pool,
};

#[tokio::test]
async fn group_thread_can_bind_multiple_roles() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let roles = vec![
        "presales".to_string(),
        "pm".to_string(),
        "consultant".to_string(),
    ];

    bind_thread_roles_with_pool(
        &pool,
        "thread-oppty-001",
        "tenant-a",
        "opportunity_review",
        &roles,
    )
    .await
    .expect("bind roles to thread");

    let cfg = get_thread_role_config_with_pool(&pool, "thread-oppty-001")
        .await
        .expect("read thread config");

    assert_eq!(cfg.thread_id, "thread-oppty-001");
    assert_eq!(cfg.tenant_id, "tenant-a");
    assert_eq!(cfg.scenario_template, "opportunity_review");
    assert_eq!(cfg.roles.len(), 3);
    assert_eq!(cfg.roles[0], "presales");
    assert_eq!(cfg.roles[1], "pm");
    assert_eq!(cfg.roles[2], "consultant");
}
