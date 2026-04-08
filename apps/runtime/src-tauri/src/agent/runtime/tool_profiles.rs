use crate::agent::{ToolCategory, ToolRegistry, ToolSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolProfileName {
    SafeDefault,
    Coding,
    Browser,
    Employee,
}

pub(crate) fn resolve_tool_profile(
    registry: &ToolRegistry,
    profile: ToolProfileName,
) -> Vec<String> {
    let mut names = registry
        .tool_manifest_entries()
        .into_iter()
        .filter(|entry| matches_profile(profile, entry))
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn matches_profile(
    profile: ToolProfileName,
    entry: &crate::agent::ToolManifestEntry,
) -> bool {
    match profile {
        ToolProfileName::SafeDefault => matches_safe_default(entry),
        ToolProfileName::Coding => matches_coding(entry),
        ToolProfileName::Browser => matches_browser(entry),
        ToolProfileName::Employee => matches_employee(entry),
    }
}

fn matches_safe_default(entry: &crate::agent::ToolManifestEntry) -> bool {
    if matches!(
        entry.name.as_str(),
        "skill"
            | "compact"
            | "ask_user"
            | "read"
            | "find"
            | "ls"
            | "web_search"
            | "web_fetch"
            | "read_file"
            | "glob"
            | "grep"
            | "list_dir"
            | "file_stat"
            | "todo_write"
    ) {
        return true;
    }

    entry.read_only
        && !entry.destructive
        && !matches!(entry.category, ToolCategory::Browser | ToolCategory::Shell)
        && !matches!(entry.source, ToolSource::Plugin)
}

fn matches_coding(entry: &crate::agent::ToolManifestEntry) -> bool {
    matches_safe_default(entry)
        || matches!(
            entry.name.as_str(),
            "write_file"
                | "edit"
                | "file_copy"
                | "file_move"
                | "open_in_folder"
                | "bash"
                | "bash_output"
                | "bash_kill"
                | "exec"
        )
}

fn matches_browser(entry: &crate::agent::ToolManifestEntry) -> bool {
    matches_safe_default(entry)
        || matches!(entry.category, ToolCategory::Browser)
        || entry.name.starts_with("browser_")
}

fn matches_employee(entry: &crate::agent::ToolManifestEntry) -> bool {
    matches_safe_default(entry)
        || matches!(
            entry.name.as_str(),
            "task"
                | "memory"
                | "employee_manage"
                | "clawhub_search"
                | "clawhub_recommend"
                | "github_repo_download"
        )
}

#[cfg(test)]
mod tests {
    use super::{resolve_tool_profile, ToolProfileName};
    use crate::agent::runtime::tool_registry_builder::{
        RuntimeToolRegistryBuilder, DEFAULT_BROWSER_SIDECAR_URL,
    };
    use crate::agent::tools::{ProcessManager, TaskTool};
    use crate::agent::ToolRegistry;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn named_profiles_resolve_predictable_tool_sets() {
        let registry = Arc::new(ToolRegistry::with_standard_tools());
        let builder = RuntimeToolRegistryBuilder::new(registry.as_ref());
        builder.register_process_shell_tools(Arc::new(ProcessManager::new()));
        builder.register_browser_and_alias_tools(DEFAULT_BROWSER_SIDECAR_URL);
        builder.register_skill_and_compaction_tools("sess-profile", Vec::new(), 2);

        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let memory_dir = tempdir().expect("temp dir");
        let task_tool = TaskTool::new(
            Arc::clone(&registry),
            "openai".to_string(),
            "https://example.com".to_string(),
            "test-key".to_string(),
            "gpt-4o-mini".to_string(),
        );
        builder.register_runtime_support_tools(task_tool, db, memory_dir.path().to_path_buf());

        let safe_default = resolve_tool_profile(registry.as_ref(), ToolProfileName::SafeDefault);
        let coding = resolve_tool_profile(registry.as_ref(), ToolProfileName::Coding);
        let browser = resolve_tool_profile(registry.as_ref(), ToolProfileName::Browser);
        let employee = resolve_tool_profile(registry.as_ref(), ToolProfileName::Employee);

        assert!(safe_default.contains(&"read_file".to_string()));
        assert!(safe_default.contains(&"skill".to_string()));
        assert!(!safe_default.contains(&"write_file".to_string()));
        assert!(!safe_default.contains(&"browser_launch".to_string()));

        assert!(coding.contains(&"write_file".to_string()));
        assert!(coding.contains(&"bash".to_string()));

        assert!(browser.contains(&"browser_launch".to_string()));
        assert!(browser.contains(&"browser_snapshot".to_string()));
        assert!(!browser.contains(&"bash".to_string()));

        assert!(employee.contains(&"task".to_string()));
        assert!(employee.contains(&"employee_manage".to_string()));
        assert!(employee.contains(&"memory".to_string()));
    }
}
