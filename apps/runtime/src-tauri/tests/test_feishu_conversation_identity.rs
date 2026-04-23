use runtime_lib::commands::feishu_gateway::{parse_feishu_payload, ParsedFeishuPayload};
use runtime_lib::commands::im_host::parse_normalized_im_event_value;
#[test]
fn parse_feishu_payload_assigns_peer_conversation_metadata() {
    let payload = serde_json::json!({
        "header": {
            "event_id": "evt_peer_1",
            "event_type": "im.message.receive_v1",
            "tenant_key": "tenant_peer"
        },
        "event": {
            "message": {
                "message_id": "om_peer_1",
                "chat_id": "oc_peer_1",
                "chat_type": "group",
                "content": "{\"text\":\"请继续推进\"}"
            },
            "sender": {
                "sender_id": {
                    "open_id": "ou_sender_1"
                }
            }
        }
    });

    let parsed = parse_feishu_payload(&payload.to_string()).expect("payload should parse");
    match parsed {
        ParsedFeishuPayload::Event(event) => {
            assert_eq!(
                event.conversation_id.as_deref(),
                Some("feishu:tenant_peer:group:oc_peer_1")
            );
            assert_eq!(
                event.base_conversation_id.as_deref(),
                Some("feishu:tenant_peer:group:oc_peer_1")
            );
            assert!(event.parent_conversation_candidates.is_empty());
            assert_eq!(event.conversation_scope.as_deref(), Some("peer"));
        }
        ParsedFeishuPayload::Challenge(_) => panic!("should parse event"),
    }
}

#[test]
fn parse_feishu_payload_assigns_topic_conversation_metadata() {
    let payload = serde_json::json!({
        "header": {
            "event_id": "evt_topic_1",
            "event_type": "im.message.receive_v1",
            "tenant_key": "tenant_topic"
        },
        "event": {
            "message": {
                "message_id": "om_topic_reply_1",
                "chat_id": "oc_topic_chat_1",
                "chat_type": "group",
                "root_id": "om_topic_root_1",
                "thread_id": "omt_topic_1",
                "content": "{\"text\":\"继续这个主题\"}"
            },
            "sender": {
                "sender_id": {
                    "open_id": "ou_sender_2"
                }
            }
        }
    });

    let parsed = parse_feishu_payload(&payload.to_string()).expect("payload should parse");
    match parsed {
        ParsedFeishuPayload::Event(event) => {
            assert_eq!(
                event.conversation_id.as_deref(),
                Some("feishu:tenant_topic:group:oc_topic_chat_1:topic:om_topic_root_1")
            );
            assert_eq!(
                event.base_conversation_id.as_deref(),
                Some("feishu:tenant_topic:group:oc_topic_chat_1")
            );
            assert_eq!(
                event.parent_conversation_candidates,
                vec!["feishu:tenant_topic:group:oc_topic_chat_1".to_string()]
            );
            assert_eq!(event.conversation_scope.as_deref(), Some("topic"));
        }
        ParsedFeishuPayload::Challenge(_) => panic!("should parse event"),
    }
}

#[test]
fn feishu_topic_contract_matches_normalized_bridge_contract() {
    let feishu_payload = serde_json::json!({
        "header": {
            "event_id": "evt_contract_1",
            "event_type": "im.message.receive_v1",
            "tenant_key": "tenant_contract"
        },
        "event": {
            "message": {
                "message_id": "om_contract_reply",
                "chat_id": "oc_contract_group",
                "chat_type": "group",
                "root_id": "om_contract_root",
                "content": "{\"text\":\"继续这个话题\"}"
            },
            "sender": {
                "sender_id": {
                    "open_id": "ou_contract_sender"
                }
            }
        }
    });

    let feishu_event =
        match parse_feishu_payload(&feishu_payload.to_string()).expect("parse feishu payload") {
            ParsedFeishuPayload::Event(event) => event,
            ParsedFeishuPayload::Challenge(_) => panic!("expected event"),
        };
    let normalized_event = parse_normalized_im_event_value(&serde_json::json!({
        "channel": "feishu",
        "workspace_id": "tenant_contract",
        "account_id": "tenant_contract",
        "thread_id": "oc_contract_group",
        "message_id": "om_contract_reply",
        "sender_id": "ou_contract_sender",
        "chat_type": "group",
        "root_id": "om_contract_root",
        "text": "继续这个话题"
    }))
    .expect("parse normalized event");

    assert_eq!(
        feishu_event.conversation_id,
        normalized_event.conversation_id
    );
    assert_eq!(
        feishu_event.base_conversation_id,
        normalized_event.base_conversation_id
    );
    assert_eq!(
        feishu_event.parent_conversation_candidates,
        normalized_event.parent_conversation_candidates
    );
    assert_eq!(
        feishu_event.conversation_scope,
        normalized_event.conversation_scope
    );
}

