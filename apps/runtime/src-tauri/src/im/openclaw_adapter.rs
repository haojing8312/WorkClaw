use serde_json::{json, Value};

pub fn build_openclaw_outbound_message(
    thread_id: &str,
    role_id: &str,
    role_name: &str,
    content: &str,
) -> Value {
    json!({
        "thread_id": thread_id,
        "sender": {
            "type": "role",
            "id": role_id,
            "name": role_name
        },
        "message": {
            "content_type": "text/markdown",
            "content": content
        }
    })
}
