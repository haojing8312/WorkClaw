use runtime_lib::agent::tools::browser_tools::{register_browser_tools, BROWSER_TOOL_NAMES};
use runtime_lib::agent::ToolRegistry;

#[test]
fn test_register_browser_tools_count() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    // 验证注册了 17 个浏览器工具
    let defs = registry.get_tool_definitions();
    assert_eq!(defs.len(), 17, "应注册 17 个浏览器工具");
}

#[test]
fn test_all_browser_tools_registered() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    // 验证每个工具都能通过名称获取
    for name in &BROWSER_TOOL_NAMES {
        assert!(
            registry.get(name).is_some(),
            "工具 {} 应已注册",
            name
        );
    }
}

#[test]
fn test_browser_navigate_schema_has_required_url() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    let tool = registry.get("browser_navigate").expect("browser_navigate 应已注册");
    let schema = tool.input_schema();

    // 验证 schema 包含 url 属性
    assert!(
        schema["properties"]["url"].is_object(),
        "browser_navigate schema 应包含 url 属性"
    );

    // 验证 url 是必填字段
    let required = schema["required"].as_array().expect("应有 required 数组");
    assert!(
        required.iter().any(|v| v.as_str() == Some("url")),
        "url 应在 required 列表中"
    );
}

#[test]
fn test_browser_type_schema_has_required_fields() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    let tool = registry.get("browser_type").expect("browser_type 应已注册");
    let schema = tool.input_schema();

    // 验证 selector 和 text 都是必填
    let required = schema["required"].as_array().expect("应有 required 数组");
    assert!(
        required.iter().any(|v| v.as_str() == Some("selector")),
        "selector 应在 required 列表中"
    );
    assert!(
        required.iter().any(|v| v.as_str() == Some("text")),
        "text 应在 required 列表中"
    );
}

#[test]
fn test_browser_scroll_schema_has_direction_enum() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    let tool = registry.get("browser_scroll").expect("browser_scroll 应已注册");
    let schema = tool.input_schema();

    // 验证 direction 有 enum 约束
    let direction = &schema["properties"]["direction"];
    let enum_values = direction["enum"].as_array().expect("direction 应有 enum");
    assert_eq!(enum_values.len(), 4, "direction 应有 4 个枚举值");

    // 验证 direction 是必填
    let required = schema["required"].as_array().expect("应有 required 数组");
    assert!(
        required.iter().any(|v| v.as_str() == Some("direction")),
        "direction 应在 required 列表中"
    );
}

#[test]
fn test_browser_tools_descriptions_are_chinese() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    // 验证所有工具描述都包含中文字符
    for name in &BROWSER_TOOL_NAMES {
        let tool = registry.get(name).unwrap();
        let desc = tool.description();
        assert!(
            desc.chars().any(|c| c >= '\u{4e00}' && c <= '\u{9fff}'),
            "工具 {} 的描述应包含中文: {}",
            name,
            desc
        );
    }
}

#[test]
fn test_browser_tools_not_in_file_tools() {
    // 验证浏览器工具不会出现在 with_file_tools() 注册表中
    let registry = ToolRegistry::with_file_tools();

    for name in &BROWSER_TOOL_NAMES {
        assert!(
            registry.get(name).is_none(),
            "工具 {} 不应出现在 with_file_tools() 中",
            name
        );
    }
}

#[test]
fn test_browser_tools_with_prefix() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    // 使用 tools_with_prefix 查找所有浏览器工具
    let browser_tools = registry.tools_with_prefix("browser_");
    assert_eq!(
        browser_tools.len(),
        17,
        "tools_with_prefix(\"browser_\") 应返回 17 个工具"
    );
}

#[test]
fn test_browser_evaluate_schema_requires_script() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    let tool = registry.get("browser_evaluate").expect("browser_evaluate 应已注册");
    let schema = tool.input_schema();

    let required = schema["required"].as_array().expect("应有 required 数组");
    assert!(
        required.iter().any(|v| v.as_str() == Some("script")),
        "script 应在 required 列表中"
    );
}

#[test]
fn test_browser_tools_no_required_for_optional_tools() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    // 这些工具没有必填参数
    let optional_tools = [
        "browser_launch",
        "browser_screenshot",
        "browser_get_dom",
        "browser_wait_for",
        "browser_go_back",
        "browser_go_forward",
        "browser_reload",
        "browser_get_state",
        "browser_snapshot",
    ];

    for name in &optional_tools {
        let tool = registry.get(name).expect(&format!("{} 应已注册", name));
        let schema = tool.input_schema();
        // 这些工具的 schema 不应有 required 字段，或 required 为空
        if let Some(required) = schema.get("required") {
            if let Some(arr) = required.as_array() {
                assert!(
                    arr.is_empty(),
                    "工具 {} 不应有必填参数",
                    name
                );
            }
        }
        // 如果没有 required 字段也是正确的
    }
}

#[test]
fn test_browser_launch_schema_is_local_playwright_only() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    let tool = registry.get("browser_launch").expect("browser_launch 应已注册");
    let schema = tool.input_schema();
    assert!(schema["properties"]["headless"].is_object(), "应包含 headless");
    assert!(schema["properties"]["viewport"].is_object(), "应包含 viewport");
    assert!(
        schema["properties"].get("provider").is_none(),
        "browser_launch schema 不应包含 provider"
    );
}

#[test]
fn test_browser_act_schema_requires_kind() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    let tool = registry.get("browser_act").expect("browser_act 应已注册");
    let schema = tool.input_schema();

    let required = schema["required"].as_array().expect("应有 required 数组");
    assert!(
        required.iter().any(|v| v.as_str() == Some("kind")),
        "kind 应在 required 列表中"
    );
    assert!(
        schema["properties"]["kind"]["enum"].is_array(),
        "kind 应有 enum 约束"
    );
}

#[test]
fn test_browser_act_schema_covers_extended_local_actions() {
    let registry = ToolRegistry::new();
    register_browser_tools(&registry, "http://localhost:8765");

    let tool = registry.get("browser_act").expect("browser_act 应已注册");
    let schema = tool.input_schema();
    let props = &schema["properties"];

    for field in [
        "startRef",
        "endRef",
        "startSelector",
        "endSelector",
        "fields",
        "width",
        "height",
        "timeMs",
        "textGone",
    ] {
        assert!(
            props[field].is_object(),
            "browser_act schema 应包含扩展字段: {}",
            field
        );
    }
}
