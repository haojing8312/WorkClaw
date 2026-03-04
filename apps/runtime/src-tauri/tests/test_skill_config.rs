use runtime_lib::agent::skill_config::SkillConfig;

#[test]
fn test_parse_with_frontmatter() {
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
fn test_parse_without_frontmatter() {
    let content = "You are a helpful assistant.\n\nDo stuff.";
    let config = SkillConfig::parse(content);
    assert!(config.name.is_none());
    assert!(config.allowed_tools.is_none());
    assert_eq!(config.system_prompt, content);
}

#[test]
fn test_parse_empty_frontmatter() {
    let content = "---\n---\nJust a prompt.";
    let config = SkillConfig::parse(content);
    assert!(config.name.is_none());
    assert_eq!(config.system_prompt.trim(), "Just a prompt.");
}

#[test]
fn test_parse_empty_content() {
    let config = SkillConfig::parse("");
    assert!(config.name.is_none());
    assert_eq!(config.system_prompt, "");
}

#[test]
fn test_parse_partial_frontmatter() {
    let content = "---\nname: partial\n---\nPrompt here.";
    let config = SkillConfig::parse(content);
    assert_eq!(config.name.as_deref(), Some("partial"));
    assert!(config.allowed_tools.is_none());
    assert!(config.model.is_none());
    assert_eq!(config.system_prompt.trim(), "Prompt here.");
}

#[test]
fn test_parse_no_closing_frontmatter() {
    let content = "---\nname: broken\nno closing marker";
    let config = SkillConfig::parse(content);
    // 没有结束标记，整个内容作为 prompt
    assert!(config.name.is_none());
    assert_eq!(config.system_prompt, content);
}

// === Claude Code 兼容字段测试 ===

#[test]
fn test_parse_claude_code_fields() {
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
fn test_default_claude_code_fields() {
    // 不指定 Claude Code 字段时的默认值
    let content = "---\nname: simple\n---\nHello.";
    let config = SkillConfig::parse(content);
    assert!(config.argument_hint.is_none());
    assert!(!config.disable_model_invocation); // 默认 false
    assert!(config.user_invocable); // 默认 true
    assert!(config.context.is_none());
    assert!(config.agent.is_none());
}

#[test]
fn test_allowed_tools_comma_separated() {
    let content =
        "---\nname: csv-tools\nallowed_tools: \"Bash, Read, Glob\"\n---\nUse these tools.";
    let config = SkillConfig::parse(content);
    assert_eq!(
        config.allowed_tools,
        Some(vec!["Bash".into(), "Read".into(), "Glob".into()])
    );
}

#[test]
fn test_allowed_tools_array_format() {
    let content = "---\nname: arr-tools\nallowed_tools:\n  - Bash\n  - Read\n---\nUse tools.";
    let config = SkillConfig::parse(content);
    assert_eq!(
        config.allowed_tools,
        Some(vec!["Bash".into(), "Read".into()])
    );
}

#[test]
fn test_allowed_tools_inline_array() {
    let content = "---\nallowed_tools: [\"Bash\", \"Write\"]\n---\nPrompt.";
    let config = SkillConfig::parse(content);
    assert_eq!(
        config.allowed_tools,
        Some(vec!["Bash".into(), "Write".into()])
    );
}

// === substitute_arguments 测试 ===

#[test]
fn test_substitute_arguments_all() {
    let mut config = SkillConfig {
        system_prompt: "Process $ARGUMENTS now.".to_string(),
        ..Default::default()
    };
    config.substitute_arguments(&["file.txt", "output.md"], "sess-123");
    assert_eq!(config.system_prompt, "Process file.txt output.md now.");
}

#[test]
fn test_substitute_arguments_indexed() {
    let mut config = SkillConfig {
        system_prompt: "Read $ARGUMENTS[0] and write to $ARGUMENTS[1].".to_string(),
        ..Default::default()
    };
    config.substitute_arguments(&["input.rs", "output.rs"], "sess-456");
    assert_eq!(
        config.system_prompt,
        "Read input.rs and write to output.rs."
    );
}

#[test]
fn test_substitute_arguments_shorthand() {
    let mut config = SkillConfig {
        system_prompt: "File: $0, Dest: $1".to_string(),
        ..Default::default()
    };
    config.substitute_arguments(&["src.rs", "dst.rs"], "sess-789");
    assert_eq!(config.system_prompt, "File: src.rs, Dest: dst.rs");
}

#[test]
fn test_substitute_session_id() {
    let mut config = SkillConfig {
        system_prompt: "Session: ${CLAUDE_SESSION_ID}".to_string(),
        ..Default::default()
    };
    config.substitute_arguments(&[], "my-session-id");
    assert_eq!(config.system_prompt, "Session: my-session-id");
}

#[test]
fn test_substitute_mixed_placeholders() {
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

#[test]
fn test_substitute_no_args() {
    let mut config = SkillConfig {
        system_prompt: "No args: $ARGUMENTS end.".to_string(),
        ..Default::default()
    };
    config.substitute_arguments(&[], "s2");
    assert_eq!(config.system_prompt, "No args:  end.");
}

#[test]
fn test_yaml_alias_underscore_forms() {
    // 同时支持下划线和连字符形式
    let content = "---\nname: alias-test\nargument_hint: <path>\ndisable_model_invocation: true\nuser_invocable: false\n---\nPrompt.";
    let config = SkillConfig::parse(content);
    // serde alias 只允许 alias 作为替代名，原始字段名用下划线
    // 但我们的 struct 字段名就是下划线形式，alias 是连字符形式
    // 所以 YAML 中写下划线也应该能解析
    assert_eq!(config.argument_hint.as_deref(), Some("<path>"));
    assert!(config.disable_model_invocation);
    assert!(!config.user_invocable);
}
