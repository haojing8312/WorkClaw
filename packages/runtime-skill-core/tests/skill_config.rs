use runtime_skill_core::{SkillCommandArgMode, SkillConfig};

#[test]
fn parse_with_frontmatter() {
    let content = "---\nname: test-skill\ndescription: A test skill\nallowed_tools:\n  - read_file\n  - edit\n  - bash\ndenied_tools:\n  - bash\n  - file_delete\nallowed_tool_sources:\n  - native\n  - mcp\ndenied_tool_sources:\n  - plugin\nallowed_tool_categories:\n  - file\n  - browser\ndenied_tool_categories:\n  - shell\n  - browser\nmodel: gpt-4o\nmax_iterations: 5\n---\nYou are a helpful assistant.\n\nDo your best work.\n";
    let config = SkillConfig::parse(content);
    assert_eq!(config.name.as_deref(), Some("test-skill"));
    assert_eq!(config.description.as_deref(), Some("A test skill"));
    assert_eq!(
        config.allowed_tools,
        Some(vec!["read_file".into(), "edit".into(), "bash".into()])
    );
    assert_eq!(
        config.denied_tools,
        Some(vec!["bash".into(), "file_delete".into()])
    );
    assert_eq!(
        config.allowed_tool_sources,
        Some(vec!["native".into(), "mcp".into()])
    );
    assert_eq!(config.denied_tool_sources, Some(vec!["plugin".into()]));
    assert_eq!(
        config.allowed_tool_categories,
        Some(vec!["file".into(), "browser".into()])
    );
    assert_eq!(
        config.denied_tool_categories,
        Some(vec!["shell".into(), "browser".into()])
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
    assert_eq!(config.invocation.user_invocable, config.user_invocable);
    assert_eq!(
        config.invocation.disable_model_invocation,
        config.disable_model_invocation
    );
}

#[test]
fn parse_openclaw_metadata_and_command_dispatch_fields() {
    let content = r#"---
name: dispatched-skill
description: Deterministic tool-routed skill
user-invocable: true
disable-model-invocation: true
command-dispatch: tool
command-tool: exec
command-arg-mode: raw
metadata:
  {
    "openclaw":
      {
        "always": true,
        "emoji": "🌐",
        "skillKey": "pm-summary",
        "primaryEnv": "OPENAI_API_KEY",
        "os": ["windows", "linux"],
        "requires":
          {
            "bins": ["python"],
            "anyBins": ["py", "python3"],
            "env": ["OPENAI_API_KEY"],
            "config": ["skills.entries.pm-summary.apiKey"],
          },
      },
  }
---
Run the standard command.
"#;

    let config = SkillConfig::parse(content);
    assert!(config.user_invocable);
    assert!(config.disable_model_invocation);
    assert_eq!(
        config
            .command_dispatch
            .as_ref()
            .map(|spec| spec.tool_name.as_str()),
        Some("exec")
    );
    assert_eq!(
        config
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.primary_env.as_deref()),
        Some("OPENAI_API_KEY")
    );
    assert_eq!(
        config
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.skill_key.as_deref()),
        Some("pm-summary")
    );
    assert_eq!(
        config
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.requires.as_ref())
            .map(|requires| requires.bins.clone()),
        Some(vec!["python".to_string()])
    );
    assert_eq!(
        config
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.requires.as_ref())
            .map(|requires| requires.any_bins.clone()),
        Some(vec!["py".to_string(), "python3".to_string()])
    );
}

#[test]
fn parse_openclaw_string_boolean_invocation_fields() {
    let content = r#"---
name: string-bool-skill
description: OpenClaw-style string booleans
user-invocable: "no"
disable-model-invocation: "yes"
command-dispatch: tool
command-tool: exec
---
Run the command.
"#;

    let config = SkillConfig::parse(content);
    assert_eq!(config.name.as_deref(), Some("string-bool-skill"));
    assert!(!config.user_invocable);
    assert!(config.disable_model_invocation);
    assert_eq!(config.invocation.user_invocable, config.user_invocable);
    assert_eq!(
        config.invocation.disable_model_invocation,
        config.disable_model_invocation
    );
    assert_eq!(
        config
            .command_dispatch
            .as_ref()
            .map(|dispatch| dispatch.tool_name.as_str()),
        Some("exec")
    );
}

#[test]
fn parse_openclaw_numeric_boolean_invocation_fields() {
    let content = r#"---
name: numeric-bool-skill
user-invocable: 0
disable-model-invocation: 1
---
Run the command.
"#;

    let config = SkillConfig::parse(content);
    assert_eq!(config.name.as_deref(), Some("numeric-bool-skill"));
    assert!(!config.user_invocable);
    assert!(config.disable_model_invocation);
    assert_eq!(config.invocation.user_invocable, config.user_invocable);
    assert_eq!(
        config.invocation.disable_model_invocation,
        config.disable_model_invocation
    );
}

