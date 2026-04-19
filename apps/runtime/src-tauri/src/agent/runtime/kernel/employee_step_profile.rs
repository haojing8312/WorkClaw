use crate::agent::run_guard::{RunBudgetPolicy, RunBudgetScope};
use crate::agent::skill_config::SkillConfig;
use crate::agent_catalog::agent_definition::{
    default_memory_scope_for_role, normalize_agent_id, AgentDefinition, AgentRoleKind,
};
use crate::agent_catalog::agent_permissions::{
    derive_allowed_tools_for_role, derive_capabilities_for_role,
};
use crate::agent_catalog::agent_workspace::build_agent_profile_context;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub(crate) struct EmployeeStepPersona<'a> {
    pub name: &'a str,
    pub employee_id: &'a str,
    pub role_id: &'a str,
    pub persona: &'a str,
    pub default_work_dir: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EmployeeStepExecutionProfile {
    pub base_prompt: String,
    pub allowed_tools: Option<Vec<String>>,
    pub max_iterations: usize,
}

pub(crate) fn build_employee_step_execution_profile(
    employee: EmployeeStepPersona<'_>,
    session_skill_id: &str,
) -> EmployeeStepExecutionProfile {
    let agent_definition = build_executor_agent_definition(employee);
    let skill_config = SkillConfig::parse(crate::builtin_skills::builtin_general_skill_markdown());
    let base_prompt = if skill_config.system_prompt.trim().is_empty() {
        "你是一名专业、可靠、注重交付结果的 AI 员工。".to_string()
    } else {
        skill_config.system_prompt.clone()
    };
    let profile_markdown = load_group_step_profile_markdown(&employee);
    let mut sections = vec![
        base_prompt,
        "---".to_string(),
        "你当前正在复杂任务团队中，以真实员工身份执行内部步骤。".to_string(),
        format!("- 员工名称: {}", employee.name),
        format!("- employee_id: {}", employee.employee_id),
        format!("- role_id: {}", employee.role_id),
        format!(
            "- primary_skill_id: {}",
            if session_skill_id.trim().is_empty() {
                "builtin-general"
            } else {
                session_skill_id.trim()
            }
        ),
    ];
    if !employee.default_work_dir.trim().is_empty() {
        sections.push(format!("- 工作目录: {}", agent_definition.workspace_dir));
    }
    if !agent_definition.persona_text.trim().is_empty() {
        sections.push(format!("- 员工人设: {}", agent_definition.persona_text));
    }
    sections.push(format!("- role_kind: {:?}", agent_definition.role_kind));
    sections.push(format!(
        "- memory_scope: {:?}",
        agent_definition.memory_scope
    ));
    sections.push(
        "执行要求:\n- 聚焦当前分配步骤\n- 优先直接用自然语言给出结论，只有在当前步骤明确需要读取文件、编辑文件、执行命令或抓取网页时才使用工具\n- 先给结论，再给关键依据或产出\n- 不要输出“模拟结果”或“占位结果”措辞".to_string(),
    );
    if !profile_markdown.is_empty() {
        sections.push(format!("员工资料:\n{profile_markdown}"));
    }

    EmployeeStepExecutionProfile {
        base_prompt: sections.join("\n"),
        allowed_tools: Some(agent_definition.allowed_tools),
        max_iterations: RunBudgetPolicy::resolve(
            RunBudgetScope::Employee,
            skill_config.max_iterations,
        )
        .max_turns,
    }
}

pub(crate) fn build_employee_step_user_prompt(
    run_id: &str,
    step_id: &str,
    user_goal: &str,
    step_input: &str,
    employee: EmployeeStepPersona<'_>,
) -> String {
    let effective_input = if step_input.trim().is_empty() {
        user_goal.trim()
    } else {
        step_input.trim()
    };
    format!(
        "你正在执行多员工团队中的 execute 步骤。\n- run_id: {run_id}\n- step_id: {step_id}\n- 当前负责人: {} ({})\n- 用户总目标: {}\n- 当前步骤要求: {}\n\n请直接给出你的执行结果。如果信息不足，先指出缺口，再给最合理的下一步。",
        employee.name,
        employee.employee_id,
        user_goal.trim(),
        effective_input,
    )
}

