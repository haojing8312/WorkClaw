use super::{
    clear_employee_memory_from_root, collect_employee_memory_stats_from_root,
    employee_memory_skills_root, export_employee_memory_from_root, normalize_employee_id,
    EmployeeMemoryExport, EmployeeMemoryStats, UpsertAgentEmployeeInput,
};
use tauri::Manager;

pub async fn get_employee_memory_stats(
    employee_id: String,
    skill_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<EmployeeMemoryStats, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let normalized_employee_id = normalize_employee_id(&employee_id)?;
    let skills_root = employee_memory_skills_root(&app_data_dir, &normalized_employee_id);
    collect_employee_memory_stats_from_root(
        &normalized_employee_id,
        skill_id.as_deref(),
        &skills_root,
    )
}

pub async fn export_employee_memory(
    employee_id: String,
    skill_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<EmployeeMemoryExport, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let normalized_employee_id = normalize_employee_id(&employee_id)?;
    let skills_root = employee_memory_skills_root(&app_data_dir, &normalized_employee_id);
    export_employee_memory_from_root(&normalized_employee_id, skill_id.as_deref(), &skills_root)
}

pub async fn clear_employee_memory(
    employee_id: String,
    skill_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<EmployeeMemoryStats, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let normalized_employee_id = normalize_employee_id(&employee_id)?;
    let skills_root = employee_memory_skills_root(&app_data_dir, &normalized_employee_id);
    clear_employee_memory_from_root(&normalized_employee_id, skill_id.as_deref(), &skills_root)?;
    collect_employee_memory_stats_from_root(
        &normalized_employee_id,
        skill_id.as_deref(),
        &skills_root,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        clear_employee_memory_from_root, collect_employee_memory_stats_from_root,
        export_employee_memory_from_root, UpsertAgentEmployeeInput,
    };
    use std::fs;
    use tempfile::TempDir;

    fn build_skill_file(root: &std::path::Path, skill_id: &str, rel: &str, content: &str) {
        let path = root.join(skill_id).join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create dir");
        }
        fs::write(path, content).expect("write file");
    }

    #[test]
    fn collect_employee_memory_stats_supports_all_and_single_skill_scope() {
        let tmp = TempDir::new().expect("tmp");
        let skills_root = tmp.path().join("skills");
        build_skill_file(
            &skills_root,
            "skill-alpha",
            "roles/main/MEMORY.md",
            "alpha fact",
        );
        build_skill_file(&skills_root, "skill-beta", "sessions/t1.md", "beta fact");

        let all = collect_employee_memory_stats_from_root("sales_lead", None, &skills_root)
            .expect("collect all");
        assert_eq!(all.employee_id, "sales_lead");
        assert_eq!(all.total_files, 2);
        assert_eq!(all.skills.len(), 2);
        assert!(all.total_bytes >= 18);

        let alpha = collect_employee_memory_stats_from_root(
            "sales_lead",
            Some("skill-alpha"),
            &skills_root,
        )
        .expect("collect alpha");
        assert_eq!(alpha.total_files, 1);
        assert_eq!(alpha.skills.len(), 1);
        assert_eq!(alpha.skills[0].skill_id, "skill-alpha");
    }

    #[test]
    fn clear_employee_memory_respects_skill_scope() {
        let tmp = TempDir::new().expect("tmp");
        let skills_root = tmp.path().join("skills");
        build_skill_file(&skills_root, "skill-alpha", "roles/main/MEMORY.md", "alpha");
        build_skill_file(&skills_root, "skill-beta", "sessions/t1.md", "beta");

        clear_employee_memory_from_root("sales_lead", Some("skill-alpha"), &skills_root)
            .expect("clear alpha");

        let remained = collect_employee_memory_stats_from_root("sales_lead", None, &skills_root)
            .expect("collect remained");
        assert_eq!(remained.total_files, 1);
        assert_eq!(remained.skills.len(), 1);
        assert_eq!(remained.skills[0].skill_id, "skill-beta");

        clear_employee_memory_from_root("sales_lead", None, &skills_root).expect("clear all");
        let empty = collect_employee_memory_stats_from_root("sales_lead", None, &skills_root)
            .expect("collect empty");
        assert_eq!(empty.total_files, 0);
        assert_eq!(empty.total_bytes, 0);
        assert!(empty.skills.is_empty());
    }

    #[test]
    fn export_employee_memory_returns_structured_json_payload() {
        let tmp = TempDir::new().expect("tmp");
        let skills_root = tmp.path().join("skills");
        build_skill_file(
            &skills_root,
            "skill-alpha",
            "roles/main/MEMORY.md",
            "customer prefers weekly summary",
        );

        let exported =
            export_employee_memory_from_root("sales_lead", Some("skill-alpha"), &skills_root)
                .expect("export");
        assert_eq!(exported.employee_id, "sales_lead");
        assert_eq!(exported.total_files, 1);
        assert_eq!(exported.total_bytes, 31);
        assert_eq!(exported.files.len(), 1);
        assert_eq!(exported.files[0].skill_id, "skill-alpha");
        assert_eq!(exported.files[0].relative_path, "roles/main/MEMORY.md");
        assert_eq!(exported.files[0].content, "customer prefers weekly summary");
    }

    #[test]
    fn upsert_input_defaults_routing_priority_when_missing() {
        let payload = serde_json::json!({
            "employee_id": "project_manager",
            "name": "项目经理",
            "role_id": "project_manager",
            "persona": "负责推进交付",
            "feishu_open_id": "",
            "feishu_app_id": "",
            "feishu_app_secret": "",
            "primary_skill_id": "builtin-general",
            "default_work_dir": "",
            "openclaw_agent_id": "project_manager",
            "enabled_scopes": ["app"],
            "enabled": true,
            "is_default": false,
            "skill_ids": []
        });
        let parsed: UpsertAgentEmployeeInput =
            serde_json::from_value(payload).expect("deserialize upsert input");
        assert_eq!(parsed.routing_priority, 100);
    }
}
