use runtime_skill_core::SkillConfig;

#[test]
fn parse_with_frontmatter() {
    let content = "---\nname: test-skill\ndescription: A test skill\nallowed_tools:\n  - read_file\n  - edit\n  - bash\nmodel: gpt-4o\nmax_iterations: 5\n---\nYou are a helpful assistant.\n\nDo your best work.\n";
    let config = SkillConfig::parse(content);
    assert_eq!(config.name.as_deref(), Some("test-skill"));
    assert_eq!(config.description.as_deref(), Some("A test skill"));
    assert_eq!(
        config.allowed_tools,
        Some(vec!["read_file".into(), "edit".into(), "bash".into()])
    );
    assert_eq!(config.model.as_deref(), Some("gpt-4o"));
    assert_eq!(config.max_iterations, Some(5));
    assert!(config
        .system_prompt
        .contains("You are a helpful assistant."));
    assert!(config.system_prompt.contains("Do your best work."));
}

#[test]
fn parse_claude_compatible_fields() {
    let content = "---\nname: claude-skill\ndescription: Claude compatible\nargument-hint: <file_path>\ndisable-model-invocation: true\nuser-invocable: false\ncontext: fork\nagent: Explore\n---\nDo something with $ARGUMENTS.";
    let config = SkillConfig::parse(content);
    assert_eq!(config.name.as_deref(), Some("claude-skill"));
    assert_eq!(config.argument_hint.as_deref(), Some("<file_path>"));
    assert!(config.disable_model_invocation);
    assert!(!config.user_invocable);
    assert_eq!(config.context.as_deref(), Some("fork"));
    assert_eq!(config.agent.as_deref(), Some("Explore"));
}

#[test]
fn substitute_arguments_replaces_supported_placeholders() {
    let mut config = SkillConfig {
        system_prompt: "All: $ARGUMENTS, First: $0, Session: ${CLAUDE_SESSION_ID}".to_string(),
        ..Default::default()
    };
    config.substitute_arguments(&["hello", "world"], "s1");
    assert_eq!(
        config.system_prompt,
        "All: hello world, First: hello, Session: s1"
    );
}
