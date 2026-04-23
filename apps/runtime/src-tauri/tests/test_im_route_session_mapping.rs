mod helpers;

use runtime_lib::commands::employee_agents::{
    ensure_agent_sessions_for_event_with_pool, ensure_employee_sessions_for_event_with_pool,
    resolve_agent_session_dispatches_for_event_with_pool, upsert_agent_employee_with_pool,
    UpsertAgentEmployeeInput,
};
use runtime_lib::im::types::{ImEvent, ImEventType};
use runtime_lib::im::resolve_agent_session_dispatches_with_pool;

#[tokio::test]
async fn different_threads_do_not_reuse_existing_session() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    helpers::seed_default_model_config(&pool).await;

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert employee");

    let first = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-1".to_string()),
            message_id: Some("msg-1".to_string()),
            text: Some("hello".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
            conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            parent_conversation_candidates: Vec::new(),
            conversation_scope: Some("peer".to_string()),
        },
    )
    .await
    .expect("first ensure");
    assert_eq!(first.len(), 1);
    assert!(first[0].created);

    let second = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-2".to_string(),
            event_id: Some("evt-2".to_string()),
            message_id: Some("msg-2".to_string()),
            text: Some("hello 2".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
            conversation_id: Some("feishu:tenant-a:group:chat-2".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-2".to_string()),
            parent_conversation_candidates: Vec::new(),
            conversation_scope: Some("peer".to_string()),
        },
    )
    .await
    .expect("second ensure");
    assert_eq!(second.len(), 1);
    assert!(second[0].created);
    assert_ne!(second[0].session_id, first[0].session_id);

    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM im_thread_sessions WHERE session_id = ?")
            .bind(&first[0].session_id)
            .fetch_one(&pool)
            .await
            .expect("count mappings");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn same_thread_creates_distinct_session_when_mention_switches_employee() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    helpers::seed_default_model_config(&pool).await;

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_main".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert main");

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "dev_team".to_string(),
            name: "开发团队".to_string(),
            role_id: "dev_team".to_string(),
            persona: "".to_string(),
            feishu_open_id: "ou_dev_team".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "dev_team".to_string(),
            routing_priority: 90,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: false,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert dev");

    let first = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-1".to_string()),
            message_id: Some("msg-1".to_string()),
            text: Some("先给一个初步方案".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
            conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            parent_conversation_candidates: Vec::new(),
            conversation_scope: Some("peer".to_string()),
        },
    )
    .await
    .expect("first ensure");
    assert_eq!(first.len(), 1);
    assert!(first[0].created);

    let second = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-2".to_string()),
            message_id: Some("msg-2".to_string()),
            text: Some("@开发团队 细化技术方案".to_string()),
            role_id: Some("ou_dev_team".to_string()),
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
            conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            parent_conversation_candidates: Vec::new(),
            conversation_scope: Some("peer".to_string()),
        },
    )
    .await
    .expect("second ensure");

    assert_eq!(second.len(), 1);
    assert!(second[0].created);
    assert_ne!(second[0].session_id, first[0].session_id);

    let (mapping_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM im_thread_sessions WHERE thread_id = 'chat-1'")
            .fetch_one(&pool)
            .await
            .expect("count thread mappings");
    assert_eq!(mapping_count, 2);

    let (distinct_session_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT session_id) FROM im_thread_sessions WHERE thread_id = 'chat-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("count distinct session ids");
    assert_eq!(distinct_session_count, 2);
}

