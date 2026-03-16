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
    };
    let out = tool
        .execute(json!({"skill_name": "child-skill"}), &ctx)
        .expect("skill invoke should succeed");

    assert!(out.contains("声明工具: ReadFile, web_search"));
    assert!(out.contains("收紧后工具: read_file"));
}

#[test]
fn skill_tool_denies_when_child_tools_outside_parent_scope() {
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
    };
    let err = tool
        .execute(json!({"skill_name": "child-skill"}), &ctx)
        .expect_err("should be denied");

    assert!(
        err.to_string().contains("PERMISSION_DENIED"),
        "unexpected error: {}",
        err
    );
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
