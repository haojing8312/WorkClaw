use crate::types::{ChatEmployeeSnapshot, ChatExecutionGuidance};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemPromptSections {
    pub base_prompt: String,
    pub capability_snapshot: String,
    pub workspace_skills_prompt: Option<String>,
    pub employee_collaboration_guidance: Option<String>,
    pub memory_content: Option<String>,
    pub temporal_execution_guidance: Option<String>,
    pub runtime_notes: Vec<String>,
    pub tool_runtime_notes: Vec<String>,
}

pub fn compose_system_prompt_from_tool_names(
    tool_names: &[String],
    base_prompt: &str,
    model_name: &str,
    max_iter: usize,
    guidance: &ChatExecutionGuidance,
    workspace_skills_prompt: Option<&str>,
    employee_collaboration_guidance: Option<&str>,
    memory_content: Option<&str>,
) -> String {
    let sections = build_system_prompt_sections(
        base_prompt,
        &tool_names.join(", "),
        model_name,
        max_iter,
        guidance,
        workspace_skills_prompt,
        employee_collaboration_guidance,
        memory_content,
        &[],
    );
    compose_system_prompt_from_sections(&sections)
}

pub fn compose_system_prompt(
    base_prompt: &str,
    tool_names: &str,
    model_name: &str,
    max_iter: usize,
    guidance: &ChatExecutionGuidance,
    workspace_skills_prompt: Option<&str>,
    employee_collaboration_guidance: Option<&str>,
    memory_content: Option<&str>,
) -> String {
    let sections = build_system_prompt_sections(
        base_prompt,
        tool_names,
        model_name,
        max_iter,
        guidance,
        workspace_skills_prompt,
        employee_collaboration_guidance,
        memory_content,
        &[],
    );
    compose_system_prompt_from_sections(&sections)
}

pub fn build_system_prompt_sections(
    base_prompt: &str,
    tool_names: &str,
    model_name: &str,
    max_iter: usize,
    guidance: &ChatExecutionGuidance,
    workspace_skills_prompt: Option<&str>,
    employee_collaboration_guidance: Option<&str>,
    memory_content: Option<&str>,
    runtime_notes: &[String],
) -> SystemPromptSections {
    let capability_snapshot = if guidance.effective_work_dir.trim().is_empty() {
        format!(
            "运行环境:\n- 可用工具: {}\n- 模型: {}\n- 最大迭代次数: {}",
            tool_names, model_name, max_iter,
        )
    } else {
        format!(
            "运行环境:\n- 工作目录: {}\n- 可用工具: {}\n- 模型: {}\n- 最大迭代次数: {}\n\n注意: 所有文件操作必须限制在工作目录范围内。",
            guidance.effective_work_dir, tool_names, model_name, max_iter,
        )
    };
    let temporal_execution_guidance = if guidance.local_date.trim().is_empty() {
        None
    } else {
        Some(format!(
            "时间上下文:\n- 本地时区: {}\n- 今天: {}\n- 明天: {}\n- 本月范围: {}\n- 遇到“今天”“明天”“昨天”“本周”“这个月”等相对时间表达时，先换算为上面的绝对日期或日期范围，再进行推理、搜索和回答。\n- 对新闻、政策、价格、日程等时效性内容，优先在回答中写出绝对日期，避免只写相对时间。",
            guidance.local_timezone,
            guidance.local_date,
            guidance.local_tomorrow,
            guidance.local_month_range
        ))
    };

    SystemPromptSections {
        base_prompt: base_prompt.to_string(),
        capability_snapshot,
        workspace_skills_prompt: workspace_skills_prompt
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        employee_collaboration_guidance: employee_collaboration_guidance
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        memory_content: memory_content
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        temporal_execution_guidance,
        runtime_notes: runtime_notes
            .iter()
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .collect(),
        tool_runtime_notes: collect_tool_runtime_notes(tool_names),
    }
}

pub fn compose_system_prompt_from_sections(sections: &SystemPromptSections) -> String {
    let mut prompt_sections = vec![sections.base_prompt.clone()];

    if !sections.capability_snapshot.trim().is_empty() {
        prompt_sections.push(format!("---\n{}", sections.capability_snapshot));
    }

    if let Some(skills_prompt) = sections
        .workspace_skills_prompt
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        prompt_sections.push(format!(
            "---\nSkills (mandatory):\nBefore replying, inspect the available skill descriptions below. Scan the descriptions first. If exactly one skill clearly applies, read only that skill's SKILL.md from the listed location and follow it. Do not read multiple skills up front, and do not read any skill when none clearly applies. When calling the `skill` tool, use its <invoke_name> or <location> as skill_name. Do not pass the display <name> as skill_name.\n{}\n",
            skills_prompt
        ));
    }

    if let Some(collaboration) = sections
        .employee_collaboration_guidance
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        prompt_sections.push(format!("---\n{}", collaboration));
    }

    if let Some(memory_content) = sections
        .memory_content
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        prompt_sections.push(format!("---\n持久内存:\n{}", memory_content));
    }

    if let Some(temporal_execution_guidance) = sections
        .temporal_execution_guidance
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        prompt_sections.push(format!("---\n{}", temporal_execution_guidance));
    }

    for note in &sections.tool_runtime_notes {
        if !note.trim().is_empty() {
            prompt_sections.push(format!("---\n{}", note));
        }
    }

    if !sections.runtime_notes.is_empty() {
        prompt_sections.push(format!(
            "---\n[联网检索状态]\n{}",
            sections.runtime_notes.join("\n")
        ));
    }

    prompt_sections.join("\n\n")
}

