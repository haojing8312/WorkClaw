use crate::agent::registry::ToolRegistry;
use crate::agent::tools::SidecarBridgeTool;
use serde_json::json;
use std::sync::Arc;

/// 注册 17 个浏览器工具到 ToolRegistry
///
/// 所有浏览器工具通过 SidecarBridgeTool 桥接到 Node.js Sidecar 的 Playwright 端点。
/// 这些工具不在 `with_file_tools()` 中注册，而是在 chat.rs 中动态注册，
/// 仅当 Sidecar 已启动时才可用。
pub fn register_browser_tools(registry: &ToolRegistry, sidecar_url: &str) {
    let url = sidecar_url.to_string();

    // 1. browser_launch - 启动浏览器
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/launch".to_string(),
        "browser_launch".to_string(),
        "启动浏览器实例。可选择无头模式或指定视口大小。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "headless": {
                    "type": "boolean",
                    "description": "是否以无头模式启动，默认 true"
                },
                "viewport": {
                    "type": "object",
                    "description": "视口大小",
                    "properties": {
                        "width": { "type": "integer", "description": "视口宽度（像素）" },
                        "height": { "type": "integer", "description": "视口高度（像素）" }
                    }
                }
            }
        }),
    )));

    // 2. browser_navigate - 导航到指定 URL
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/navigate".to_string(),
        "browser_navigate".to_string(),
        "导航浏览器到指定 URL 地址。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "目标 URL 地址"
                }
            },
            "required": ["url"]
        }),
    )));

    // 3. browser_click - 点击页面元素
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/click".to_string(),
        "browser_click".to_string(),
        "点击页面元素。可通过 CSS 选择器或坐标定位。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "string",
                    "description": "CSS 选择器"
                },
                "x": {
                    "type": "number",
                    "description": "点击的 X 坐标"
                },
                "y": {
                    "type": "number",
                    "description": "点击的 Y 坐标"
                }
            }
        }),
    )));

    // 4. browser_type - 输入文本
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/type".to_string(),
        "browser_type".to_string(),
        "在指定元素中输入文本。支持设置输入延迟以模拟真实打字。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "string",
                    "description": "目标输入框的 CSS 选择器"
                },
                "text": {
                    "type": "string",
                    "description": "要输入的文本内容"
                },
                "delay": {
                    "type": "integer",
                    "description": "每个字符之间的延迟（毫秒）"
                }
            },
            "required": ["selector", "text"]
        }),
    )));

    // 5. browser_scroll - 滚动页面
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/scroll".to_string(),
        "browser_scroll".to_string(),
        "滚动页面。支持上下滚动和滚动到顶部/底部。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "direction": {
                    "type": "string",
                    "enum": ["up", "down", "to_top", "to_bottom"],
                    "description": "滚动方向：up（向上）、down（向下）、to_top（到顶部）、to_bottom（到底部）"
                },
                "amount": {
                    "type": "integer",
                    "description": "滚动距离（像素），仅对 up/down 有效"
                }
            },
            "required": ["direction"]
        }),
    )));

    // 6. browser_hover - 悬停在元素上
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/hover".to_string(),
        "browser_hover".to_string(),
        "将鼠标悬停在指定元素上。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "string",
                    "description": "目标元素的 CSS 选择器"
                }
            },
            "required": ["selector"]
        }),
    )));

    // 7. browser_press_key - 按下键盘按键
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/press_key".to_string(),
        "browser_press_key".to_string(),
        "模拟键盘按键。支持组合键（如 Ctrl+C）。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "按键名称，如 Enter、Tab、Escape、ArrowDown 等"
                },
                "modifiers": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "修饰键列表，如 [\"Control\", \"Shift\"]"
                }
            },
            "required": ["key"]
        }),
    )));

    // 8. browser_screenshot - 截取页面截图
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/screenshot".to_string(),
        "browser_screenshot".to_string(),
        "截取当前页面或指定元素的截图。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "截图保存路径"
                },
                "selector": {
                    "type": "string",
                    "description": "仅截取指定元素的 CSS 选择器"
                },
                "full_page": {
                    "type": "boolean",
                    "description": "是否截取整个页面（包括滚动区域）"
                }
            }
        }),
    )));

    // 9. browser_get_dom - 获取 DOM 结构
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/get_dom".to_string(),
        "browser_get_dom".to_string(),
        "获取当前页面的 DOM 结构。可指定选择器和最大深度。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "string",
                    "description": "起始元素的 CSS 选择器，默认为 body"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "DOM 树的最大遍历深度"
                }
            }
        }),
    )));

    // 10. browser_evaluate - 执行 JavaScript 脚本
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/evaluate".to_string(),
        "browser_evaluate".to_string(),
        "在浏览器中执行 JavaScript 脚本并返回结果。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "script": {
                    "type": "string",
                    "description": "要执行的 JavaScript 代码"
                }
            },
            "required": ["script"]
        }),
    )));

    // 11. browser_wait_for - 等待条件满足
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/wait_for".to_string(),
        "browser_wait_for".to_string(),
        "等待页面元素出现或条件满足。可设置超时时间。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "string",
                    "description": "等待出现的元素的 CSS 选择器"
                },
                "condition": {
                    "type": "string",
                    "description": "等待条件，如 visible、hidden、attached"
                },
                "timeout": {
                    "type": "integer",
                    "description": "超时时间（毫秒），默认 30000"
                }
            }
        }),
    )));

    // 12. browser_go_back - 浏览器后退
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/go_back".to_string(),
        "browser_go_back".to_string(),
        "浏览器后退到上一页。".to_string(),
        json!({
            "type": "object",
            "properties": {}
        }),
    )));

    // 13. browser_go_forward - 浏览器前进
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/go_forward".to_string(),
        "browser_go_forward".to_string(),
        "浏览器前进到下一页。".to_string(),
        json!({
            "type": "object",
            "properties": {}
        }),
    )));

    // 14. browser_reload - 刷新页面
    registry.register(Arc::new(SidecarBridgeTool::new(
        url.clone(),
        "/api/browser/reload".to_string(),
        "browser_reload".to_string(),
        "刷新当前页面。".to_string(),
        json!({
            "type": "object",
            "properties": {}
        }),
    )));

    // 15. browser_get_state - 获取浏览器状态
    registry.register(Arc::new(SidecarBridgeTool::new(
        url,
        "/api/browser/get_state".to_string(),
        "browser_get_state".to_string(),
        "获取浏览器当前状态，包括 URL、标题、是否加载完成等信息。".to_string(),
        json!({
            "type": "object",
            "properties": {}
        }),
    )));

    // 16. browser_snapshot - 获取页面快照
    registry.register(Arc::new(SidecarBridgeTool::new(
        sidecar_url.to_string(),
        "/api/browser/snapshot".to_string(),
        "browser_snapshot".to_string(),
        "获取页面快照。支持 ai/aria 格式，返回 ref 映射用于后续 browser_act。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["ai", "aria"],
                    "description": "快照格式，ai 或 aria"
                },
                "targetId": { "type": "string", "description": "目标标签页 ID" },
                "limit": { "type": "integer", "description": "快照节点数量限制" },
                "maxChars": { "type": "integer", "description": "快照文本最大长度" },
                "mode": { "type": "string", "description": "快照模式（如 efficient）" },
                "refs": { "type": "string", "enum": ["role", "aria"], "description": "ref 生成模式" },
                "interactive": { "type": "boolean", "description": "是否交互式快照" },
                "compact": { "type": "boolean", "description": "是否压缩快照输出" },
                "depth": { "type": "integer", "description": "快照深度" },
                "selector": { "type": "string", "description": "限制快照的 CSS 选择器" },
                "frame": { "type": "string", "description": "限制快照的 frame 选择器" },
                "labels": { "type": "boolean", "description": "是否返回标签信息" }
            }
        }),
    )));

    // 17. browser_act - 执行动作
    registry.register(Arc::new(SidecarBridgeTool::new(
        sidecar_url.to_string(),
        "/api/browser/act".to_string(),
        "browser_act".to_string(),
        "执行浏览器动作。支持 click/type/press/hover/select/wait/evaluate/close 等。".to_string(),
        json!({
            "type": "object",
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["click", "type", "press", "hover", "drag", "select", "fill", "resize", "wait", "evaluate", "close"],
                    "description": "动作类型"
                },
                "targetId": { "type": "string", "description": "目标标签页 ID" },
                "ref": { "type": "string", "description": "快照引用或选择器（回退模式）" },
                "selector": { "type": "string", "description": "CSS 选择器（回退模式）" },
                "startRef": { "type": "string", "description": "拖拽起点 ref（drag）" },
                "endRef": { "type": "string", "description": "拖拽终点 ref（drag）" },
                "startSelector": { "type": "string", "description": "拖拽起点选择器（drag）" },
                "endSelector": { "type": "string", "description": "拖拽终点选择器（drag）" },
                "fields": {
                    "type": "array",
                    "description": "批量填充字段（fill）",
                    "items": {
                        "type": "object",
                        "properties": {
                            "selector": { "type": "string" },
                            "ref": { "type": "string" },
                            "text": { "type": "string" }
                        }
                    }
                },
                "text": { "type": "string", "description": "输入文本（type）" },
                "key": { "type": "string", "description": "按键（press）" },
                "values": { "type": "array", "items": { "type": "string" }, "description": "选择值（select）" },
                "width": { "type": "integer", "description": "视口宽度（resize）" },
                "height": { "type": "integer", "description": "视口高度（resize）" },
                "timeMs": { "type": "integer", "description": "等待时长（wait）" },
                "timeoutMs": { "type": "integer", "description": "等待时长（wait）" },
                "textGone": { "type": "string", "description": "等待文本消失（wait）" },
                "fn": { "type": "string", "description": "执行脚本（evaluate）" },
                "submit": { "type": "boolean", "description": "输入后是否回车提交（type）" },
                "slowly": { "type": "boolean", "description": "是否慢速输入（type）" }
            },
            "required": ["kind"]
        }),
    )));
}

/// 所有浏览器工具名称列表，用于白名单或批量操作
pub const BROWSER_TOOL_NAMES: [&str; 17] = [
    "browser_launch",
    "browser_navigate",
    "browser_click",
    "browser_type",
    "browser_scroll",
    "browser_hover",
    "browser_press_key",
    "browser_screenshot",
    "browser_get_dom",
    "browser_evaluate",
    "browser_wait_for",
    "browser_go_back",
    "browser_go_forward",
    "browser_reload",
    "browser_get_state",
    "browser_snapshot",
    "browser_act",
];
