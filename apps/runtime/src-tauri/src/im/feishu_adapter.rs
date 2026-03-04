use serde_json::{json, Value};

pub fn build_feishu_text_message(chat_id: &str, text: &str) -> Value {
    json!({
        "receive_id": chat_id,
        "msg_type": "text",
        "content": serde_json::to_string(&json!({ "text": text })).unwrap_or_else(|_| "{\"text\":\"\"}".to_string())
    })
}

pub fn build_feishu_markdown_message(chat_id: &str, markdown: &str) -> Value {
    json!({
        "receive_id": chat_id,
        "msg_type": "post",
        "content": serde_json::to_string(&json!({
            "zh_cn": {
                "title": "智能体协作更新",
                "content": [[{
                    "tag": "text",
                    "text": markdown
                }]]
            }
        }))
        .unwrap_or_else(|_| "{\"zh_cn\":{\"title\":\"智能体协作更新\",\"content\":[[{\"tag\":\"text\",\"text\":\"\"}]]}}".to_string())
    })
}
