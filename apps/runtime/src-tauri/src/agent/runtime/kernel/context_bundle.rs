use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
use runtime_chat_app::{
    build_system_prompt_sections, compose_system_prompt_from_sections, ChatExecutionGuidance,
    SystemPromptSections,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ContextBundle {
    pub system_prompt: String,
    pub prompt_sections: SystemPromptSections,
}

impl ContextBundle {
    pub(crate) fn build(
        base_prompt: &str,
        capability_snapshot: &CapabilitySnapshot,
        model_name: &str,
        max_iter: usize,
        guidance: &ChatExecutionGuidance,
        workspace_skills_prompt: Option<String>,
        employee_collaboration_guidance: Option<String>,
        memory_content: Option<String>,
    ) -> Self {
        let prompt_sections = build_system_prompt_sections(
            base_prompt,
            &capability_snapshot.resolved_tool_names.join(", "),
            model_name,
            max_iter,
            guidance,
            workspace_skills_prompt.as_deref(),
            employee_collaboration_guidance.as_deref(),
            memory_content.as_deref(),
            &capability_snapshot.runtime_notes,
        );
        let system_prompt = compose_system_prompt_from_sections(&prompt_sections);

        Self {
            system_prompt,
            prompt_sections,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ContextBundle;
    use crate::agent::runtime::kernel::capability_snapshot::CapabilitySnapshot;
    use runtime_chat_app::ChatExecutionGuidance;

    #[test]
    fn context_bundle_exposes_prompt_sections() {
        let bundle = ContextBundle::default();

        assert!(bundle.system_prompt.is_empty());
        assert!(bundle.prompt_sections.workspace_skills_prompt.is_none());
        assert!(bundle.prompt_sections.memory_content.is_none());
        assert!(bundle.prompt_sections.runtime_notes.is_empty());
    }

    #[test]
    fn context_bundle_builds_prompt_from_capability_snapshot() {
        let snapshot = CapabilitySnapshot {
            allowed_tools: Some(vec!["browser".to_string(), "read".to_string()]),
            resolved_tool_names: vec!["browser".to_string(), "read".to_string()],
            skill_command_specs: Vec::new(),
            runtime_notes: vec!["当前未配置搜索引擎".to_string()],
        };
        let bundle = ContextBundle::build(
            "Base skill prompt",
            &snapshot,
            "gpt-4.1",
            8,
            &ChatExecutionGuidance {
                effective_work_dir: "E:/workspace/demo".to_string(),
                local_timezone: "Asia/Shanghai".to_string(),
                local_date: "2026-03-20".to_string(),
                local_tomorrow: "2026-03-21".to_string(),
                local_month_range: "2026-03-01 ~ 2026-03-31".to_string(),
            },
            Some("<available_skills />".to_string()),
            Some("Collaborate with employee-1".to_string()),
            Some("Remember previous delivery constraints.".to_string()),
        );

        assert!(bundle.system_prompt.contains("Base skill prompt"));
        assert!(bundle.system_prompt.contains("可用工具: browser, read"));
        assert!(bundle.system_prompt.contains("Collaborate with employee-1"));
        assert!(bundle
            .system_prompt
            .contains("Remember previous delivery constraints."));
        assert!(bundle.system_prompt.contains("当前未配置搜索引擎"));
        assert_eq!(
            bundle.prompt_sections.workspace_skills_prompt.as_deref(),
            Some("<available_skills />")
        );
        assert_eq!(
            bundle.prompt_sections.memory_content.as_deref(),
            Some("Remember previous delivery constraints.")
        );
        assert_eq!(
            bundle.prompt_sections.runtime_notes,
            vec!["当前未配置搜索引擎".to_string()]
        );
        assert!(bundle
            .prompt_sections
            .temporal_execution_guidance
            .as_deref()
            .expect("temporal execution guidance")
            .contains("今天: 2026-03-20"));
    }
}
