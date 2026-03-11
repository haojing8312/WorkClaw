pub const BUILTIN_GENERAL_SKILL_ID: &str = "builtin-general";
pub const BUILTIN_SKILL_CREATOR_ID: &str = "builtin-skill-creator";
pub const BUILTIN_DOCX_SKILL_ID: &str = "builtin-docx";
pub const BUILTIN_PDF_SKILL_ID: &str = "builtin-pdf";
pub const BUILTIN_PPTX_SKILL_ID: &str = "builtin-pptx";
pub const BUILTIN_XLSX_SKILL_ID: &str = "builtin-xlsx";
pub const BUILTIN_FIND_SKILLS_ID: &str = "builtin-find-skills";
pub const BUILTIN_EMPLOYEE_CREATOR_ID: &str = "builtin-employee-creator";

const BUILTIN_GENERAL_SKILL_MD: &str = include_str!("../builtin-skills/general-assistant/SKILL.md");
const BUILTIN_SKILL_CREATOR_MD: &str = include_str!("../builtin-skills/skill-creator/SKILL.md");
const BUILTIN_DOCX_SKILL_MD: &str = include_str!("../builtin-skills/docx/SKILL.md");
const BUILTIN_PDF_SKILL_MD: &str = include_str!("../builtin-skills/pdf/SKILL.md");
const BUILTIN_PPTX_SKILL_MD: &str = include_str!("../builtin-skills/pptx/SKILL.md");
const BUILTIN_XLSX_SKILL_MD: &str = include_str!("../builtin-skills/xlsx/SKILL.md");
const BUILTIN_FIND_SKILLS_MD: &str = include_str!("../builtin-skills/find-skills/SKILL.md");
const BUILTIN_EMPLOYEE_CREATOR_SKILL_MD: &str =
    include_str!("../builtin-skills/employee-creator/SKILL.md");
const LOCAL_SKILL_TEMPLATE_MD: &str =
    include_str!("../builtin-skills/skill-creator-guide/templates/LOCAL_SKILL_TEMPLATE.md");

pub struct BuiltinSkillEntry {
    pub id: &'static str,
    pub markdown: &'static str,
}

const BUILTIN_SKILL_ENTRIES: [BuiltinSkillEntry; 8] = [
    BuiltinSkillEntry {
        id: BUILTIN_GENERAL_SKILL_ID,
        markdown: BUILTIN_GENERAL_SKILL_MD,
    },
    BuiltinSkillEntry {
        id: BUILTIN_SKILL_CREATOR_ID,
        markdown: BUILTIN_SKILL_CREATOR_MD,
    },
    BuiltinSkillEntry {
        id: BUILTIN_DOCX_SKILL_ID,
        markdown: BUILTIN_DOCX_SKILL_MD,
    },
    BuiltinSkillEntry {
        id: BUILTIN_PDF_SKILL_ID,
        markdown: BUILTIN_PDF_SKILL_MD,
    },
    BuiltinSkillEntry {
        id: BUILTIN_PPTX_SKILL_ID,
        markdown: BUILTIN_PPTX_SKILL_MD,
    },
    BuiltinSkillEntry {
        id: BUILTIN_XLSX_SKILL_ID,
        markdown: BUILTIN_XLSX_SKILL_MD,
    },
    BuiltinSkillEntry {
        id: BUILTIN_FIND_SKILLS_ID,
        markdown: BUILTIN_FIND_SKILLS_MD,
    },
    BuiltinSkillEntry {
        id: BUILTIN_EMPLOYEE_CREATOR_ID,
        markdown: BUILTIN_EMPLOYEE_CREATOR_SKILL_MD,
    },
];

pub fn builtin_skill_markdown(skill_id: &str) -> Option<&'static str> {
    builtin_skill_entries()
        .iter()
        .find(|entry| entry.id == skill_id)
        .map(|entry| entry.markdown)
}

pub fn builtin_skill_entries() -> &'static [BuiltinSkillEntry] {
    &BUILTIN_SKILL_ENTRIES
}

pub fn builtin_general_skill_markdown() -> &'static str {
    BUILTIN_GENERAL_SKILL_MD
}

