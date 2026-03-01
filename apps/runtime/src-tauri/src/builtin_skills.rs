pub const BUILTIN_GENERAL_SKILL_ID: &str = "builtin-general";

const BUILTIN_GENERAL_SKILL_MD: &str =
    include_str!("../builtin-skills/general-assistant/SKILL.md");
const LOCAL_SKILL_TEMPLATE_MD: &str =
    include_str!("../builtin-skills/skill-creator-guide/templates/LOCAL_SKILL_TEMPLATE.md");

pub fn builtin_skill_markdown(skill_id: &str) -> Option<&'static str> {
    match skill_id {
        BUILTIN_GENERAL_SKILL_ID => Some(BUILTIN_GENERAL_SKILL_MD),
        _ => None,
    }
}

pub fn builtin_general_skill_markdown() -> &'static str {
    BUILTIN_GENERAL_SKILL_MD
}

pub fn local_skill_template_markdown() -> &'static str {
    LOCAL_SKILL_TEMPLATE_MD
}
