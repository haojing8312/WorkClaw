mod helpers;

use runtime_lib::commands::im_gateway::process_im_event;
use runtime_lib::im::types::{ImEvent, ImEventType};

#[tokio::test]
async fn callback_same_event_id_is_processed_once() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let event = ImEvent {
        event_type: ImEventType::MessageCreated,
        thread_id: "thread-a".to_string(),
        event_id: Some("evt-001".to_string()),
        message_id: Some("msg-001".to_string()),
        text: Some("hello".to_string()),
        role_id: None,
        tenant_id: None,
    };

    let first = process_im_event(&pool, event.clone())
        .await
        .expect("first callback should pass");
    assert!(first.accepted);
    assert!(!first.deduped);

    let second = process_im_event(&pool, event)
        .await
        .expect("second callback should pass");
    assert!(second.accepted);
    assert!(second.deduped);
}