fn collect_tool_runtime_notes(tool_names: &str) -> Vec<String> {
    let browser_runtime_note = if tool_names.contains("browser") {
        Some(
            "WorkClaw 浏览器运行时说明:\n- WorkClaw 内置本地 browser sidecar，地址固定为 http://localhost:8765\n- 对 OpenClaw / Xiaohongshu 一类 skill，直接使用 WorkClaw 已提供的 `browser` 兼容工具和现有工具别名\n- 不要要求用户手动启动 OpenClaw 浏览器服务\n- 不要检查 openclaw-desktop.exe\n- 不要要求固定安装目录，例如 D:\\AI；不要要求用户额外安装 OpenClaw 桌面版\n- 如果浏览器自动化失败，应归因于 WorkClaw 内置 sidecar 或浏览器启动失败，而不是外部 OpenClaw 服务未启动".to_string(),
        )
    } else {
        None
    };
    let file_tool_note = if tool_names.contains("list_dir")
        || tool_names.contains("file_move")
        || tool_names.contains("file_copy")
        || tool_names.contains("file_delete")
    {
        Some(
            "文件工具使用说明:\n- `list_dir` 会在可读列表后追加结构化 entries JSON\n- 后续 `file_move` / `file_copy` / `file_delete` 等文件工具处理目录枚举结果时，优先直接复用 entries 中的原始 `path`\n- 不要手写或改写文件名，尤其不要自行增删空格、中文标点或扩展名".to_string(),
        )
    } else {
        None
    };
    let structured_tool_result_note = if tool_names.contains("read_file")
        || tool_names.contains("write_file")
        || tool_names.contains("edit")
        || tool_names.contains("glob")
        || tool_names.contains("grep")
        || tool_names.contains("exec")
        || tool_names.contains("bash")
        || tool_names.contains("bash_output")
        || tool_names.contains("bash_kill")
        || tool_names.contains("list_dir")
        || tool_names.contains("file_copy")
        || tool_names.contains("file_delete")
        || tool_names.contains("file_move")
        || tool_names.contains("file_stat")
    {
        Some(
            "结构化工具结果说明:\n- 对支持结构化结果的工具，优先使用工具结果中的 `summary` 和 `details` 字段进行后续推理\n- 不要从展示文本中二次猜测路径、匹配位置或命令状态\n- 文件类结果优先复用 `details` 中的精确路径或元信息\n- 命令执行结果优先读取 `exit_code`、`timed_out`、`stdout`、`stderr`".to_string(),
        )
    } else {
        None
    };

    [
        browser_runtime_note,
        file_tool_note,
        structured_tool_result_note,
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn employee_matches_session(session_employee_id: &str, employee: &ChatEmployeeSnapshot) -> bool {
    let target = session_employee_id.trim();
    if target.is_empty() {
        return false;
    }
    target.eq_ignore_ascii_case(employee.employee_id.trim())
        || target.eq_ignore_ascii_case(employee.role_id.trim())
        || target.eq_ignore_ascii_case(employee.id.trim())
}

pub(crate) fn build_employee_collaboration_guidance(
    session_employee_id: &str,
    employees: &[ChatEmployeeSnapshot],
) -> Option<String> {
    let current = employees
        .iter()
        .find(|employee| employee_matches_session(session_employee_id, employee))?;
    let collaborators = employees
        .iter()
        .filter(|employee| employee.enabled && employee.id != current.id)
        .collect::<Vec<_>>();
    if collaborators.is_empty() {
        return None;
    }

    let mut lines = vec![
        "员工协作协议:".to_string(),
        format!(
            "- 当前员工: {} (employee_id={})",
            current.name, current.employee_id
        ),
        "- 可委托员工清单:".to_string(),
    ];
    for employee in collaborators {
        lines.push(format!(
            "  - {} (employee_id={}, role_id={}, feishu_open_id={})",
            employee.name,
            employee.employee_id,
            employee.role_id,
            if employee.feishu_open_id.trim().is_empty() {
                "-"
            } else {
                employee.feishu_open_id.trim()
            }
        ));
    }
    lines.push(
        "- 当任务需要专项能力时，优先调用 task 工具委托，并在参数中填入 delegate_role_id / delegate_role_name。".to_string(),
    );
    lines.push(
        "- task.prompt 必须写清目标、输入上下文、输出格式、验收标准。收到子任务结果后再统一汇总回复用户。".to_string(),
    );
    lines.push(
        "- 如果在 IM/飞书场景需要转交某员工，先在回复中明确“已转交给谁”，再执行委托，不得只给笼统答复。".to_string(),
    );

    Some(lines.join("\n"))
}
