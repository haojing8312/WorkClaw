mod helpers;
#[path = "../src/db/migrations.rs"]
mod db_migrations;

use runtime_lib::im::{
    find_agent_conversation_binding, find_agent_conversation_binding_for_candidates,
    find_channel_delivery_route, upsert_agent_conversation_binding, upsert_channel_delivery_route,
    AgentConversationBindingUpsert, ChannelDeliveryRouteUpsert,
};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn legacy_thread_only_db_gains_conversation_binding_tables() {
    let (pool, _tmp) = helpers::setup_legacy_thread_only_db().await;

    sqlx::query(
        "INSERT INTO im_thread_sessions (
            thread_id,
            employee_id,
            session_id,
            route_session_key,
            created_at,
            updated_at
         )
         VALUES (
            'legacy-thread',
            'emp-legacy',
            'session-legacy',
            '',
            '2026-04-22T00:00:00Z',
            '2026-04-22T00:00:01Z'
         )",
    )
    .execute(&pool)
    .await
    .expect("seed legacy thread row");

    db_migrations::apply_legacy_migrations_for_test(&pool)
        .await
        .expect("apply legacy migrations");

    let counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM im_conversation_sessions),
            (SELECT COUNT(*) FROM agent_conversation_bindings),
            (SELECT COUNT(*) FROM channel_delivery_routes)",
    )
    .fetch_one(&pool)
    .await
    .expect("query migrated authority counts");

    assert_eq!(counts.0, 1, "expected conversation session backfill");
    assert_eq!(counts.1, 1, "expected agent conversation binding backfill");
    assert_eq!(counts.2, 1, "expected channel delivery route backfill");
}

#[tokio::test]
async fn legacy_migration_creates_channel_delivery_route_channel_account_index_and_normalizes_blank_peer_id()
{
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("create sqlite memory pool");

    sqlx::query(
        "CREATE TABLE im_thread_sessions (
            thread_id TEXT NOT NULL,
            employee_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            route_session_key TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            channel TEXT NOT NULL DEFAULT '',
            account_id TEXT NOT NULL DEFAULT '',
            conversation_id TEXT NOT NULL DEFAULT '',
            base_conversation_id TEXT NOT NULL DEFAULT '',
            parent_conversation_candidates_json TEXT NOT NULL DEFAULT '[]',
            scope TEXT NOT NULL DEFAULT '',
            peer_kind TEXT NOT NULL DEFAULT '',
            peer_id TEXT NOT NULL DEFAULT '',
            topic_id TEXT NOT NULL DEFAULT '',
            sender_id TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (thread_id, employee_id)
        )",
    )
    .execute(&pool)
    .await
    .expect("create legacy thread sessions table with cutover columns");

    sqlx::query(
        "INSERT INTO im_thread_sessions (
            thread_id,
            employee_id,
            session_id,
            route_session_key,
            created_at,
            updated_at,
            channel,
            account_id,
            conversation_id,
            base_conversation_id,
            parent_conversation_candidates_json,
            scope,
            peer_kind,
            peer_id,
            topic_id,
            sender_id
         )
         VALUES (
            'legacy-thread',
            'emp-legacy',
            'session-legacy',
            '',
            '2026-04-22T00:00:00Z',
            '2026-04-22T00:00:01Z',
            '',
            '',
            '',
            '',
            '[]',
            '',
            '',
            '',
            '',
            ''
         )",
    )
    .execute(&pool)
    .await
    .expect("seed blank peer id legacy row");

    db_migrations::apply_legacy_migrations_for_test(&pool)
        .await
        .expect("apply legacy migrations");

    let peer_id: String = sqlx::query_scalar(
        "SELECT peer_id
         FROM im_conversation_sessions
         WHERE conversation_id = 'legacy-thread' AND employee_id = 'emp-legacy'",
    )
    .fetch_one(&pool)
    .await
    .expect("query normalized peer id");
    assert_eq!(peer_id, "legacy-thread");

    let index_names: Vec<String> = sqlx::query_scalar(
        "SELECT name
         FROM sqlite_master
         WHERE type = 'index'
           AND name = 'idx_channel_delivery_routes_channel_account'",
    )
    .fetch_all(&pool)
    .await
    .expect("query channel delivery route channel/account index");
    assert_eq!(
        index_names,
        vec!["idx_channel_delivery_routes_channel_account".to_string()]
    );
}

