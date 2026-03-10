mod helpers;

use runtime_lib::commands::im_gateway::{list_recent_im_threads_with_pool, process_im_event};
use runtime_lib::im::types::{ImEvent, ImEventType};

#[tokio::test]
async fn list_recent_threads_returns_latest_per_thread() {
    let (pool, _tmp) = helpers::setup_test_db().await;

    process_im_event(
        &pool,
        ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-a".to_string(),
            event_id: Some("evt-a-1".to_string()),
            message_id: Some("msg-a-1".to_string()),
            text: Some("hello a1".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: None,
        },
    )
    .await
    .expect("insert a1");

    process_im_event(
        &pool,
        ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-b".to_string(),
            event_id: Some("evt-b-1".to_string()),
            message_id: Some("msg-b-1".to_string()),
            text: Some("hello b1".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: None,
        },
    )
    .await
    .expect("insert b1");

    process_im_event(
        &pool,
        ImEvent {
            channel: "feishu".to_string(),
            event_type: ImEventType::MessageCreated,
            thread_id: "chat-a".to_string(),
            event_id: Some("evt-a-2".to_string()),
            message_id: Some("msg-a-2".to_string()),
            text: Some("hello a2".to_string()),
            role_id: None,
            account_id: None,
            tenant_id: None,
        },
    )
    .await
    .expect("insert a2");

    let threads = list_recent_im_threads_with_pool(&pool, 10)
        .await
        .expect("list threads");
    assert_eq!(threads.len(), 2);
    assert_eq!(threads[0].thread_id, "chat-a");
    assert_eq!(threads[0].last_text_preview, "hello a2");
}
