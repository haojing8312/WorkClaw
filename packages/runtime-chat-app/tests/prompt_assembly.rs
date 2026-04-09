use runtime_chat_app::{
    build_system_prompt_sections, compose_system_prompt, compose_system_prompt_from_sections,
    compose_system_prompt_from_tool_names, ChatExecutionGuidance,
};

#[test]
fn compose_system_prompt_includes_execution_guidance_and_optional_sections() {
    let prompt = compose_system_prompt(
        "Base skill prompt",
        "bash, read, write, browser",
        "gpt-4.1",
        8,
        &ChatExecutionGuidance {
            effective_work_dir: "E:/workspace/demo".to_string(),
            local_timezone: "Asia/Shanghai".to_string(),
            local_date: "2026-03-20".to_string(),
            local_tomorrow: "2026-03-21".to_string(),
            local_month_range: "2026-03-01 ~ 2026-03-31".to_string(),
        },
        Some(
            "<available_skills>\n<skill><name>xhs</name><invoke_name>xhs</invoke_name><location>E:/workspace/demo/skills/xhs/SKILL.md</location></skill>\n</available_skills>",
        ),
        Some("Collaborate with employee-1 when domain knowledge is required."),
        Some("Remember previous delivery constraints."),
    );

    assert!(prompt.contains("Base skill prompt"));
    assert!(prompt.contains("工作目录: E:/workspace/demo"));
    assert!(prompt.contains("可用工具: bash, read, write, browser"));
    assert!(prompt.contains("模型: gpt-4.1"));
    assert!(prompt.contains("最大迭代次数: 8"));
    assert!(prompt.contains("Skills (mandatory):"));
    assert!(prompt.contains("<available_skills>"));
    assert!(prompt.contains("Scan the descriptions first."));
    assert!(prompt.contains("use its <invoke_name> or <location> as skill_name"));
    assert!(prompt.contains("Do not read multiple skills up front"));
    assert!(prompt.contains("do not read any skill when none clearly applies"));
    assert!(prompt.contains("E:/workspace/demo/skills/xhs/SKILL.md"));
    assert!(prompt.contains("WorkClaw 内置本地 browser sidecar"));
    assert!(prompt.contains("http://localhost:8765"));
    assert!(prompt.contains("不要要求用户手动启动 OpenClaw 浏览器服务"));
    assert!(prompt.contains("不要检查 openclaw-desktop.exe"));
    assert!(prompt.contains("不要要求固定安装目录"));
    assert!(prompt.contains("Collaborate with employee-1"));
    assert!(prompt.contains("持久内存:\nRemember previous delivery constraints."));
    assert!(prompt.contains("时间上下文:"));
    assert!(prompt.contains("本地时区: Asia/Shanghai"));
    assert!(prompt.contains("今天: 2026-03-20"));
    assert!(prompt.contains("明天: 2026-03-21"));
    assert!(prompt.contains("本月范围: 2026-03-01 ~ 2026-03-31"));
    assert!(prompt.contains("遇到“今天”“明天”“昨天”“本周”“这个月”"));
}

#[test]
fn compose_system_prompt_includes_file_tool_guidance_when_directory_tools_are_available() {
    let prompt = compose_system_prompt(
        "Base skill prompt",
        "list_dir, file_move, file_copy",
        "gpt-4.1",
        8,
        &ChatExecutionGuidance {
            effective_work_dir: "E:/workspace/demo".to_string(),
            local_timezone: "Asia/Shanghai".to_string(),
            local_date: "2026-03-20".to_string(),
            local_tomorrow: "2026-03-21".to_string(),
            local_month_range: "2026-03-01 ~ 2026-03-31".to_string(),
        },
        None,
        None,
        None,
    );

    assert!(prompt.contains("文件工具使用说明:"));
    assert!(prompt.contains("`list_dir` 会在可读列表后追加结构化 entries JSON"));
    assert!(prompt.contains("`file_move` / `file_copy` / `file_delete`"));
    assert!(prompt.contains("优先直接复用 entries 中的原始 `path`"));
    assert!(prompt.contains("不要手写或改写文件名"));
}

