mod builtin_skills;
mod skill_config;

pub use builtin_skills::{
    builtin_general_skill_markdown, builtin_skill_entries, builtin_skill_markdown,
    local_skill_template_markdown, BuiltinSkillEntry, BUILTIN_DOCX_SKILL_ID,
    BUILTIN_EMPLOYEE_CREATOR_ID, BUILTIN_FIND_SKILLS_ID, BUILTIN_GENERAL_SKILL_ID,
    BUILTIN_PDF_SKILL_ID, BUILTIN_PPTX_SKILL_ID, BUILTIN_SKILL_CREATOR_ID, BUILTIN_XLSX_SKILL_ID,
};
pub use skill_config::{McpServerDep, SkillConfig};
