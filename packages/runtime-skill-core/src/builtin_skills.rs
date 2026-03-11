pub const BUILTIN_GENERAL_SKILL_ID: &str = "builtin-general";
pub const BUILTIN_SKILL_CREATOR_ID: &str = "builtin-skill-creator";
pub const BUILTIN_DOCX_SKILL_ID: &str = "builtin-docx";
pub const BUILTIN_PDF_SKILL_ID: &str = "builtin-pdf";
pub const BUILTIN_PPTX_SKILL_ID: &str = "builtin-pptx";
pub const BUILTIN_XLSX_SKILL_ID: &str = "builtin-xlsx";
pub const BUILTIN_FIND_SKILLS_ID: &str = "builtin-find-skills";
pub const BUILTIN_EMPLOYEE_CREATOR_ID: &str = "builtin-employee-creator";

const BUILTIN_GENERAL_SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/runtime/src-tauri/builtin-skills/general-assistant/SKILL.md"
));
const BUILTIN_SKILL_CREATOR_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/runtime/src-tauri/builtin-skills/skill-creator/SKILL.md"
));
const BUILTIN_DOCX_SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/runtime/src-tauri/builtin-skills/docx/SKILL.md"
));
const BUILTIN_PDF_SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/runtime/src-tauri/builtin-skills/pdf/SKILL.md"
));
const BUILTIN_PPTX_SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/runtime/src-tauri/builtin-skills/pptx/SKILL.md"
));
const BUILTIN_XLSX_SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/runtime/src-tauri/builtin-skills/xlsx/SKILL.md"
));
const BUILTIN_FIND_SKILLS_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/runtime/src-tauri/builtin-skills/find-skills/SKILL.md"
));
const BUILTIN_EMPLOYEE_CREATOR_SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/runtime/src-tauri/builtin-skills/employee-creator/SKILL.md"
));
const LOCAL_SKILL_TEMPLATE_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/runtime/src-tauri/builtin-skills/skill-creator-guide/templates/LOCAL_SKILL_TEMPLATE.md"
));

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