pub fn local_skill_template_markdown() -> &'static str {
    LOCAL_SKILL_TEMPLATE_MD
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::skill_config::SkillConfig;

    #[test]
    fn builtin_skill_entries_include_expert_presets() {
        let ids: Vec<&str> = builtin_skill_entries()
            .iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(ids.len(), 8);
        assert!(ids.contains(&BUILTIN_GENERAL_SKILL_ID));
        assert!(ids.contains(&BUILTIN_SKILL_CREATOR_ID));
        assert!(ids.contains(&BUILTIN_DOCX_SKILL_ID));
        assert!(ids.contains(&BUILTIN_PDF_SKILL_ID));
        assert!(ids.contains(&BUILTIN_PPTX_SKILL_ID));
        assert!(ids.contains(&BUILTIN_XLSX_SKILL_ID));
        assert!(ids.contains(&BUILTIN_FIND_SKILLS_ID));
        assert!(ids.contains(&BUILTIN_EMPLOYEE_CREATOR_ID));
    }

    #[test]
    fn builtin_skill_markdown_resolves_new_skill_ids() {
        assert!(builtin_skill_markdown(BUILTIN_SKILL_CREATOR_ID).is_some());
        assert!(builtin_skill_markdown(BUILTIN_DOCX_SKILL_ID).is_some());
        assert!(builtin_skill_markdown(BUILTIN_PDF_SKILL_ID).is_some());
        assert!(builtin_skill_markdown(BUILTIN_PPTX_SKILL_ID).is_some());
        assert!(builtin_skill_markdown(BUILTIN_XLSX_SKILL_ID).is_some());
        assert!(builtin_skill_markdown(BUILTIN_FIND_SKILLS_ID).is_some());
        assert!(builtin_skill_markdown(BUILTIN_EMPLOYEE_CREATOR_ID).is_some());
    }

    #[test]
    fn builtin_employee_creator_skill_enforces_review_before_create() {
        let markdown = builtin_skill_markdown(BUILTIN_EMPLOYEE_CREATOR_ID)
            .expect("builtin employee creator markdown should exist");
        assert!(
            markdown.contains("list_employees"),
            "employee creator should inspect existing employees before create"
        );
        assert!(
            markdown.contains("JSON"),
            "employee creator should provide a structured draft preview"
        );
        assert!(
            markdown.contains("确认创建"),
            "employee creator should require explicit confirmation before create"
        );
        assert!(
            markdown.contains("AGENTS.md")
                && markdown.contains("SOUL.md")
                && markdown.contains("USER.md"),
            "employee creator should include AGENTS/SOUL/USER delivery"
        );
        assert!(
            markdown.contains("profile_answers"),
            "employee creator should pass profile answers into employee creation"
        );
        assert!(
            markdown.contains("未提供时系统会自动从 `skill_ids` 的第一个技能推导"),
            "employee creator should explain primary skill auto-derivation"
        );
        assert!(
            !markdown.contains("routing_priority"),
            "employee creator should not ask users to configure routing priority"
        );
        assert!(
            markdown.contains("AGENTS.md`：定义员工的角色定位")
                && markdown.contains("SOUL.md`：定义行为准则")
                && markdown.contains("USER.md`：定义服务对象画像"),
            "employee creator should explain the purpose of AGENTS/SOUL/USER files"
        );
    }

    #[test]
    fn builtin_find_skills_declares_confirmation_and_github_download_tools() {
        let markdown =
            builtin_skill_markdown(BUILTIN_FIND_SKILLS_ID).expect("find-skills markdown exists");
        let config = SkillConfig::parse(markdown);
        let allowed_tools = config.allowed_tools.unwrap_or_default();

        assert!(
            allowed_tools.iter().any(|tool| tool == "ask_user"),
            "find-skills should be able to ask for 1/2 confirmation"
        );
        assert!(
            allowed_tools
                .iter()
                .any(|tool| tool == "github_repo_download"),
            "find-skills should be able to download github repos into workspace"
        );
        assert!(
            markdown.contains("输入 1 或 2"),
            "find-skills prompt should guide numeric confirmation"
        );
        assert!(
            markdown.contains("安装技能 -> 本地 Skill 目录"),
            "find-skills prompt should guide manual local install after download"
        );
    }
}