async fn setup_openclaw_binding_store_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("create sqlite memory pool");

    sqlx::query(
        "CREATE TABLE agent_conversation_bindings (
            conversation_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            account_id TEXT NOT NULL DEFAULT '',
            agent_id TEXT NOT NULL,
            session_key TEXT NOT NULL,
            session_id TEXT NOT NULL DEFAULT '',
            base_conversation_id TEXT NOT NULL DEFAULT '',
            parent_conversation_candidates_json TEXT NOT NULL DEFAULT '[]',
            scope TEXT NOT NULL DEFAULT '',
            peer_kind TEXT NOT NULL DEFAULT '',
            peer_id TEXT NOT NULL DEFAULT '',
            topic_id TEXT NOT NULL DEFAULT '',
            sender_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (conversation_id, agent_id)
        )",
    )
    .execute(&pool)
    .await
    .expect("create agent_conversation_bindings");

    sqlx::query(
        "CREATE TABLE channel_delivery_routes (
            session_key TEXT NOT NULL PRIMARY KEY,
            channel TEXT NOT NULL,
            account_id TEXT NOT NULL DEFAULT '',
            conversation_id TEXT NOT NULL,
            reply_target TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .expect("create channel_delivery_routes");

    pool
}

#[tokio::test]
async fn openclaw_binding_store_works_without_legacy_im_tables() {
    let pool = setup_openclaw_binding_store_pool().await;
    let parent_candidates = vec!["feishu:tenant-a:group:chat-1".to_string()];

    let binding = AgentConversationBindingUpsert {
        conversation_id: "feishu:tenant-a:group:chat-1:topic:om_root_1",
        channel: "feishu",
        account_id: "tenant-a",
        agent_id: "main-agent",
        session_key: "agent/main-agent/conversation/chat-1-topic-1",
        session_id: "session-123",
        base_conversation_id: "feishu:tenant-a:group:chat-1",
        parent_conversation_candidates: &parent_candidates,
        scope: "topic",
        peer_kind: "group",
        peer_id: "chat-1",
        topic_id: "om_root_1",
        sender_id: "ou_sender",
        created_at: "2026-04-22T00:00:00Z",
        updated_at: "2026-04-22T00:00:00Z",
    };
    upsert_agent_conversation_binding(&pool, &binding)
        .await
        .expect("upsert agent conversation binding");

    let route = ChannelDeliveryRouteUpsert {
        session_key: binding.session_key,
        channel: binding.channel,
        account_id: binding.account_id,
        conversation_id: binding.conversation_id,
        reply_target: "om_root_1",
        updated_at: "2026-04-22T00:00:01Z",
    };
    upsert_channel_delivery_route(&pool, &route)
        .await
        .expect("upsert channel delivery route");

    let stored_binding =
        find_agent_conversation_binding(&pool, binding.conversation_id, binding.agent_id)
            .await
            .expect("find binding")
            .expect("binding exists");
    assert_eq!(stored_binding.conversation_id, binding.conversation_id);
    assert_eq!(stored_binding.agent_id, binding.agent_id);
    assert_eq!(stored_binding.session_key, binding.session_key);
    assert_eq!(
        stored_binding.parent_conversation_candidates,
        parent_candidates
    );

    let stored_route = find_channel_delivery_route(&pool, binding.session_key)
        .await
        .expect("find delivery route")
        .expect("delivery route exists");
    assert_eq!(stored_route.session_key, route.session_key);
    assert_eq!(stored_route.reply_target, route.reply_target);

    let legacy_tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master
         WHERE type = 'table'
         AND name IN ('im_thread_sessions', 'im_conversation_sessions')",
    )
    .fetch_all(&pool)
    .await
    .expect("query legacy tables");
    assert!(legacy_tables.is_empty());
}

#[tokio::test]
async fn openclaw_binding_store_falls_back_to_parent_candidates() {
    let pool = setup_openclaw_binding_store_pool().await;

    upsert_agent_conversation_binding(
        &pool,
        &AgentConversationBindingUpsert {
            conversation_id: "feishu:tenant-a:group:chat-1",
            channel: "feishu",
            account_id: "tenant-a",
            agent_id: "main-agent",
            session_key: "agent/main-agent/conversation/chat-1",
            session_id: "session-parent",
            base_conversation_id: "feishu:tenant-a:group:chat-1",
            parent_conversation_candidates: &[],
            scope: "peer",
            peer_kind: "group",
            peer_id: "chat-1",
            topic_id: "",
            sender_id: "",
            created_at: "2026-04-22T00:00:00Z",
            updated_at: "2026-04-22T00:00:00Z",
        },
    )
    .await
    .expect("upsert parent binding");

    let binding = find_agent_conversation_binding_for_candidates(
        &pool,
        "feishu:tenant-a:group:chat-1:topic:om_root_2",
        &["feishu:tenant-a:group:chat-1".to_string()],
        "main-agent",
    )
    .await
    .expect("resolve by parent candidates")
    .expect("binding exists");

    assert_eq!(binding.conversation_id, "feishu:tenant-a:group:chat-1");
    assert_eq!(binding.session_id, "session-parent");
}