#[tokio::test]
async fn recreates_session_when_thread_mapping_points_to_deleted_session() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    helpers::seed_default_model_config(&pool).await;

    let employee_db_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert employee");

    let first = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-1".to_string()),
            message_id: Some("msg-1".to_string()),
            text: Some("hello".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
            conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            parent_conversation_candidates: Vec::new(),
            conversation_scope: Some("peer".to_string()),
        },
    )
    .await
    .expect("first ensure");
    assert_eq!(first.len(), 1);
    let stale_session_id = first[0].session_id.clone();
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&stale_session_id)
        .execute(&pool)
        .await
        .expect("delete stale session");

    let second = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-1".to_string(),
            event_id: Some("evt-2".to_string()),
            message_id: Some("msg-2".to_string()),
            text: Some("hello again".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: None,
            conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-1".to_string()),
            parent_conversation_candidates: Vec::new(),
            conversation_scope: Some("peer".to_string()),
        },
    )
    .await
    .expect("second ensure");

    assert_eq!(second.len(), 1);
    assert!(second[0].created);
    assert_ne!(second[0].session_id, stale_session_id);

    let (session_exists,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE id = ?")
        .bind(&second[0].session_id)
        .fetch_one(&pool)
        .await
        .expect("query recreated session");
    assert_eq!(session_exists, 1);

    let (mapped_session_id,): (String,) = sqlx::query_as(
        "SELECT session_id FROM im_thread_sessions WHERE thread_id = ? AND employee_id = ?",
    )
    .bind("chat-1")
    .bind(&employee_db_id)
    .fetch_one(&pool)
    .await
    .expect("query thread mapping");
    assert_eq!(mapped_session_id, second[0].session_id);
}

#[tokio::test]
async fn same_thread_same_employee_different_conversations_get_distinct_sessions() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    helpers::seed_default_model_config(&pool).await;

    let employee_db_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert employee");

    let first = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-topic".to_string(),
            event_id: Some("evt-topic-1".to_string()),
            message_id: Some("msg-topic-1".to_string()),
            text: Some("topic 1".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: Some("ou_sender".to_string()),
            chat_type: Some("group".to_string()),
            conversation_id: Some("feishu:tenant-a:group:chat-topic:topic:om_root_1".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-topic".to_string()),
            parent_conversation_candidates: vec!["feishu:tenant-a:group:chat-topic".to_string()],
            conversation_scope: Some("topic".to_string()),
        },
    )
    .await
    .expect("ensure first topic session");

    let second = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-topic".to_string(),
            event_id: Some("evt-topic-2".to_string()),
            message_id: Some("msg-topic-2".to_string()),
            text: Some("topic 2".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: Some("ou_sender".to_string()),
            chat_type: Some("group".to_string()),
            conversation_id: Some("feishu:tenant-a:group:chat-topic:topic:om_root_2".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-topic".to_string()),
            parent_conversation_candidates: vec!["feishu:tenant-a:group:chat-topic".to_string()],
            conversation_scope: Some("topic".to_string()),
        },
    )
    .await
    .expect("ensure second topic session");

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].session_id, second[0].session_id);

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM im_conversation_sessions WHERE thread_id = 'chat-topic' AND employee_id = ?",
    )
    .bind(&employee_db_id)
    .fetch_one(&pool)
    .await
    .expect("count conversation mappings");
    assert_eq!(count, 2);
}

#[tokio::test]
async fn agent_session_runtime_resolves_dispatches_through_binding_entrypoint() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    helpers::seed_default_model_config(&pool).await;

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "agent-main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert employee");

    let dispatches = resolve_agent_session_dispatches_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-runtime".to_string(),
            event_id: Some("evt-runtime-1".to_string()),
            message_id: Some("msg-runtime-1".to_string()),
            text: Some("hello runtime".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: Some("group".to_string()),
            conversation_id: Some("feishu:tenant-a:group:chat-runtime".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-runtime".to_string()),
            parent_conversation_candidates: Vec::new(),
            conversation_scope: Some("peer".to_string()),
        },
        Some(&serde_json::json!({
            "agentId": "agent-main",
            "sessionKey": "feishu:tenant-a:agent-main:feishu:tenant-a:group:chat-runtime",
            "matchedBy": "openclaw",
        })),
    )
    .await
    .expect("resolve dispatches");

    assert_eq!(dispatches.len(), 1);
    assert_eq!(dispatches[0].route_agent_id, "agent-main");
    assert_eq!(dispatches[0].matched_by, "openclaw");
}

