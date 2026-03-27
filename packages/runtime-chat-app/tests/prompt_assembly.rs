use runtime_chat_app::{compose_system_prompt, ChatExecutionGuidance};

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
