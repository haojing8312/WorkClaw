use super::runtime_support::tool_ctx_from_work_dir;
use super::types::{
    WorkspaceSkillCommandSpec, WorkspaceSkillContent, WorkspaceSkillPromptEntry,
    WorkspaceSkillRuntimeEntry,
};
use crate::agent::skill_config::SkillConfig;
use serde_json::Value;

const SKILL_COMMAND_MAX_LENGTH: usize = 32;
const SKILL_COMMAND_FALLBACK: &str = "skill";
const WORKSPACE_SKILL_ID_MARKER_FILE: &str = ".workclaw-skill-id";

pub fn normalize_workspace_skill_dir_name(skill_id: &str) -> String {
    let mut out = String::new();
    let mut last_sep = false;
    for ch in skill_id.trim().chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            out.push(normalized);
            last_sep = false;
        } else if matches!(normalized, '-' | '_') {
            out.push(normalized);
            last_sep = false;
        } else if !last_sep {
            out.push('-');
            last_sep = true;
        }
    }
    let trimmed = out.trim_matches(['-', '_']).to_string();
    if trimmed.is_empty() {
        "skill".to_string()
    } else {
        trimmed
    }
}

pub fn build_workspace_skill_markdown_path(
    work_dir: &std::path::Path,
    projected_dir_name: &str,
) -> std::path::PathBuf {
    work_dir
        .join("skills")
        .join(normalize_workspace_skill_dir_name(projected_dir_name))
        .join("SKILL.md")
}

pub fn build_workspace_skill_prompt_entry(entry: &WorkspaceSkillPromptEntry) -> String {
    format!(
        "<skill>\n<name>{}</name>\n<invoke_name>{}</invoke_name>\n<description>{}</description>\n<location>{}</location>\n</skill>",
        entry.name.trim(),
        entry.invoke_name.trim(),
        entry.description.trim(),
        entry.skill_md_path.trim()
    )
}

pub fn build_workspace_skills_prompt(entries: &[WorkspaceSkillPromptEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut blocks = Vec::with_capacity(entries.len() + 2);
    blocks.push("<available_skills>".to_string());
    blocks.extend(entries.iter().map(build_workspace_skill_prompt_entry));
    blocks.push("</available_skills>".to_string());
    blocks.join("\n")
}

fn sanitize_skill_command_name(raw: &str) -> String {
    let mut out = String::new();
    let mut last_sep = false;
    for ch in raw.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            out.push(ch);
            last_sep = false;
        } else if !last_sep {
            out.push('_');
            last_sep = true;
        }

        if out.len() >= SKILL_COMMAND_MAX_LENGTH {
            break;
        }
    }

    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        SKILL_COMMAND_FALLBACK.to_string()
    } else {
        trimmed.to_string()
    }
}

fn resolve_unique_skill_command_name(
    base: &str,
    used: &mut std::collections::HashSet<String>,
) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
    }

    for index in 2..10_000 {
        let suffix = format!("_{index}");
        let max_base_len = SKILL_COMMAND_MAX_LENGTH.saturating_sub(suffix.len()).max(1);
        let trimmed_base = &base[..base.len().min(max_base_len)];
        let candidate = format!("{trimmed_base}{suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }

    let fallback = format!(
        "{}_x",
        &base[..base
            .len()
            .min(SKILL_COMMAND_MAX_LENGTH.saturating_sub(2).max(1))]
    );
    let _ = used.insert(fallback.clone());
    fallback
}

fn resolve_unique_projected_skill_dir_name(
    base: &str,
    used: &mut std::collections::HashSet<String>,
) -> String {
    let normalized_base = normalize_workspace_skill_dir_name(base);
    if used.insert(normalized_base.clone()) {
        return normalized_base;
    }

    for index in 2..10_000 {
        let candidate = format!("{normalized_base}-{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }

    let fallback = format!("{normalized_base}-x");
    let _ = used.insert(fallback.clone());
    fallback
}

fn resolve_projected_workspace_entries(
    entries: &[WorkspaceSkillRuntimeEntry],
) -> Vec<WorkspaceSkillRuntimeEntry> {
    let mut used = std::collections::HashSet::new();
    entries
        .iter()
        .cloned()
        .map(|mut entry| {
            entry.projected_dir_name =
                resolve_unique_projected_skill_dir_name(&entry.projected_dir_name, &mut used);
            entry
        })
        .collect()
}

pub fn build_workspace_skill_command_specs(
    entries: &[WorkspaceSkillRuntimeEntry],
) -> Vec<WorkspaceSkillCommandSpec> {
    let mut used = std::collections::HashSet::new();

    entries
        .iter()
        .filter(|entry| {
            entry.invocation.user_invocable
                && (!entry.invocation.disable_model_invocation || entry.command_dispatch.is_some())
        })
        .map(|entry| {
            let base = sanitize_skill_command_name(&entry.name);
            let unique = resolve_unique_skill_command_name(&base, &mut used);
            WorkspaceSkillCommandSpec {
                name: unique,
                skill_id: entry.skill_id.clone(),
                skill_name: entry.name.clone(),
                description: if entry.description.trim().is_empty() {
                    entry.name.clone()
                } else {
                    entry.description.clone()
                },
                dispatch: entry.command_dispatch.clone(),
            }
        })
        .collect()
}

pub fn extract_skill_prompt_from_decrypted_files(
    files: &std::collections::HashMap<String, Vec<u8>>,
) -> Option<String> {
    for key in ["SKILL.md", "skill.md"] {
        if let Some(bytes) = files.get(key) {
            return String::from_utf8(bytes.clone()).ok();
        }
    }
    for (path, bytes) in files {
        if path
            .rsplit(['/', '\\'])
            .next()
            .map(|name| name.eq_ignore_ascii_case("skill.md"))
            .unwrap_or(false)
        {
            if let Ok(content) = String::from_utf8(bytes.clone()) {
                return Some(content);
            }
        }
    }
    None
}

pub(crate) fn read_local_skill_prompt(pack_path: &str) -> Option<String> {
    let base = std::path::Path::new(pack_path);
    if !base.exists() {
        return None;
    }
    if base.is_file() {
        return std::fs::read_to_string(base).ok();
    }

    let entries = std::fs::read_dir(base).ok()?;
    for entry in entries.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case("skill.md")
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                return Some(content);
            }
        }
    }

    None
}