#[tokio::test]
async fn employee_wrappers_match_agent_first_session_views() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    helpers::seed_default_model_config(&pool).await;

    upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "agent-main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert employee");

    let event = ImEvent {
        channel: "feishu".to_string(),
        event_type: ImEventType::MessageCreated,
        thread_id: "chat-agent-alias".to_string(),
        event_id: Some("evt-agent-alias".to_string()),
        message_id: Some("msg-agent-alias".to_string()),
        text: Some("hello aliases".to_string()),
        role_id: None,
        account_id: None,
        tenant_id: Some("tenant-a".to_string()),
        sender_id: None,
        chat_type: Some("group".to_string()),
        conversation_id: Some("feishu:tenant-a:group:chat-agent-alias".to_string()),
        base_conversation_id: Some("feishu:tenant-a:group:chat-agent-alias".to_string()),
        parent_conversation_candidates: Vec::new(),
        conversation_scope: Some("peer".to_string()),
    };

    let employee_sessions = ensure_employee_sessions_for_event_with_pool(&pool, &event)
        .await
        .expect("ensure employee sessions");
    let agent_sessions = ensure_agent_sessions_for_event_with_pool(&pool, &event)
        .await
        .expect("ensure agent sessions");

    assert_eq!(employee_sessions.len(), 1);
    assert_eq!(agent_sessions.len(), 1);
    assert_eq!(employee_sessions[0].employee_id, agent_sessions[0].agent_id);
    assert_eq!(
        employee_sessions[0].employee_name,
        agent_sessions[0].agent_name
    );
    assert_eq!(
        employee_sessions[0].session_id,
        agent_sessions[0].session_id
    );

    let agent_dispatches = resolve_agent_session_dispatches_for_event_with_pool(
        &pool,
        &event,
        Some(&serde_json::json!({
            "agentId": "agent-main",
            "sessionKey": "feishu:tenant-a:agent-main:feishu:tenant-a:group:chat-agent-alias",
            "matchedBy": "openclaw",
        })),
    )
    .await
    .expect("resolve agent dispatches");

    assert_eq!(agent_dispatches.len(), 1);
    assert_eq!(
        agent_dispatches[0].agent_id,
        employee_sessions[0].employee_id
    );
    assert_eq!(
        agent_dispatches[0].agent_name,
        employee_sessions[0].employee_name
    );
}

#[tokio::test]
async fn conversation_rows_beat_blank_legacy_thread_rows_on_same_thread() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    helpers::seed_default_model_config(&pool).await;

    let employee_db_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert employee");

    for session_id in ["legacy-session", "topic-1-session"] {
        sqlx::query(
            "INSERT INTO sessions (
                id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id
             )
             VALUES (?, 'builtin-general', ?, '2026-04-22T00:00:00Z', 'm1', 'standard', '', 'main', 'general', '')",
        )
        .bind(session_id)
        .bind(format!("session {session_id}"))
        .execute(&pool)
        .await
        .expect("seed session");
    }

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
            'chat-topic',
            ?,
            'legacy-session',
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
    .bind(&employee_db_id)
    .execute(&pool)
    .await
    .expect("seed blank legacy thread row");

    sqlx::query(
        "INSERT INTO im_conversation_sessions (
            conversation_id,
            employee_id,
            thread_id,
            session_id,
            route_session_key,
            created_at,
            updated_at,
            channel,
            account_id,
            base_conversation_id,
            parent_conversation_candidates_json,
            scope,
            peer_kind,
            peer_id,
            topic_id,
            sender_id
         )
         VALUES (
            'feishu:tenant-a:group:chat-topic:topic:om_root_1',
            ?,
            'chat-topic',
            'topic-1-session',
            'feishu:tenant-a:main:feishu:tenant-a:group:chat-topic:topic:om_root_1',
            '2026-04-22T00:00:00Z',
            '2026-04-22T00:00:01Z',
            'feishu',
            'tenant-a',
            'feishu:tenant-a:group:chat-topic',
            '[\"feishu:tenant-a:group:chat-topic\"]',
            'topic',
            'group',
            'chat-topic',
            'om_root_1',
            'ou_sender'
         )",
    )
    .bind(&employee_db_id)
    .execute(&pool)
    .await
    .expect("seed authoritative conversation row");

    let ensured = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-topic".to_string(),
            event_id: Some("evt-topic-2".to_string()),
            message_id: Some("msg-topic-2".to_string()),
            text: Some("topic 2".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: Some("ou_sender".to_string()),
            chat_type: Some("group".to_string()),
            conversation_id: Some("feishu:tenant-a:group:chat-topic:topic:om_root_2".to_string()),
            base_conversation_id: Some("feishu:tenant-a:group:chat-topic".to_string()),
            parent_conversation_candidates: vec!["feishu:tenant-a:group:chat-topic".to_string()],
            conversation_scope: Some("topic".to_string()),
        },
    )
    .await
    .expect("ensure second topic session");

    assert_eq!(ensured.len(), 1);
    assert!(ensured[0].created);
    assert_ne!(ensured[0].session_id, "legacy-session");
    assert_ne!(ensured[0].session_id, "topic-1-session");
}

