#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceSkillPromptEntry {
    pub skill_id: String,
    pub invoke_name: String,
    pub name: String,
    pub description: String,
    pub skill_md_path: String,
}

#[derive(Debug, Clone)]
pub(crate) enum WorkspaceSkillContent {
    LocalDir(std::path::PathBuf),
    FileTree(std::collections::HashMap<String, Vec<u8>>),
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceSkillRuntimeEntry {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub source_type: String,
    pub projected_dir_name: String,
    pub content: WorkspaceSkillContent,
}