pub(crate) fn resolve_directory_backed_skill_root(
    source_type: &str,
    pack_path: &str,
) -> Option<std::path::PathBuf> {
    match source_type {
        "local" | "vendored" => {
            let path = std::path::PathBuf::from(pack_path);
            if path.exists() {
                Some(path)
            } else {
                None
            }
        }
        "builtin" => {
            let path = std::path::PathBuf::from(pack_path);
            if pack_path.trim().is_empty() || !path.exists() {
                None
            } else {
                Some(path)
            }
        }
        _ => None,
    }
}

fn load_legacy_builtin_embedded_files(
    skill_id: &str,
) -> (std::collections::HashMap<String, Vec<u8>>, String) {
    let markdown = crate::builtin_skills::builtin_skill_markdown(skill_id)
        .unwrap_or(crate::builtin_skills::builtin_general_skill_markdown());
    let files = crate::builtin_skills::builtin_skill_files(skill_id).unwrap_or_else(|| {
        let mut fallback = std::collections::HashMap::new();
        fallback.insert("SKILL.md".to_string(), markdown.as_bytes().to_vec());
        fallback
    });
    (files, markdown.to_string())
}

fn load_legacy_builtin_embedded_markdown(skill_id: &str) -> String {
    crate::builtin_skills::builtin_skill_markdown(skill_id)
        .unwrap_or(crate::builtin_skills::builtin_general_skill_markdown())
        .to_string()
}

pub fn resolve_workspace_skill_runtime_entry(
    skill_id: &str,
    manifest_json: &str,
    username: &str,
    pack_path: &str,
    source_type: &str,
) -> Result<WorkspaceSkillRuntimeEntry, String> {
    let manifest: skillpack_rs::SkillManifest =
        serde_json::from_str(manifest_json).map_err(|e| e.to_string())?;
    let (projected_dir_name, content, raw_skill_markdown) =
        if let Some(skill_root) = resolve_directory_backed_skill_root(source_type, pack_path) {
            let projected_dir_name = skill_root
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.trim().is_empty())
                .map(|name| name.to_string())
                .unwrap_or_else(|| normalize_workspace_skill_dir_name(skill_id));
            (
                projected_dir_name,
                WorkspaceSkillContent::LocalDir(skill_root),
                read_local_skill_prompt(pack_path),
            )
        } else if source_type == "builtin" {
            let (files, markdown) = load_legacy_builtin_embedded_files(skill_id);
            (
                normalize_workspace_skill_dir_name(skill_id),
                WorkspaceSkillContent::FileTree(files),
                Some(markdown),
            )
        } else {
            let unpacked = skillpack_rs::verify_and_unpack(pack_path, username)
                .map_err(|e| format!("解包 Skill 失败: {}", e))?;
            let markdown = extract_skill_prompt_from_decrypted_files(&unpacked.files);
            (
                normalize_workspace_skill_dir_name(skill_id),
                WorkspaceSkillContent::FileTree(unpacked.files),
                markdown,
            )
        };
    let config = raw_skill_markdown
        .as_deref()
        .map(SkillConfig::parse)
        .unwrap_or_default();

    Ok(WorkspaceSkillRuntimeEntry {
        skill_id: skill_id.to_string(),
        name: config.name.clone().unwrap_or(manifest.name),
        description: config.description.clone().unwrap_or(manifest.description),
        source_type: source_type.to_string(),
        projected_dir_name,
        invocation: config.invocation.clone(),
        metadata: config.metadata.clone(),
        command_dispatch: config.command_dispatch.clone(),
        config,
        content,
    })
}

fn validate_relative_skill_file_path(path: &str) -> Result<std::path::PathBuf, String> {
    let candidate = std::path::PathBuf::from(path);
    if candidate.is_absolute() {
        return Err(format!("Skill 文件路径必须是相对路径: {}", path));
    }
    for component in candidate.components() {
        match component {
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(format!("Skill 文件路径不安全: {}", path));
            }
            _ => {}
        }
    }
    Ok(candidate)
}