#[test]
fn compose_system_prompt_includes_structured_tool_result_guidance_for_core_tools() {
    let prompt = compose_system_prompt(
        "Base skill prompt",
        "read_file, write_file, edit, glob, grep, bash, bash_output, bash_kill, list_dir, file_copy, file_delete, file_move, file_stat",
        "gpt-4.1",
        8,
        &ChatExecutionGuidance {
            effective_work_dir: "E:/workspace/demo".to_string(),
            local_timezone: "Asia/Shanghai".to_string(),
            local_date: "2026-03-20".to_string(),
            local_tomorrow: "2026-03-21".to_string(),
            local_month_range: "2026-03-01 ~ 2026-03-31".to_string(),
        },
        None,
        None,
        None,
    );

    assert!(prompt.contains("结构化工具结果说明:"));
    assert!(prompt.contains("优先使用工具结果中的 `summary` 和 `details` 字段"));
    assert!(prompt.contains("不要从展示文本中二次猜测路径"));
    assert!(prompt.contains("命令执行结果优先读取 `exit_code`"));
    assert!(prompt.contains("文件类结果优先复用 `details` 中的精确路径或元信息"));
}

#[test]
fn compose_system_prompt_from_tool_names_matches_joined_tool_names() {
    let tool_names = vec![
        "bash".to_string(),
        "read".to_string(),
        "browser".to_string(),
    ];
    let guidance = ChatExecutionGuidance {
        effective_work_dir: "E:/workspace/demo".to_string(),
        local_timezone: "Asia/Shanghai".to_string(),
        local_date: "2026-03-20".to_string(),
        local_tomorrow: "2026-03-21".to_string(),
        local_month_range: "2026-03-01 ~ 2026-03-31".to_string(),
    };

    let prompt_from_list = compose_system_prompt_from_tool_names(
        &tool_names,
        "Base skill prompt",
        "gpt-4.1",
        8,
        &guidance,
        None,
        None,
        None,
    );
    let prompt_from_joined = compose_system_prompt(
        "Base skill prompt",
        "bash, read, browser",
        "gpt-4.1",
        8,
        &guidance,
        None,
        None,
        None,
    );

    assert_eq!(prompt_from_list, prompt_from_joined);
}

#[test]
fn build_system_prompt_sections_preserves_explicit_optional_sections() {
    let sections = build_system_prompt_sections(
        "Base skill prompt",
        "bash, read, browser",
        "gpt-4.1",
        8,
        &ChatExecutionGuidance {
            effective_work_dir: "E:/workspace/demo".to_string(),
            local_timezone: "Asia/Shanghai".to_string(),
            local_date: "2026-03-20".to_string(),
            local_tomorrow: "2026-03-21".to_string(),
            local_month_range: "2026-03-01 ~ 2026-03-31".to_string(),
        },
        Some("<available_skills />"),
        Some("Collaborate with employee-1"),
        Some("Remember previous delivery constraints."),
        &["当前未配置搜索引擎".to_string()],
    );

    assert_eq!(sections.base_prompt, "Base skill prompt");
    assert!(sections
        .capability_snapshot
        .contains("可用工具: bash, read, browser"));
    assert_eq!(
        sections.workspace_skills_prompt.as_deref(),
        Some("<available_skills />")
    );
    assert_eq!(
        sections.employee_collaboration_guidance.as_deref(),
        Some("Collaborate with employee-1")
    );
    assert_eq!(
        sections.memory_content.as_deref(),
        Some("Remember previous delivery constraints.")
    );
    assert_eq!(
        sections.runtime_notes,
        vec!["当前未配置搜索引擎".to_string()]
    );
    assert!(sections
        .temporal_execution_guidance
        .as_deref()
        .expect("temporal execution guidance")
        .contains("今天: 2026-03-20"));
}

#[test]
fn compose_system_prompt_from_sections_matches_legacy_builder() {
    let guidance = ChatExecutionGuidance {
        effective_work_dir: "E:/workspace/demo".to_string(),
        local_timezone: "Asia/Shanghai".to_string(),
        local_date: "2026-03-20".to_string(),
        local_tomorrow: "2026-03-21".to_string(),
        local_month_range: "2026-03-01 ~ 2026-03-31".to_string(),
    };

    let sections = build_system_prompt_sections(
        "Base skill prompt",
        "bash, read, browser",
        "gpt-4.1",
        8,
        &guidance,
        Some("<available_skills />"),
        Some("Collaborate with employee-1"),
        Some("Remember previous delivery constraints."),
        &[],
    );
    let prompt_from_sections = compose_system_prompt_from_sections(&sections);
    let legacy_prompt = compose_system_prompt(
        "Base skill prompt",
        "bash, read, browser",
        "gpt-4.1",
        8,
        &guidance,
        Some("<available_skills />"),
        Some("Collaborate with employee-1"),
        Some("Remember previous delivery constraints."),
    );

    assert_eq!(prompt_from_sections, legacy_prompt);
}
