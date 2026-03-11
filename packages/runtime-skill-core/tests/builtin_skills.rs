use runtime_skill_core::{
    builtin_skill_entries, builtin_skill_markdown, local_skill_template_markdown, SkillConfig,
    BUILTIN_DOCX_SKILL_ID, BUILTIN_EMPLOYEE_CREATOR_ID, BUILTIN_FIND_SKILLS_ID,
    BUILTIN_PDF_SKILL_ID, BUILTIN_PPTX_SKILL_ID, BUILTIN_SKILL_CREATOR_ID, BUILTIN_XLSX_SKILL_ID,
};

#[test]
fn builtin_skill_entries_include_expert_presets() {
    let ids: Vec<&str> = builtin_skill_entries()
        .iter()
        .map(|entry| entry.id)
        .collect();
    assert!(ids.contains(&BUILTIN_SKILL_CREATOR_ID));
    assert!(ids.contains(&BUILTIN_DOCX_SKILL_ID));
    assert!(ids.contains(&BUILTIN_PDF_SKILL_ID));
    assert!(ids.contains(&BUILTIN_PPTX_SKILL_ID));
    assert!(ids.contains(&BUILTIN_XLSX_SKILL_ID));
    assert!(ids.contains(&BUILTIN_FIND_SKILLS_ID));
    assert!(ids.contains(&BUILTIN_EMPLOYEE_CREATOR_ID));
}

#[test]
fn builtin_find_skills_declares_expected_tools() {
    let markdown =
        builtin_skill_markdown(BUILTIN_FIND_SKILLS_ID).expect("find-skills markdown exists");
    let config = SkillConfig::parse(markdown);
    let allowed_tools = config.allowed_tools.unwrap_or_default();

    assert!(allowed_tools.iter().any(|tool| tool == "ask_user"));
    assert!(allowed_tools
        .iter()
        .any(|tool| tool == "github_repo_download"));
}

#[test]
fn local_template_contains_prompt_sections() {
    let markdown = local_skill_template_markdown();
    assert!(markdown.contains("## When to Use"));
    assert!(markdown.contains("## Prompt Examples"));
}
