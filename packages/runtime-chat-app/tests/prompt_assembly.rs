use runtime_chat_app::{compose_system_prompt, ChatExecutionGuidance};

#[test]
fn compose_system_prompt_includes_execution_guidance_and_optional_sections() {
    let prompt = compose_system_prompt(
        "Base skill prompt",
        "bash, read, write",
        "gpt-4.1",
        8,
        &ChatExecutionGuidance {
            effective_work_dir: "E:/workspace/demo".to_string(),
            imported_mcp_guidance: Some("Prefer MCP for external content".to_string()),
        },
        Some("Collaborate with employee-1 when domain knowledge is required."),
        Some("Imported MCP server is available."),
        Some("Remember previous delivery constraints."),
    );

    assert!(prompt.contains("Base skill prompt"));
    assert!(prompt.contains("工作目录: E:/workspace/demo"));
    assert!(prompt.contains("可用工具: bash, read, write"));
    assert!(prompt.contains("模型: gpt-4.1"));
    assert!(prompt.contains("最大迭代次数: 8"));
    assert!(prompt.contains("Collaborate with employee-1"));
    assert!(prompt.contains("Imported MCP server is available."));
    assert!(prompt.contains("持久内存:\nRemember previous delivery constraints."));
}