#[test]
fn feishu_blank_account_falls_back_to_tenant_like_normalized_bridge_contract() {
    let feishu_payload = serde_json::json!({
        "header": {
            "event_id": "evt_blank_account_1",
            "event_type": "im.message.receive_v1",
            "tenant_key": "   "
        },
        "event": {
            "message": {
                "message_id": "om_blank_account_1",
                "chat_id": "oc_blank_account_1",
                "chat_type": "group",
                "content": "{\"text\":\"继续这个边界情况\"}"
            },
            "sender": {
                "sender_id": {
                    "open_id": "ou_sender_blank_account"
                }
            }
        }
    });

    let feishu_event =
        match parse_feishu_payload(&feishu_payload.to_string()).expect("parse feishu payload") {
            ParsedFeishuPayload::Event(event) => event,
            ParsedFeishuPayload::Challenge(_) => panic!("expected event"),
        };
    let normalized_event = parse_normalized_im_event_value(&serde_json::json!({
        "channel": "feishu",
        "account_id": "   ",
        "tenant_id": "ou_sender_blank_account",
        "thread_id": "oc_blank_account_1",
        "message_id": "om_blank_account_1",
        "sender_id": "ou_sender_blank_account",
        "chat_type": "group",
        "text": "继续这个边界情况"
    }))
    .expect("parse normalized event");

    assert_eq!(
        feishu_event.conversation_id,
        normalized_event.conversation_id
    );
    assert_eq!(
        feishu_event.base_conversation_id,
        normalized_event.base_conversation_id
    );
    assert_eq!(
        feishu_event.parent_conversation_candidates,
        normalized_event.parent_conversation_candidates
    );
    assert_eq!(
        feishu_event.conversation_scope,
        normalized_event.conversation_scope
    );
}

#[test]
fn feishu_sparse_account_contract_matches_across_raw_normalized_and_direct_event_paths() {
    let raw_payload = serde_json::json!({
        "header": {
            "event_id": "evt_sparse_account_1",
            "event_type": "im.message.receive_v1"
        },
        "event": {
            "message": {
                "message_id": "om_sparse_account_1",
                "chat_id": "oc_sparse_account_1",
                "chat_type": "group",
                "content": "{\"text\":\"继续这个仅 sender 的情况\"}"
            },
            "sender": {
                "sender_id": {
                    "open_id": "ou_sparse_sender"
                }
            }
        }
    });
    let raw_event =
        match parse_feishu_payload(&raw_payload.to_string()).expect("parse raw feishu payload") {
            ParsedFeishuPayload::Event(event) => event,
            ParsedFeishuPayload::Challenge(_) => panic!("expected raw event"),
        };

    let normalized_event = parse_normalized_im_event_value(&serde_json::json!({
        "channel": "feishu",
        "account_id": "   ",
        "thread_id": "oc_sparse_account_1",
        "message_id": "om_sparse_account_1",
        "sender_id": "ou_sparse_sender",
        "chat_type": "group",
        "text": "继续这个仅 sender 的情况"
    }))
    .expect("parse normalized sparse-account event");

    let direct_payload = serde_json::json!({
        "channel": "feishu",
        "event_type": "message.created",
        "thread_id": "oc_sparse_account_1",
        "event_id": "evt_sparse_account_direct",
        "message_id": "om_sparse_account_1",
        "text": "继续这个仅 sender 的情况",
        "account_id": "   ",
        "sender_id": "ou_sparse_sender",
        "chat_type": "group"
    });
    let direct_event = match parse_feishu_payload(&direct_payload.to_string())
        .expect("parse direct fast-path event")
    {
        ParsedFeishuPayload::Event(event) => event,
        ParsedFeishuPayload::Challenge(_) => panic!("expected direct event"),
    };

    assert_eq!(raw_event.tenant_id.as_deref(), Some("ou_sparse_sender"));
    assert_eq!(
        normalized_event.tenant_id.as_deref(),
        Some("ou_sparse_sender")
    );
    assert_eq!(direct_event.tenant_id.as_deref(), Some("ou_sparse_sender"));

    assert_eq!(raw_event.conversation_id, normalized_event.conversation_id);
    assert_eq!(raw_event.conversation_id, direct_event.conversation_id);
    assert_eq!(
        raw_event.base_conversation_id,
        normalized_event.base_conversation_id
    );
    assert_eq!(
        raw_event.base_conversation_id,
        direct_event.base_conversation_id
    );
    assert_eq!(
        raw_event.parent_conversation_candidates,
        normalized_event.parent_conversation_candidates
    );
    assert_eq!(
        raw_event.parent_conversation_candidates,
        direct_event.parent_conversation_candidates
    );
    assert_eq!(
        raw_event.conversation_scope,
        normalized_event.conversation_scope
    );
    assert_eq!(
        raw_event.conversation_scope,
        direct_event.conversation_scope
    );
}
