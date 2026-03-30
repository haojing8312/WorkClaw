use runtime_lib::agent::tools::SkillInvokeTool;
use runtime_lib::agent::types::{Tool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

fn create_skill(root: &TempDir, name: &str, skill_md: &str) {
    let skill_dir = root.path().join(name);
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).expect("write SKILL.md");
}

#[test]
fn skill_tool_returns_narrowed_allowed_tools() {
    let tmp = TempDir::new().expect("temp dir");
    create_skill(
        &tmp,
        "child-skill",
        "---\nname: child-skill\nallowed_tools: \"ReadFile, web_search\"\n---\n\nChild prompt",
    );

    let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
    let ctx = ToolContext {
        work_dir: None,
        allowed_tools: Some(vec!["read_file".to_string(), "glob".to_string()]),
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };
    let out = tool
        .execute(json!({"skill_name": "child-skill"}), &ctx)
        .expect("skill invoke should succeed");

    assert!(out.contains("声明工具: ReadFile, web_search"));
    assert!(out.contains("收紧后工具: read_file"));
    assert!(out.contains("解析模式: prompt_following"));
}

#[test]
fn skill_tool_only_denies_explicit_dispatch_when_parent_scope_blocks_target_tool() {
    let tmp = TempDir::new().expect("temp dir");
    create_skill(
        &tmp,
        "child-skill",
        "---\nname: child-skill\ncommand-dispatch: tool\ncommand-tool: exec\n---\n\nChild prompt",
    );

    let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
    let ctx = ToolContext {
        work_dir: None,
        allowed_tools: Some(vec!["read_file".to_string()]),
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };
    let err = tool
        .execute(json!({"skill_name": "child-skill"}), &ctx)
        .expect_err("should be denied");

    assert!(
        err.to_string().contains("PERMISSION_DENIED"),
        "unexpected error: {}",
        err
    );
    assert!(!err.to_string().contains("Child prompt"));
}

#[test]
fn skill_tool_resolve_invocation_reports_dispatch_mode() {
    let tmp = TempDir::new().expect("temp dir");
    create_skill(
        &tmp,
        "child-skill",
        "---\nname: child-skill\ndisable-model-invocation: true\ncommand-dispatch: tool\ncommand-tool: exec\n---\n\nChild prompt",
    );

    let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
    let ctx = ToolContext {
        work_dir: None,
        allowed_tools: Some(vec!["exec".to_string(), "read_file".to_string()]),
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };
    let resolved = tool
        .resolve_invocation(json!({"skill_name": "child-skill"}), &ctx)
        .expect("dispatch skill should resolve");

    assert_eq!(resolved.mode.as_str(), "command_dispatch");
    assert_eq!(
        resolved
            .command_dispatch
            .as_ref()
            .map(|dispatch| dispatch.tool_name.as_str()),
        Some("exec")
    );
}

#[test]
fn skill_tool_keeps_prompt_skill_even_when_declared_tools_do_not_overlap_parent_scope() {
    let tmp = TempDir::new().expect("temp dir");
    create_skill(
        &tmp,
        "child-skill",
        "---\nname: child-skill\nallowed_tools: \"bash\"\n---\n\nChild prompt",
    );

    let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
    let ctx = ToolContext {
        work_dir: None,
        allowed_tools: Some(vec!["read_file".to_string()]),
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };
    let out = tool
        .execute(json!({"skill_name": "child-skill"}), &ctx)
        .expect("prompt skill should still resolve");

    assert!(out.contains("解析模式: prompt_following"));
    assert!(out.contains("收紧后工具: (无显式收紧结果)"));
}

#[test]
fn skill_tool_accepts_display_name_via_frontmatter_mapping() {
    let tmp = TempDir::new().expect("temp dir");
    create_skill(
        &tmp,
        "builtin-general",
        "---\nname: 通用助手\nallowed_tools: \"read_file\"\n---\n\nChild prompt",
    );

    let tool = SkillInvokeTool::new("sess-1".to_string(), vec![tmp.path().to_path_buf()]);
    let ctx = ToolContext {
        work_dir: None,
        allowed_tools: None,
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };
    let out = tool
        .execute(json!({"skill_name": "通用助手"}), &ctx)
        .expect("display name should map to directory skill");

    assert!(out.contains("## Skill: builtin-general"));
    assert!(out.contains("Child prompt"));
}

#[test]
fn skill_tool_accepts_skill_md_path_within_search_roots() {
    let actual_root = TempDir::new().expect("actual root");
    create_skill(
        &actual_root,
        "child-skill",
        "---\nname: child-skill\nallowed_tools: \"read_file\"\n---\n\nChild prompt",
    );

    let tool = SkillInvokeTool::new("sess-1".to_string(), vec![actual_root.path().to_path_buf()]);
    let ctx = ToolContext {
        work_dir: None,
        allowed_tools: None,
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };
    let skill_md_path = actual_root.path().join("child-skill").join("SKILL.md");
    let out = tool
        .execute(json!({"skill_name": skill_md_path.to_string_lossy()}), &ctx)
        .expect("skill path should be resolved when under allowed roots");

    assert!(out.contains("## Skill: child-skill"));
    assert!(out.contains("Child prompt"));
}

#[test]
fn skill_tool_rejects_skill_md_path_outside_search_roots() {
    let actual_root = TempDir::new().expect("actual root");
    create_skill(
        &actual_root,
        "child-skill",
        "---\nname: child-skill\nallowed_tools: \"read_file\"\n---\n\nChild prompt",
    );
    let isolated_root = TempDir::new().expect("isolated root");

    let tool = SkillInvokeTool::new(
        "sess-1".to_string(),
        vec![isolated_root.path().to_path_buf()],
    );
    let ctx = ToolContext {
        work_dir: None,
        allowed_tools: None,
        session_id: None,
        task_temp_dir: None,
        execution_caps: None,
        file_task_caps: None,
    };
    let skill_md_path = actual_root.path().join("child-skill").join("SKILL.md");
    let err = tool
        .execute(json!({"skill_name": skill_md_path.to_string_lossy()}), &ctx)
        .expect_err("skill path outside allowed roots should be rejected");

    assert!(
        err.to_string().contains("PERMISSION_DENIED"),
        "unexpected error: {}",
        err
    );
}