#[tokio::test]
async fn migrated_blank_channel_same_id_conversation_rows_still_block_legacy_thread_fallback() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    helpers::seed_default_model_config(&pool).await;

    let employee_db_id = upsert_agent_employee_with_pool(
        &pool,
        UpsertAgentEmployeeInput {
            id: None,
            employee_id: "main".to_string(),
            name: "主员工".to_string(),
            role_id: "main".to_string(),
            persona: "".to_string(),
            feishu_open_id: "".to_string(),
            feishu_app_id: "".to_string(),
            feishu_app_secret: "".to_string(),
            primary_skill_id: "builtin-general".to_string(),
            default_work_dir: "".to_string(),
            openclaw_agent_id: "main".to_string(),
            routing_priority: 100,
            enabled_scopes: vec!["feishu".to_string()],
            enabled: true,
            is_default: true,
            skill_ids: vec![],
        },
    )
    .await
    .expect("upsert employee");

    for session_id in [
        "legacy-session",
        "migrated-conversation-session",
        "new-conversation-session",
    ] {
        sqlx::query(
            "INSERT INTO sessions (
                id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id, session_mode, team_id
             )
             VALUES (?, 'builtin-general', ?, '2026-04-22T00:00:00Z', 'm1', 'standard', '', 'main', 'general', '')",
        )
        .bind(session_id)
        .bind(format!("session {session_id}"))
        .execute(&pool)
        .await
        .expect("seed session");
    }

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
            'chat-migrated',
            ?,
            'legacy-session',
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
    .bind(&employee_db_id)
    .execute(&pool)
    .await
    .expect("seed legacy thread row");

    sqlx::query(
        "INSERT INTO im_conversation_sessions (
            conversation_id,
            employee_id,
            thread_id,
            session_id,
            route_session_key,
            created_at,
            updated_at,
            channel,
            account_id,
            base_conversation_id,
            parent_conversation_candidates_json,
            scope,
            peer_kind,
            peer_id,
            topic_id,
            sender_id
         )
         VALUES (
            'chat-migrated',
            ?,
            'chat-migrated',
            'migrated-conversation-session',
            '',
            '2026-04-22T00:00:00Z',
            '2026-04-22T00:00:01Z',
            '',
            '',
            'chat-migrated',
            '[]',
            '',
            '',
            'chat-migrated',
            '',
            ''
         )",
    )
    .bind(&employee_db_id)
    .execute(&pool)
    .await
    .expect("seed migrated conversation row");

    let ensured = ensure_employee_sessions_for_event_with_pool(
        &pool,
        &ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-migrated".to_string(),
            event_id: Some("evt-migrated".to_string()),
            message_id: Some("msg-migrated".to_string()),
            text: Some("migrated blank channel row".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: Some("tenant-a".to_string()),
            sender_id: None,
            chat_type: Some("group".to_string()),
            conversation_id: Some("feishu:tenant-a:group:chat-migrated:topic:om_root_2".to_string()),
            base_conversation_id: Some("chat-migrated".to_string()),
            parent_conversation_candidates: vec!["chat-migrated".to_string()],
            conversation_scope: Some("topic".to_string()),
        },
    )
    .await
    .expect("ensure migrated conversation session");

    assert_eq!(ensured.len(), 1);
    assert!(ensured[0].created);
    assert_ne!(ensured[0].session_id, "legacy-session");
    assert_ne!(ensured[0].session_id, "migrated-conversation-session");

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM im_conversation_sessions
         WHERE thread_id = 'chat-migrated' AND employee_id = ?",
    )
    .bind(&employee_db_id)
    .fetch_one(&pool)
    .await
    .expect("count migrated thread conversation mappings");
    assert_eq!(count, 2);
}