#[test]
fn parse_openclaw_yaml_metadata_install_specs() {
    let content = r#"---
name: installable-skill
description: Install-aware OpenClaw skill
user-invocable: false
disable-model-invocation: true
metadata:
  openclaw:
    always: true
    emoji: wrench
    homepage: https://example.com/skills/installable
    skillKey: installable
    primaryEnv: FEISHU_API_KEY
    os: windows, linux
    requires:
      bins: pwsh, powershell
      anyBins:
        - pwsh
        - powershell
      env:
        - FEISHU_API_KEY
      config: skills.entries.installable.apiKey
    install:
      - id: runtime-node
        kind: node
        package: feishu-pm-runtime@^1.0.0
        bins: feishu-pm-runtime
        label: Install the shared runtime via npm
      - id: runtime-brew
        kind: brew
        formula: feishu-pm-runtime
        bins: feishu-pm-runtime
        label: Install the shared runtime via Homebrew
      - id: runtime-download
        kind: download
        url: https://example.com/runtime.zip
        archive: zip
        extract: true
        stripComponents: 1
        targetDir: runtime
        os:
          - windows
---
Use the runtime.
"#;

    let config = SkillConfig::parse(content);
    let metadata = config.metadata.as_ref().expect("metadata should parse");
    assert_eq!(metadata.always, Some(true));
    assert_eq!(metadata.emoji.as_deref(), Some("wrench"));
    assert_eq!(
        metadata.homepage.as_deref(),
        Some("https://example.com/skills/installable")
    );
    assert_eq!(metadata.skill_key.as_deref(), Some("installable"));
    assert_eq!(metadata.primary_env.as_deref(), Some("FEISHU_API_KEY"));
    assert_eq!(
        metadata.os,
        vec!["windows".to_string(), "linux".to_string()]
    );
    assert_eq!(
        metadata
            .requires
            .as_ref()
            .map(|requires| requires.bins.clone()),
        Some(vec!["pwsh".to_string(), "powershell".to_string()])
    );
    assert_eq!(
        metadata
            .requires
            .as_ref()
            .map(|requires| requires.config.clone()),
        Some(vec!["skills.entries.installable.apiKey".to_string()])
    );
    let install = metadata.install.as_ref().expect("install should parse");
    assert_eq!(install.len(), 3);
    assert_eq!(install[0].id.as_deref(), Some("runtime-node"));
    assert_eq!(install[0].kind.as_str(), "node");
    assert_eq!(
        install[0].package.as_deref(),
        Some("feishu-pm-runtime@^1.0.0")
    );
    assert_eq!(install[0].bins, vec!["feishu-pm-runtime".to_string()]);
    assert_eq!(
        install[0].label.as_deref(),
        Some("Install the shared runtime via npm")
    );
    assert_eq!(install[1].id.as_deref(), Some("runtime-brew"));
    assert_eq!(install[1].kind.as_str(), "brew");
    assert_eq!(install[1].formula.as_deref(), Some("feishu-pm-runtime"));
    assert_eq!(install[1].bins, vec!["feishu-pm-runtime".to_string()]);
    assert_eq!(
        install[1].label.as_deref(),
        Some("Install the shared runtime via Homebrew")
    );
    assert_eq!(install[2].id.as_deref(), Some("runtime-download"));
    assert_eq!(install[2].kind.as_str(), "download");
    assert_eq!(
        install[2].url.as_deref(),
        Some("https://example.com/runtime.zip")
    );
    assert_eq!(install[2].archive.as_deref(), Some("zip"));
    assert_eq!(install[2].extract, Some(true));
    assert_eq!(install[2].strip_components, Some(1));
    assert_eq!(install[2].target_dir.as_deref(), Some("runtime"));
    assert_eq!(install[2].os, vec!["windows".to_string()]);
}

#[test]
fn parse_openclaw_command_dispatch_unknown_arg_mode_falls_back_to_raw() {
    let content = r#"---
name: fallback-dispatch
command-dispatch: tool
command-tool: exec
command-arg-mode: json
---
Run the command.
"#;

    let config = SkillConfig::parse(content);
    let dispatch = config
        .command_dispatch
        .as_ref()
        .expect("dispatch should still parse");
    assert_eq!(dispatch.tool_name, "exec");
    assert_eq!(dispatch.arg_mode, SkillCommandArgMode::Raw);
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

#[test]
fn parse_denied_tools_from_comma_separated_frontmatter() {
    let content = "---\nname: deny-skill\ndenied_tools: bash, file_delete, edit\n---\nStay safe.\n";

    let config = SkillConfig::parse(content);

    assert_eq!(
        config.denied_tools,
        Some(vec![
            "bash".to_string(),
            "file_delete".to_string(),
            "edit".to_string()
        ])
    );
}

#[test]
fn parse_denied_tool_categories_from_comma_separated_frontmatter() {
    let content =
        "---\nname: deny-category-skill\ndenied_tool_categories: shell, browser, integration\n---\nStay scoped.\n";

    let config = SkillConfig::parse(content);

    assert_eq!(
        config.denied_tool_categories,
        Some(vec![
            "shell".to_string(),
            "browser".to_string(),
            "integration".to_string()
        ])
    );
}

#[test]
fn parse_denied_tool_sources_from_comma_separated_frontmatter() {
    let content =
        "---\nname: deny-sources\ndenied_tool_sources: plugin, alias, runtime\n---\nPrompt";
    let config = SkillConfig::parse(content);

    assert_eq!(
        config.denied_tool_sources,
        Some(vec!["plugin".into(), "alias".into(), "runtime".into()])
    );
}

#[test]
fn parse_allowed_tool_categories_from_comma_separated_frontmatter() {
    let content =
        "---\nname: allow-categories\nallowed_tool_categories: file, browser, search\n---\nPrompt";
    let config = SkillConfig::parse(content);

    assert_eq!(
        config.allowed_tool_categories,
        Some(vec!["file".into(), "browser".into(), "search".into()])
    );
}

#[test]
fn parse_allowed_tool_sources_from_comma_separated_frontmatter() {
    let content =
        "---\nname: allow-source-skill\nallowed_tool_sources: native, runtime, mcp\n---\nStay scoped.\n";

    let config = SkillConfig::parse(content);

    assert_eq!(
        config.allowed_tool_sources,
        Some(vec![
            "native".to_string(),
            "runtime".to_string(),
            "mcp".to_string()
        ])
    );
}
