use runtime_skill_core::{
    OpenClawSkillMetadata, SkillCommandDispatchSpec, SkillConfig, SkillInvocationPolicy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSkillPromptEntry {
    pub skill_id: String,
    pub invoke_name: String,
    pub name: String,
    pub description: String,
    pub skill_md_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSkillCommandSpec {
    pub name: String,
    pub skill_id: String,
    pub skill_name: String,
    pub description: String,
    pub dispatch: Option<SkillCommandDispatchSpec>,
}

#[derive(Debug, Clone)]
pub enum WorkspaceSkillContent {
    LocalDir(std::path::PathBuf),
    FileTree(std::collections::HashMap<String, Vec<u8>>),
}

#[derive(Debug, Clone)]
pub struct WorkspaceSkillRuntimeEntry {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub source_type: String,
    pub projected_dir_name: String,
    pub config: SkillConfig,
    pub invocation: SkillInvocationPolicy,
    pub metadata: Option<OpenClawSkillMetadata>,
    pub command_dispatch: Option<SkillCommandDispatchSpec>,
    pub content: WorkspaceSkillContent,
}