fn copy_local_skill_dir_recursive(
    source_dir: &std::path::Path,
    dest_dir: &std::path::Path,
) -> Result<(), String> {
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    for entry in walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".git")
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        let rel = path.strip_prefix(source_dir).map_err(|e| e.to_string())?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dest_dir.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(path, &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn write_skill_file_tree(
    dest_dir: &std::path::Path,
    files: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<(), String> {
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    for (rel_path, bytes) in files {
        let safe_rel = validate_relative_skill_file_path(rel_path)?;
        let target = dest_dir.join(safe_rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(target, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_workspace_skill_id_marker(
    dest_dir: &std::path::Path,
    skill_id: &str,
) -> Result<(), String> {
    std::fs::write(dest_dir.join(WORKSPACE_SKILL_ID_MARKER_FILE), skill_id)
        .map_err(|e| e.to_string())
}

pub fn sync_workspace_skills_to_directory(
    work_dir: &std::path::Path,
    entries: &[WorkspaceSkillRuntimeEntry],
) -> Result<(), String> {
    let projected_entries = resolve_projected_workspace_entries(entries);
    let skills_root = work_dir.join("skills");
    if skills_root.exists() {
        std::fs::remove_dir_all(&skills_root).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&skills_root).map_err(|e| e.to_string())?;

    for entry in &projected_entries {
        let dest_dir = skills_root.join(&entry.projected_dir_name);
        match &entry.content {
            WorkspaceSkillContent::LocalDir(source_dir) => {
                copy_local_skill_dir_recursive(source_dir, &dest_dir)?
            }
            WorkspaceSkillContent::FileTree(files) => write_skill_file_tree(&dest_dir, files)?,
        }
        write_workspace_skill_id_marker(&dest_dir, &entry.skill_id)?;
    }

    Ok(())
}

pub fn build_workspace_skill_prompt_entries(
    work_dir: &std::path::Path,
    entries: &[WorkspaceSkillRuntimeEntry],
) -> Vec<WorkspaceSkillPromptEntry> {
    resolve_projected_workspace_entries(entries)
        .into_iter()
        .filter(|entry| !entry.invocation.disable_model_invocation)
        .map(|entry| WorkspaceSkillPromptEntry {
            skill_id: entry.skill_id.clone(),
            invoke_name: entry.skill_id.clone(),
            name: entry.name.clone(),
            description: entry.description.clone(),
            skill_md_path: build_workspace_skill_markdown_path(
                work_dir,
                &entry.projected_dir_name,
            )
                .to_string_lossy()
                .to_string(),
        })
        .collect()
}

pub fn prepare_workspace_skills_prompt(
    work_dir: &std::path::Path,
    entries: &[WorkspaceSkillRuntimeEntry],
) -> Result<String, String> {
    sync_workspace_skills_to_directory(work_dir, entries)?;
    let prompt_entries = build_workspace_skill_prompt_entries(work_dir, entries);
    Ok(build_workspace_skills_prompt(&prompt_entries))
}

pub async fn load_workspace_skill_runtime_entries_with_pool(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<WorkspaceSkillRuntimeEntry>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, manifest, username, pack_path, COALESCE(source_type, 'encrypted')
         FROM installed_skills
         ORDER BY installed_at ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    for (skill_id, manifest_json, username, pack_path, source_type) in rows {
        match resolve_workspace_skill_runtime_entry(
            &skill_id,
            &manifest_json,
            &username,
            &pack_path,
            &source_type,
        ) {
            Ok(entry) => entries.push(entry),
            Err(err) => {
                eprintln!(
                    "[skills] 跳过无法投影的 skill {} (source_type={}): {}",
                    skill_id, source_type, err
                );
            }
        }
    }

    Ok(entries)
}

pub fn extract_assistant_text_content(content: &str) -> String {
    let Ok(parsed) = serde_json::from_str::<Value>(content) else {
        return content.to_string();
    };

    parsed
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| content.to_string())
}

pub fn load_skill_prompt(
    skill_id: &str,
    manifest_json: &str,
    username: &str,
    pack_path: &str,
    source_type: &str,
) -> Result<String, String> {
    let raw_prompt = if let Some(_) = resolve_directory_backed_skill_root(source_type, pack_path) {
        read_local_skill_prompt(pack_path).unwrap_or_else(|| {
            serde_json::from_str::<skillpack_rs::SkillManifest>(manifest_json)
                .map(|m| m.description)
                .unwrap_or_default()
        })
    } else if source_type == "builtin" {
        load_legacy_builtin_embedded_markdown(skill_id)
    } else {
        match skillpack_rs::verify_and_unpack(pack_path, username) {
            Ok(unpacked) => extract_skill_prompt_from_decrypted_files(&unpacked.files)
                .unwrap_or_else(|| {
                    serde_json::from_str::<skillpack_rs::SkillManifest>(manifest_json)
                        .map(|m| m.description)
                        .unwrap_or_default()
                }),
            Err(_) => {
                let manifest: skillpack_rs::SkillManifest =
                    serde_json::from_str(manifest_json).map_err(|e| e.to_string())?;
                manifest.description
            }
        }
    };

    Ok(crate::builtin_skills::apply_builtin_todowrite_governance(
        skill_id,
        source_type,
        &raw_prompt,
    ))
}

pub fn build_skill_roots(
    effective_work_dir: &str,
    source_type: &str,
    pack_path: &str,
) -> Vec<std::path::PathBuf> {
    let mut skill_roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(wd) = tool_ctx_from_work_dir(effective_work_dir) {
        skill_roots.push(wd.join(".claude").join("skills"));
        skill_roots.push(wd.join("skills"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        skill_roots.push(cwd.join(".claude").join("skills"));
    }
    if let Some(skill_path) = resolve_directory_backed_skill_root(source_type, pack_path) {
        if let Some(parent) = skill_path.parent() {
            skill_roots.push(parent.to_path_buf());
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        skill_roots.push(
            std::path::PathBuf::from(profile)
                .join(".claude")
                .join("skills"),
        );
    }
    let mut seen = std::collections::HashSet::new();
    skill_roots.retain(|path| seen.insert(path.clone()));
    skill_roots
}

#[cfg(test)]
mod workspace_skill_projection_tests {
    use super::{
        build_skill_roots, build_workspace_skill_command_specs, build_workspace_skill_markdown_path,
        build_workspace_skill_prompt_entries, build_workspace_skill_prompt_entry,
        build_workspace_skills_prompt, extract_assistant_text_content,
        normalize_workspace_skill_dir_name, prepare_workspace_skills_prompt,
        resolve_workspace_skill_runtime_entry, sync_workspace_skills_to_directory,
        WorkspaceSkillContent, WorkspaceSkillPromptEntry, WorkspaceSkillRuntimeEntry,
    };
    use chrono::Utc;
    use runtime_skill_core::SkillConfig;
    use skillpack_rs::{pack, PackConfig, SkillManifest};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn normalize_workspace_skill_dir_name_uses_skill_id_and_sanitizes() {
        assert_eq!(
            normalize_workspace_skill_dir_name(" Local Skill/Auto Redbook "),
            "local-skill-auto-redbook"
        );
        assert_eq!(
            normalize_workspace_skill_dir_name("builtin.general"),
            "builtin-general"
        );
        assert_eq!(normalize_workspace_skill_dir_name("___"), "skill");
    }

    #[test]
    fn build_workspace_skill_markdown_path_uses_projected_skill_dir() {
        let path = build_workspace_skill_markdown_path(
            Path::new("E:\\workspace\\session-a"),
            "Local Skill/Auto Redbook",
        );
        assert_eq!(
            path,
            Path::new("E:\\workspace\\session-a")
                .join("skills")
                .join("local-skill-auto-redbook")
                .join("SKILL.md")
        );
    }

    #[test]
    fn build_skill_roots_include_projected_workspace_skills_directory() {
        let work_dir = Path::new("E:\\workspace\\session-a");
        let roots = build_skill_roots(&work_dir.to_string_lossy(), "builtin", "");

        assert!(roots.contains(&work_dir.join(".claude").join("skills")));
        assert!(roots.contains(&work_dir.join("skills")));
    }

    #[test]
    fn build_skill_roots_keep_workspace_roots_first() {
        let work_dir = Path::new("E:\\workspace\\session-a");
        let roots = build_skill_roots(&work_dir.to_string_lossy(), "builtin", "");

        assert_eq!(roots.first(), Some(&work_dir.join(".claude").join("skills")));
        assert_eq!(roots.get(1), Some(&work_dir.join("skills")));
    }

    #[test]
    fn build_workspace_skill_prompt_entry_includes_location() {
        let entry = WorkspaceSkillPromptEntry {
            skill_id: "local-auto-redbook".to_string(),
            invoke_name: "local-auto-redbook".to_string(),
            name: "xhs-note-creator".to_string(),
            description: "Create Xiaohongshu content".to_string(),
            skill_md_path: "E:\\workspace\\skills\\local-auto-redbook\\SKILL.md".to_string(),
        };

        let prompt = build_workspace_skill_prompt_entry(&entry);
        assert!(prompt.contains("<name>xhs-note-creator</name>"));
        assert!(prompt.contains("<invoke_name>local-auto-redbook</invoke_name>"));
        assert!(prompt.contains("<description>Create Xiaohongshu content</description>"));
        assert!(prompt
            .contains("<location>E:\\workspace\\skills\\local-auto-redbook\\SKILL.md</location>"));
    }

    #[test]
    fn build_workspace_skills_prompt_wraps_available_skills_block() {
        let prompt = build_workspace_skills_prompt(&[WorkspaceSkillPromptEntry {
            skill_id: "builtin-general".to_string(),
            invoke_name: "builtin-general".to_string(),
            name: "General Assistant".to_string(),
            description: "Generic work".to_string(),
            skill_md_path: "E:\\workspace\\skills\\builtin-general\\SKILL.md".to_string(),
        }]);

        assert!(prompt.starts_with("<available_skills>"));
        assert!(prompt
            .contains("<location>E:\\workspace\\skills\\builtin-general\\SKILL.md</location>"));
        assert!(prompt.ends_with("</available_skills>"));
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_for_local_skill_uses_local_dir() {
        let tmp = tempdir().unwrap();
        let skill_dir = tmp.path().join("auto-redbook");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Local Skill").unwrap();

        let manifest = SkillManifest {
            id: "local-auto-redbook".to_string(),
            name: "xhs-note-creator".to_string(),
            description: "Create Xiaohongshu notes".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "local-auto-redbook",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            &skill_dir.to_string_lossy(),
            "local",
        )
        .unwrap();

        assert_eq!(entry.projected_dir_name, "auto-redbook");
        match entry.content {
            WorkspaceSkillContent::LocalDir(path) => {
                assert_eq!(path, skill_dir);
            }
            WorkspaceSkillContent::FileTree(_) => panic!("expected local dir content"),
        }
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_preserves_source_directory_basename_for_local_skill() {
        let tmp = tempdir().unwrap();
        let source_dir = tmp.path().join("feishu-pm-runtime");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("SKILL.md"), "# Feishu PM Runtime").unwrap();

        let manifest = SkillManifest {
            id: "feishu-pm-runtime-local".to_string(),
            name: "feishu-pm-runtime".to_string(),
            description: "Runtime helpers".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "feishu-pm-runtime-local",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            &source_dir.to_string_lossy(),
            "local",
        )
        .unwrap();

        assert_eq!(entry.projected_dir_name, "feishu-pm-runtime");
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_for_builtin_skill_creates_skill_md_file_tree() {
        let manifest = SkillManifest {
            id: "builtin-general".to_string(),
            name: "通用助手".to_string(),
            description: "Generic assistant".to_string(),
            version: "builtin".to_string(),
            author: "WorkClaw".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "builtin-general",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            "",
            "builtin",
        )
        .unwrap();

        match entry.content {
            WorkspaceSkillContent::FileTree(files) => {
                let skill_md = files
                    .get("SKILL.md")
                    .expect("builtin SKILL.md should exist");
                let text = String::from_utf8(skill_md.clone()).unwrap();
                assert!(text.contains("通用助手") || text.contains("通用任务智能体"));
            }
            WorkspaceSkillContent::LocalDir(_) => panic!("expected builtin file tree content"),
        }
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_for_builtin_docx_includes_vendored_assets() {
        let manifest = SkillManifest {
            id: "builtin-docx".to_string(),
            name: "DOCX 文档助手".to_string(),
            description: "Professional docx workflow".to_string(),
            version: "builtin".to_string(),
            author: "WorkClaw".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "builtin-docx",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            "",
            "builtin",
        )
        .unwrap();

        match entry.content {
            WorkspaceSkillContent::FileTree(files) => {
                assert!(files.contains_key("SKILL.md"));
                assert!(files.contains_key("scripts/setup.ps1"));
                assert!(files.contains_key("assets/xsd/business-rules.xsd"));
            }
            WorkspaceSkillContent::LocalDir(_) => panic!("expected builtin file tree content"),
        }
    }

    #[test]
    fn sync_workspace_skills_to_directory_keeps_sibling_local_skills_addressable_after_projection() {
        let tmp = tempdir().unwrap();
        let source_root = tmp.path().join("source");
        let runtime_dir = source_root.join("feishu-pm-runtime");
        let summary_dir = source_root.join("feishu-pm-weekly-work-summary");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        std::fs::create_dir_all(&summary_dir).unwrap();
        std::fs::write(runtime_dir.join("SKILL.md"), "# Feishu PM Runtime").unwrap();
        std::fs::write(summary_dir.join("SKILL.md"), "# Weekly Summary").unwrap();
        std::fs::write(summary_dir.join("reference.txt"), "../feishu-pm-runtime").unwrap();

        let manifest_runtime = SkillManifest {
            id: "feishu-pm-runtime-local".to_string(),
            name: "feishu-pm-runtime".to_string(),
            description: "Runtime helpers".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };
        let manifest_summary = SkillManifest {
            id: "feishu-pm-weekly-work-summary-local".to_string(),
            name: "feishu-pm-weekly-work-summary".to_string(),
            description: "Weekly summaries".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let runtime_entry = resolve_workspace_skill_runtime_entry(
            "feishu-pm-runtime-local",
            &serde_json::to_string(&manifest_runtime).unwrap(),
            "",
            &runtime_dir.to_string_lossy(),
            "local",
        )
        .unwrap();
        let summary_entry = resolve_workspace_skill_runtime_entry(
            "feishu-pm-weekly-work-summary-local",
            &serde_json::to_string(&manifest_summary).unwrap(),
            "",
            &summary_dir.to_string_lossy(),
            "local",
        )
        .unwrap();

        let work_dir = tmp.path().join("workspace");
        sync_workspace_skills_to_directory(&work_dir, &[runtime_entry, summary_entry]).unwrap();

        let projected_runtime = work_dir.join("skills").join("feishu-pm-runtime");
        let projected_summary = work_dir.join("skills").join("feishu-pm-weekly-work-summary");
        assert!(projected_runtime.join("SKILL.md").exists());
        assert!(projected_summary.join("SKILL.md").exists());
        assert!(projected_summary
            .join("..")
            .join("feishu-pm-runtime")
            .join("SKILL.md")
            .exists());
    }

    #[test]
    fn build_workspace_skill_prompt_entries_use_projected_skill_paths_for_local_skills() {
        let tmp = tempdir().unwrap();
        let source_dir = tmp.path().join("feishu-pm-runtime");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("SKILL.md"), "# Feishu PM Runtime").unwrap();

        let manifest = SkillManifest {
            id: "feishu-pm-runtime-local".to_string(),
            name: "feishu-pm-runtime".to_string(),
            description: "Runtime helpers".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "feishu-pm-runtime-local",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            &source_dir.to_string_lossy(),
            "local",
        )
        .unwrap();

        let prompt_entries = build_workspace_skill_prompt_entries(tmp.path(), &[entry]);
        assert_eq!(prompt_entries.len(), 1);
        assert_eq!(
            prompt_entries[0].skill_md_path,
            tmp.path()
                .join("skills")
                .join("feishu-pm-runtime")
                .join("SKILL.md")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_for_vendored_skill_uses_local_dir() {
        let tmp = tempdir().unwrap();
        let skill_dir = tmp.path().join("vendored-docx");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: vendored-docx
description: Vendored office skill
---
Use vendored assets.
"#,
        )
        .unwrap();
        std::fs::write(skill_dir.join("scripts").join("setup.ps1"), "Write-Host 'ok'").unwrap();

        let manifest = SkillManifest {
            id: "builtin-docx".to_string(),
            name: "manifest-name".to_string(),
            description: "manifest-description".to_string(),
            version: "vendored".to_string(),
            author: "WorkClaw".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "builtin-docx",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            &skill_dir.to_string_lossy(),
            "vendored",
        )
        .unwrap();

        assert_eq!(entry.name, "vendored-docx");
        assert_eq!(entry.description, "Vendored office skill");
        match entry.content {
            WorkspaceSkillContent::LocalDir(path) => assert_eq!(path, skill_dir),
            WorkspaceSkillContent::FileTree(_) => panic!("expected vendored local dir content"),
        }
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_for_legacy_builtin_with_pack_path_prefers_local_dir() {
        let tmp = tempdir().unwrap();
        let skill_dir = tmp.path().join("legacy-builtin-docx");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: migrated-builtin-docx
description: Directory-backed legacy builtin
---
Prefer directory content.
"#,
        )
        .unwrap();
        std::fs::write(skill_dir.join("scripts").join("setup.ps1"), "Write-Host 'legacy'").unwrap();

        let manifest = SkillManifest {
            id: "builtin-docx".to_string(),
            name: "manifest-name".to_string(),
            description: "manifest-description".to_string(),
            version: "builtin".to_string(),
            author: "WorkClaw".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "builtin-docx",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            &skill_dir.to_string_lossy(),
            "builtin",
        )
        .unwrap();

        assert_eq!(entry.name, "migrated-builtin-docx");
        assert_eq!(entry.description, "Directory-backed legacy builtin");
        match entry.content {
            WorkspaceSkillContent::LocalDir(path) => assert_eq!(path, skill_dir),
            WorkspaceSkillContent::FileTree(_) => {
                panic!("expected legacy builtin with pack_path to prefer local dir")
            }
        }
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_preserves_openclaw_frontmatter_fields() {
        let tmp = tempdir().unwrap();
        let skill_dir = tmp.path().join("skill-src");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: dispatched-skill
description: Routed through exec
user-invocable: true
disable-model-invocation: true
command-dispatch: tool
command-tool: exec
command-arg-mode: raw
metadata:
  {
    "openclaw":
      {
        "primaryEnv": "OPENAI_API_KEY",
        "requires": { "env": ["OPENAI_API_KEY"] },
      },
  }
---
Run exec directly.
"#,
        )
        .unwrap();

        let manifest = SkillManifest {
            id: "dispatched-skill".to_string(),
            name: "manifest-name".to_string(),
            description: "manifest-description".to_string(),
            version: "local".to_string(),
            author: "tester".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        let entry = resolve_workspace_skill_runtime_entry(
            "dispatched-skill",
            &serde_json::to_string(&manifest).unwrap(),
            "",
            &skill_dir.to_string_lossy(),
            "local",
        )
        .unwrap();

        assert_eq!(entry.name, "dispatched-skill");
        assert_eq!(entry.description, "Routed through exec");
        assert!(entry.invocation.user_invocable);
        assert!(entry.invocation.disable_model_invocation);
        assert_eq!(
            entry.command_dispatch.as_ref().map(|dispatch| dispatch.tool_name.as_str()),
            Some("exec")
        );
        assert_eq!(
            entry.metadata
                .as_ref()
                .and_then(|metadata| metadata.primary_env.as_deref()),
            Some("OPENAI_API_KEY")
        );
    }

    #[test]
    fn resolve_workspace_skill_runtime_entry_for_encrypted_skill_uses_unpacked_files() {
        let tmp = tempdir().unwrap();
        let skill_dir = tmp.path().join("skill-src");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: encrypted-skill\ndescription: Encrypted skill\n---\n\n# Skill\nHello",
        )
        .unwrap();
        std::fs::write(skill_dir.join("scripts").join("hello.py"), "print('hello')").unwrap();

        let output = tmp.path().join("encrypted.skillpack");
        pack(&PackConfig {
            dir_path: skill_dir.to_string_lossy().to_string(),
            name: "encrypted-skill".to_string(),
            description: "Encrypted skill".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            username: "alice".to_string(),
            recommended_model: "gpt-4o".to_string(),
            output_path: output.to_string_lossy().to_string(),
        })
        .unwrap();

        let unpacked = skillpack_rs::verify_and_unpack(&output.to_string_lossy(), "alice").unwrap();
        let entry = resolve_workspace_skill_runtime_entry(
            &unpacked.manifest.id,
            &serde_json::to_string(&unpacked.manifest).unwrap(),
            "alice",
            &output.to_string_lossy(),
            "encrypted",
        )
        .unwrap();

        match entry.content {
            WorkspaceSkillContent::FileTree(files) => {
                assert!(files.contains_key("SKILL.md"));
                assert!(files.contains_key("scripts/hello.py"));
            }
            WorkspaceSkillContent::LocalDir(_) => panic!("expected encrypted file tree content"),
        }
    }

    #[test]
    fn sync_workspace_skills_to_directory_copies_local_skill_tree() {
        let tmp = tempdir().unwrap();
        let source_dir = tmp.path().join("local-skill");
        std::fs::create_dir_all(source_dir.join("scripts")).unwrap();
        std::fs::write(source_dir.join("SKILL.md"), "# Skill").unwrap();
        std::fs::write(source_dir.join("scripts").join("hello.py"), "print('hi')").unwrap();

        let work_dir = tmp.path().join("workspace");
        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "local-skill".to_string(),
            name: "Local Skill".to_string(),
            description: "Local".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "local-skill".to_string(),
            config: SkillConfig::default(),
            invocation: runtime_skill_core::SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::LocalDir(source_dir),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        assert!(work_dir
            .join("skills")
            .join("local-skill")
            .join("SKILL.md")
            .exists());
        assert!(work_dir
            .join("skills")
            .join("local-skill")
            .join("scripts")
            .join("hello.py")
            .exists());
    }

    #[test]
    fn sync_workspace_skills_to_directory_skips_git_metadata_for_local_skill() {
        let tmp = tempdir().unwrap();
        let source_dir = tmp.path().join("local-skill");
        std::fs::create_dir_all(source_dir.join(".git").join("objects")).unwrap();
        std::fs::create_dir_all(source_dir.join("scripts")).unwrap();
        std::fs::write(source_dir.join("SKILL.md"), "# Skill").unwrap();
        std::fs::write(source_dir.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();
        std::fs::write(source_dir.join("scripts").join("hello.py"), "print('hi')").unwrap();

        let work_dir = tmp.path().join("workspace");
        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "local-skill".to_string(),
            name: "Local Skill".to_string(),
            description: "Local".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "local-skill".to_string(),
            config: SkillConfig::default(),
            invocation: runtime_skill_core::SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::LocalDir(source_dir),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        let projected = work_dir.join("skills").join("local-skill");
        assert!(projected.join("SKILL.md").exists());
        assert!(projected.join("scripts").join("hello.py").exists());
        assert!(!projected.join(".git").exists());
    }

    #[test]
    fn sync_workspace_skills_to_directory_writes_file_tree_entries() {
        let tmp = tempdir().unwrap();
        let work_dir = tmp.path().join("workspace");
        let mut files = std::collections::HashMap::new();
        files.insert("SKILL.md".to_string(), b"# Builtin".to_vec());
        files.insert("assets/template.txt".to_string(), b"hello".to_vec());

        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "builtin-general".to_string(),
            name: "Builtin".to_string(),
            description: "Builtin".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "builtin-general".to_string(),
            config: SkillConfig::default(),
            invocation: runtime_skill_core::SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(files),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        assert!(work_dir
            .join("skills")
            .join("builtin-general")
            .join("SKILL.md")
            .exists());
        assert!(work_dir
            .join("skills")
            .join("builtin-general")
            .join("assets")
            .join("template.txt")
            .exists());
    }

    #[test]
    fn sync_workspace_skills_to_directory_writes_skill_id_marker() {
        let tmp = tempdir().unwrap();
        let work_dir = tmp.path().join("workspace");
        let mut files = std::collections::HashMap::new();
        files.insert("SKILL.md".to_string(), b"# Builtin".to_vec());

        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "local-feishu-pm-runtime".to_string(),
            name: "Builtin".to_string(),
            description: "Builtin".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "feishu-pm-runtime".to_string(),
            config: SkillConfig::default(),
            invocation: runtime_skill_core::SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(files),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        let marker = work_dir
            .join("skills")
            .join("feishu-pm-runtime")
            .join(".workclaw-skill-id");
        assert_eq!(
            std::fs::read_to_string(marker).unwrap(),
            "local-feishu-pm-runtime"
        );
    }

    #[test]
    fn sync_workspace_skills_to_directory_projects_builtin_docx_assets() {
        let tmp = tempdir().unwrap();
        let work_dir = tmp.path().join("workspace");
        let files = runtime_skill_core::builtin_skill_files("builtin-docx")
            .expect("builtin docx assets should be embedded");

        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "builtin-docx".to_string(),
            name: "DOCX 文档助手".to_string(),
            description: "Professional docx workflow".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "builtin-docx".to_string(),
            config: SkillConfig::default(),
            invocation: runtime_skill_core::SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(files),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        let projected = work_dir.join("skills").join("builtin-docx");
        assert!(projected.join("SKILL.md").exists());
        assert!(projected.join("scripts").join("setup.ps1").exists());
        assert!(projected
            .join("assets")
            .join("xsd")
            .join("business-rules.xsd")
            .exists());
    }

    #[test]
    fn sync_workspace_skills_to_directory_rebuilds_skills_root() {
        let tmp = tempdir().unwrap();
        let work_dir = tmp.path().join("workspace");
        let stale_dir = work_dir.join("skills").join("stale-skill");
        std::fs::create_dir_all(&stale_dir).unwrap();
        std::fs::write(stale_dir.join("old.txt"), "stale").unwrap();

        let mut files = std::collections::HashMap::new();
        files.insert("SKILL.md".to_string(), b"# Fresh".to_vec());
        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "fresh-skill".to_string(),
            name: "Fresh".to_string(),
            description: "Fresh".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "fresh-skill".to_string(),
            config: SkillConfig::default(),
            invocation: runtime_skill_core::SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(files),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        assert!(!stale_dir.exists());
        assert!(work_dir
            .join("skills")
            .join("fresh-skill")
            .join("SKILL.md")
            .exists());
    }

    #[test]
    fn build_workspace_skill_prompt_entries_use_projected_skill_paths() {
        let work_dir = Path::new("E:\\workspace\\session");
        let entries = vec![WorkspaceSkillRuntimeEntry {
            skill_id: "builtin-general".to_string(),
            name: "General".to_string(),
            description: "Generic".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "builtin-general".to_string(),
            config: SkillConfig::default(),
            invocation: runtime_skill_core::SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
        }];

        let prompt_entries = build_workspace_skill_prompt_entries(work_dir, &entries);
        assert_eq!(prompt_entries.len(), 1);
        assert_eq!(
            prompt_entries[0].skill_md_path,
            "E:\\workspace\\session\\skills\\builtin-general\\SKILL.md"
        );
    }

    #[test]
    fn build_workspace_skill_prompt_entries_skip_disable_model_invocation_skills() {
        let work_dir = Path::new("E:\\workspace\\session");
        let entries = vec![
            WorkspaceSkillRuntimeEntry {
                skill_id: "visible-skill".to_string(),
                name: "Visible".to_string(),
                description: "Visible skill".to_string(),
                source_type: "builtin".to_string(),
                projected_dir_name: "visible-skill".to_string(),
                config: SkillConfig::default(),
                invocation: runtime_skill_core::SkillInvocationPolicy::default(),
                metadata: None,
                command_dispatch: None,
                content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
            },
            WorkspaceSkillRuntimeEntry {
                skill_id: "hidden-skill".to_string(),
                name: "Hidden".to_string(),
                description: "Hidden skill".to_string(),
                source_type: "builtin".to_string(),
                projected_dir_name: "hidden-skill".to_string(),
                config: SkillConfig::default(),
                invocation: runtime_skill_core::SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: true,
                },
                metadata: None,
                command_dispatch: None,
                content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
            },
        ];

        let prompt_entries = build_workspace_skill_prompt_entries(work_dir, &entries);
        assert_eq!(prompt_entries.len(), 1);
        assert_eq!(prompt_entries[0].skill_id, "visible-skill");
    }

    #[test]
    fn build_workspace_skill_command_specs_sanitizes_and_deduplicates_names() {
        let entries = vec![
            WorkspaceSkillRuntimeEntry {
                skill_id: "skill-a".to_string(),
                name: "PM Summary".to_string(),
                description: "Summarize PM updates".to_string(),
                source_type: "local".to_string(),
                projected_dir_name: "skill-a".to_string(),
                config: SkillConfig::default(),
                invocation: runtime_skill_core::SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
                metadata: None,
                command_dispatch: Some(runtime_skill_core::SkillCommandDispatchSpec {
                    kind: runtime_skill_core::SkillCommandDispatchKind::Tool,
                    tool_name: "exec".to_string(),
                    arg_mode: runtime_skill_core::SkillCommandArgMode::Raw,
                }),
                content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
            },
            WorkspaceSkillRuntimeEntry {
                skill_id: "skill-b".to_string(),
                name: "pm-summary".to_string(),
                description: "Second summary".to_string(),
                source_type: "local".to_string(),
                projected_dir_name: "skill-b".to_string(),
                config: SkillConfig::default(),
                invocation: runtime_skill_core::SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: false,
                },
                metadata: None,
                command_dispatch: None,
                content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
            },
            WorkspaceSkillRuntimeEntry {
                skill_id: "skill-hidden".to_string(),
                name: "Hidden".to_string(),
                description: "Not directly invocable".to_string(),
                source_type: "local".to_string(),
                projected_dir_name: "skill-hidden".to_string(),
                config: SkillConfig::default(),
                invocation: runtime_skill_core::SkillInvocationPolicy {
                    user_invocable: false,
                    disable_model_invocation: true,
                },
                metadata: None,
                command_dispatch: None,
                content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
            },
        ];

        let specs = build_workspace_skill_command_specs(&entries);
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].name, "pm_summary");
        assert_eq!(specs[1].name, "pm_summary_2");
        assert_eq!(
            specs[0]
                .dispatch
                .as_ref()
                .map(|dispatch| dispatch.tool_name.as_str()),
            Some("exec")
        );
    }

    #[test]
    fn build_workspace_skill_command_specs_skips_hidden_non_dispatch_skills() {
        let entries = vec![
            WorkspaceSkillRuntimeEntry {
                skill_id: "hidden-prompt".to_string(),
                name: "Hidden Prompt".to_string(),
                description: "Hidden from model".to_string(),
                source_type: "local".to_string(),
                projected_dir_name: "hidden-prompt".to_string(),
                config: SkillConfig::default(),
                invocation: runtime_skill_core::SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: true,
                },
                metadata: None,
                command_dispatch: None,
                content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
            },
            WorkspaceSkillRuntimeEntry {
                skill_id: "hidden-dispatch".to_string(),
                name: "Hidden Dispatch".to_string(),
                description: "Hidden but dispatchable".to_string(),
                source_type: "local".to_string(),
                projected_dir_name: "hidden-dispatch".to_string(),
                config: SkillConfig::default(),
                invocation: runtime_skill_core::SkillInvocationPolicy {
                    user_invocable: true,
                    disable_model_invocation: true,
                },
                metadata: None,
                command_dispatch: Some(runtime_skill_core::SkillCommandDispatchSpec {
                    kind: runtime_skill_core::SkillCommandDispatchKind::Tool,
                    tool_name: "exec".to_string(),
                    arg_mode: runtime_skill_core::SkillCommandArgMode::Raw,
                }),
                content: WorkspaceSkillContent::FileTree(std::collections::HashMap::new()),
            },
        ];

        let specs = build_workspace_skill_command_specs(&entries);

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].skill_name, "Hidden Dispatch");
        assert_eq!(specs[0].name, "hidden_dispatch");
    }

    #[test]
    fn prepare_workspace_skills_prompt_syncs_and_returns_available_skills_block() {
        let tmp = tempdir().unwrap();
        let work_dir = tmp.path().join("workspace");
        let mut files = std::collections::HashMap::new();
        files.insert("SKILL.md".to_string(), b"# Fresh".to_vec());
        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "fresh-skill".to_string(),
            name: "Fresh".to_string(),
            description: "Fresh description".to_string(),
            source_type: "builtin".to_string(),
            projected_dir_name: "fresh-skill".to_string(),
            config: SkillConfig::default(),
            invocation: runtime_skill_core::SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::FileTree(files),
        };

        let prompt = prepare_workspace_skills_prompt(&work_dir, &[entry]).unwrap();

        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("<name>Fresh</name>"));
        assert!(prompt.contains("<description>Fresh description</description>"));
        let projected_skill_md = work_dir
            .join("skills")
            .join("fresh-skill")
            .join("SKILL.md")
            .to_string_lossy()
            .to_string();
        assert!(prompt.contains(&projected_skill_md));
        assert!(work_dir
            .join("skills")
            .join("fresh-skill")
            .join("SKILL.md")
            .exists());
    }

    #[test]
    fn prepare_workspace_skills_prompt_deduplicates_colliding_projected_dir_names() {
        let tmp = tempdir().unwrap();
        let work_dir = tmp.path().join("workspace");

        let mut first_files = std::collections::HashMap::new();
        first_files.insert("SKILL.md".to_string(), b"# First".to_vec());
        let mut second_files = std::collections::HashMap::new();
        second_files.insert("SKILL.md".to_string(), b"# Second".to_vec());

        let entries = vec![
            WorkspaceSkillRuntimeEntry {
                skill_id: "local-first-runtime".to_string(),
                name: "First Runtime".to_string(),
                description: "First runtime".to_string(),
                source_type: "local".to_string(),
                projected_dir_name: "feishu-pm-runtime".to_string(),
                config: SkillConfig::default(),
                invocation: runtime_skill_core::SkillInvocationPolicy::default(),
                metadata: None,
                command_dispatch: None,
                content: WorkspaceSkillContent::FileTree(first_files),
            },
            WorkspaceSkillRuntimeEntry {
                skill_id: "local-second-runtime".to_string(),
                name: "Second Runtime".to_string(),
                description: "Second runtime".to_string(),
                source_type: "local".to_string(),
                projected_dir_name: "feishu-pm-runtime".to_string(),
                config: SkillConfig::default(),
                invocation: runtime_skill_core::SkillInvocationPolicy::default(),
                metadata: None,
                command_dispatch: None,
                content: WorkspaceSkillContent::FileTree(second_files),
            },
        ];

        let prompt = prepare_workspace_skills_prompt(&work_dir, &entries).unwrap();

        assert!(work_dir
            .join("skills")
            .join("feishu-pm-runtime")
            .join("SKILL.md")
            .exists());
        assert!(work_dir
            .join("skills")
            .join("feishu-pm-runtime-2")
            .join("SKILL.md")
            .exists());
        assert!(prompt.contains("feishu-pm-runtime\\SKILL.md"));
        assert!(prompt.contains("feishu-pm-runtime-2\\SKILL.md"));
    }

    #[tokio::test]
    async fn load_workspace_skill_runtime_entries_with_pool_reads_local_and_vendored_skills() {
        let tmp = tempdir().unwrap();
        let db_path = tmp.path().join("skills.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE installed_skills (
                id TEXT PRIMARY KEY,
                manifest TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                last_used_at TEXT,
                username TEXT NOT NULL,
                pack_path TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'encrypted'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        let local_skill_dir = tmp.path().join("local-skill");
        std::fs::create_dir_all(local_skill_dir.join("scripts")).unwrap();
        std::fs::write(local_skill_dir.join("SKILL.md"), "# Local Skill").unwrap();
        std::fs::write(
            local_skill_dir.join("scripts").join("hello.py"),
            "print('hi')",
        )
        .unwrap();

        let local_manifest = SkillManifest {
            id: "local-auto-redbook".to_string(),
            name: "xhs-note-creator".to_string(),
            description: "Create Xiaohongshu notes".to_string(),
            version: "local".to_string(),
            author: "tester".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };
        let vendored_manifest = SkillManifest {
            id: "builtin-general".to_string(),
            name: "通用助手".to_string(),
            description: "Generic assistant".to_string(),
            version: "vendored".to_string(),
            author: "WorkClaw".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("local-auto-redbook")
        .bind(serde_json::to_string(&local_manifest).unwrap())
        .bind(Utc::now().to_rfc3339())
        .bind("")
        .bind(local_skill_dir.to_string_lossy().to_string())
        .bind("local")
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("builtin-general")
        .bind(serde_json::to_string(&vendored_manifest).unwrap())
        .bind(Utc::now().to_rfc3339())
        .bind("")
        .bind(local_skill_dir.to_string_lossy().to_string())
        .bind("vendored")
        .execute(&pool)
        .await
        .unwrap();

        let entries = super::load_workspace_skill_runtime_entries_with_pool(&pool)
            .await
            .unwrap();

        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| {
            entry.skill_id == "local-auto-redbook"
                && matches!(entry.content, WorkspaceSkillContent::LocalDir(_))
        }));
        assert!(entries.iter().any(|entry| {
            entry.skill_id == "builtin-general"
                && matches!(entry.content, WorkspaceSkillContent::LocalDir(_))
        }));
    }

    #[tokio::test]
    async fn load_workspace_skill_runtime_entries_with_pool_keeps_legacy_builtin_fallback() {
        let tmp = tempdir().unwrap();
        let db_path = tmp.path().join("skills.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE installed_skills (
                id TEXT PRIMARY KEY,
                manifest TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                last_used_at TEXT,
                username TEXT NOT NULL,
                pack_path TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'encrypted'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        let builtin_manifest = SkillManifest {
            id: "builtin-general".to_string(),
            name: "通用助手".to_string(),
            description: "Generic assistant".to_string(),
            version: "builtin".to_string(),
            author: "WorkClaw".to_string(),
            recommended_model: "gpt-4o".to_string(),
            tags: vec![],
            created_at: Utc::now(),
            username_hint: None,
            encrypted_verify: String::new(),
        };

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("builtin-general")
        .bind(serde_json::to_string(&builtin_manifest).unwrap())
        .bind(Utc::now().to_rfc3339())
        .bind("")
        .bind("")
        .bind("builtin")
        .execute(&pool)
        .await
        .unwrap();

        let entries = super::load_workspace_skill_runtime_entries_with_pool(&pool)
            .await
            .unwrap();

        assert_eq!(entries.len(), 1);
        assert!(entries.iter().any(|entry| {
            entry.skill_id == "builtin-general"
                && matches!(entry.content, WorkspaceSkillContent::FileTree(_))
        }));
    }

    #[test]
    fn sync_workspace_skills_to_directory_preserves_auto_redbook_style_layout() {
        let tmp = tempdir().unwrap();
        let source_dir = tmp.path().join("auto-redbook-skill");
        std::fs::create_dir_all(source_dir.join("scripts")).unwrap();
        std::fs::create_dir_all(source_dir.join("assets")).unwrap();
        std::fs::create_dir_all(source_dir.join("references")).unwrap();
        std::fs::write(source_dir.join("SKILL.md"), "# Auto Redbook").unwrap();
        std::fs::write(
            source_dir.join("scripts").join("publish_xhs.py"),
            "print('publish')",
        )
        .unwrap();
        std::fs::write(
            source_dir.join("assets").join("cover.html"),
            "<html></html>",
        )
        .unwrap();
        std::fs::write(source_dir.join("references").join("params.md"), "# params").unwrap();

        let work_dir = tmp.path().join("workspace");
        let entry = WorkspaceSkillRuntimeEntry {
            skill_id: "local-auto-redbook".to_string(),
            name: "xhs-note-creator".to_string(),
            description: "Create Xiaohongshu notes".to_string(),
            source_type: "local".to_string(),
            projected_dir_name: "local-auto-redbook".to_string(),
            config: SkillConfig::default(),
            invocation: runtime_skill_core::SkillInvocationPolicy::default(),
            metadata: None,
            command_dispatch: None,
            content: WorkspaceSkillContent::LocalDir(source_dir),
        };

        sync_workspace_skills_to_directory(&work_dir, &[entry]).unwrap();

        let projected = work_dir.join("skills").join("local-auto-redbook");
        assert!(projected.join("SKILL.md").exists());
        assert!(projected.join("scripts").join("publish_xhs.py").exists());
        assert!(projected.join("assets").join("cover.html").exists());
        assert!(projected.join("references").join("params.md").exists());
    }

    #[test]
    fn extract_assistant_text_content_prefers_text_field() {
        let content =
            r#"{"text":"最终答案","reasoning":{"status":"completed","content":"内部思考"}}"#;
        assert_eq!(extract_assistant_text_content(content), "最终答案");
    }

    #[test]
    fn extract_assistant_text_content_falls_back_for_plain_text() {
        assert_eq!(extract_assistant_text_content("普通文本"), "普通文本");
    }
}
