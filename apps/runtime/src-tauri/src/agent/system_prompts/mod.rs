pub mod context_mgmt;
pub mod output_style;
pub mod tool_policy;
pub mod workflow;

/// 系统级 Prompt 组合器
///
/// 将 WorkClaw 系统级 prompt 与 Skill 开发者提供的 prompt 组合，
/// 确保所有 Skill 自动获得友好的输出风格和工作流程引导。
pub struct SystemPromptBuilder {
    include_workflow: bool,
    include_output_style: bool,
    include_tool_policy: bool,
    include_context_mgmt: bool,
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self {
            include_workflow: true,
            include_output_style: true,
            include_tool_policy: true,
            include_context_mgmt: false, // 可选，默认不启用
        }
    }

    /// 启用或禁用上下文管理 prompt
    pub fn with_context_mgmt(mut self, enabled: bool) -> Self {
        self.include_context_mgmt = enabled;
        self
    }

    /// 组合系统级 prompt 和 Skill prompt
    ///
    /// Skill prompt 放在最后，优先级最高
    pub fn build(&self, skill_prompt: &str) -> String {
        let mut parts = Vec::new();

        if self.include_workflow {
            parts.push(workflow::AGENT_WORKFLOW_PROMPT);
        }
        if self.include_output_style {
            parts.push(output_style::OUTPUT_STYLE_PROMPT);
        }
        if self.include_tool_policy {
            parts.push(tool_policy::TOOL_USAGE_POLICY);
        }
        if self.include_context_mgmt {
            parts.push(context_mgmt::CONTEXT_MANAGEMENT_PROMPT);
        }

        // Skill prompt 放在最后，优先级最高
        if !skill_prompt.is_empty() {
            parts.push(skill_prompt);
        }

        parts.join("\n\n---\n\n")
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_build_includes_core_prompts() {
        let builder = SystemPromptBuilder::default();
        let result = builder.build("测试 Skill 指令");

        assert!(result.contains("# 工作流程"));
        assert!(result.contains("# 输出风格"));
        assert!(result.contains("# 工具使用策略"));
        // 默认不包含上下文管理
        assert!(!result.contains("# 上下文管理"));
        // Skill prompt 在最后
        assert!(result.contains("测试 Skill 指令"));
    }

    #[test]
    fn test_with_context_mgmt() {
        let builder = SystemPromptBuilder::new().with_context_mgmt(true);
        let result = builder.build("Skill 内容");

        assert!(result.contains("# 上下文管理"));
    }

    #[test]
    fn test_empty_skill_prompt() {
        let builder = SystemPromptBuilder::default();
        let result = builder.build("");

        // 不应有尾部的分隔符
        assert!(!result.ends_with("---\n\n"));
        assert!(result.contains("# 工作流程"));
    }

    #[test]
    fn test_sections_separated_by_divider() {
        let builder = SystemPromptBuilder::default();
        let result = builder.build("Skill");

        // 各段落之间用 --- 分隔
        assert!(result.contains("\n\n---\n\n"));
    }
}
