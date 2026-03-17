use runtime_lib::agent::tools::browser_compat::register_browser_compat_tool;
use runtime_lib::agent::ToolRegistry;

#[test]
fn test_register_browser_compat_tool() {
    let registry = ToolRegistry::new();
    register_browser_compat_tool(&registry, "http://localhost:8765");

    let tool = registry
        .get("browser")
        .expect("browser should be registered");
    let schema = tool.input_schema();

    assert!(schema["properties"]["action"].is_object());
    assert!(schema["properties"]["profile"].is_object());
    assert!(schema["properties"]["targetId"].is_object());

    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("browser action enum should exist");
    assert!(actions.iter().any(|value| value.as_str() == Some("stop")));
    assert!(!actions.iter().any(|value| value.as_str() == Some("dialog")));
    assert!(!actions
        .iter()
        .any(|value| value.as_str() == Some("console")));
    assert!(!actions.iter().any(|value| value.as_str() == Some("pdf")));
}
