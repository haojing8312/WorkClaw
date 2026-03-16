use crate::agent::registry::ToolRegistry;
use crate::agent::tools::SidecarBridgeTool;
use serde_json::json;
use std::sync::Arc;

pub fn register_browser_compat_tool(registry: &ToolRegistry, sidecar_url: &str) {
    registry.register(Arc::new(SidecarBridgeTool::new(
        sidecar_url.to_string(),
        "/api/browser/compat".to_string(),
        "browser".to_string(),
        "OpenClaw 兼容浏览器工具。通过 action/profile/targetId 等参数驱动浏览器动作。"
            .to_string(),
        browser_compat_schema(),
    )));
}

fn browser_compat_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["status", "start", "stop", "profiles", "tabs", "open", "focus", "snapshot", "act", "upload"]
            },
            "profile": {
                "type": "string",
                "description": "浏览器 profile。P0 仅保证 openclaw 可用。"
            },
            "targetId": {
                "type": "string",
                "description": "目标标签页 ID。"
            },
            "url": {
                "type": "string",
                "description": "用于 open/navigate 等动作的 URL。"
            },
            "ref": {
                "type": "string",
                "description": "快照引用。"
            },
            "inputRef": {
                "type": "string",
                "description": "上传动作对应的输入框 ref。"
            },
            "paths": {
                "type": "array",
                "items": { "type": "string" },
                "description": "上传文件路径。"
            },
            "kind": {
                "type": "string",
                "description": "act 动作类型。"
            },
            "text": {
                "type": "string",
                "description": "输入文本。"
            },
            "key": {
                "type": "string",
                "description": "按键名。"
            },
            "fields": {
                "type": "array",
                "items": {
                    "type": "object"
                },
                "description": "批量填充字段。"
            }
        },
        "required": ["action"]
    })
}