pub(crate) fn build_employee_step_iteration_fallback_output(
    employee: EmployeeStepPersona<'_>,
    user_goal: &str,
    step_input: &str,
    error: &str,
) -> String {
    let focus = if step_input.trim().is_empty() {
        user_goal.trim()
    } else {
        step_input.trim()
    };
    let responsibility = if employee.persona.trim().is_empty() {
        format!("负责围绕“{}”完成分配到本岗位的执行项", focus)
    } else {
        employee.persona.trim().to_string()
    };
    format!(
        "{} ({}) 在执行步骤时触发了迭代上限，现切换为保守交付模式。\n- 当前步骤: {}\n- 岗位职责: {}\n- 对用户目标“{}”可立即提供: 基于本岗位职责给出能力范围说明、所需补充信息以及下一步执行建议。\n- 备注: {}",
        employee.name,
        employee.employee_id,
        focus,
        responsibility,
        user_goal.trim(),
        error.trim(),
    )
}

fn load_group_step_profile_markdown(employee: &EmployeeStepPersona<'_>) -> String {
    if employee.default_work_dir.trim().is_empty() {
        return String::new();
    }

    let profile_dir = PathBuf::from(employee.default_work_dir.trim())
        .join("openclaw")
        .join(employee.employee_id.trim());
    let mut sections = Vec::new();
    for name in ["AGENTS.md", "SOUL.md", "USER.md"] {
        let path = profile_dir.join(name);
        if let Ok(content) = std::fs::read_to_string(path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                sections.push(format!("## {name}\n{trimmed}"));
            }
        }
    }
    sections.join("\n\n")
}

fn build_executor_agent_definition(employee: EmployeeStepPersona<'_>) -> AgentDefinition {
    let profile_context = build_agent_profile_context(employee.default_work_dir, employee.persona);
    let role_kind = AgentRoleKind::Executor;
    AgentDefinition {
        agent_id: normalize_agent_id(employee.employee_id),
        display_name: employee.name.trim().to_string(),
        role_kind: role_kind.clone(),
        workspace_dir: profile_context.workspace_dir,
        persona_text: profile_context.persona_text,
        allowed_tools: derive_allowed_tools_for_role(&role_kind),
        permission_mode: "default".to_string(),
        model_id: None,
        memory_scope: default_memory_scope_for_role(&role_kind),
        capabilities: derive_capabilities_for_role(&role_kind),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_employee_step_execution_profile, build_employee_step_iteration_fallback_output,
        build_employee_step_user_prompt, EmployeeStepPersona,
    };

    fn sample_persona<'a>() -> EmployeeStepPersona<'a> {
        EmployeeStepPersona {
            name: "产品经理",
            employee_id: "pm-1",
            role_id: "role-pm",
            persona: "负责围绕目标拆解执行路径",
            default_work_dir: "E:/workspace/demo",
        }
    }

    #[test]
    fn employee_step_execution_profile_keeps_prompt_and_budget_together() {
        let profile = build_employee_step_execution_profile(sample_persona(), "builtin-general");

        assert!(profile.base_prompt.contains("复杂任务团队"));
        assert!(profile.base_prompt.contains("产品经理"));
        assert!(profile.allowed_tools.is_some());
        assert!(profile.max_iterations > 0);
    }

    #[test]
    fn employee_step_user_prompt_prefers_step_input() {
        let prompt = build_employee_step_user_prompt(
            "run-1",
            "step-1",
            "完成周报",
            "汇总日报并整理风险",
            sample_persona(),
        );

        assert!(prompt.contains("run-1"));
        assert!(prompt.contains("step-1"));
        assert!(prompt.contains("汇总日报并整理风险"));
    }

    #[test]
    fn employee_step_iteration_fallback_output_preserves_persona_context() {
        let output = build_employee_step_iteration_fallback_output(
            sample_persona(),
            "完成周报",
            "汇总日报并整理风险",
            "达到执行步数上限",
        );

        assert!(output.contains("保守交付模式"));
        assert!(output.contains("负责围绕目标拆解执行路径"));
        assert!(output.contains("达到执行步数上限"));
    }
}
