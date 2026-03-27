mod helpers;

use chrono::Utc;
use runtime_lib::agent::runtime::runtime_io::{
    build_workspace_skill_command_specs, load_workspace_skill_runtime_entries_with_pool,
};
use runtime_lib::commands::skills::{
    create_local_skill, import_local_skills_to_pool, render_local_skill_preview,
};
use std::path::Path;

fn write_skill(dir: &Path, name: &str, body: &str) {
    std::fs::create_dir_all(dir).expect("create skill dir");
    let skill_md = format!("---\nname: {name}\ndescription: test\n---\n\n{body}");
    std::fs::write(dir.join("SKILL.md"), skill_md).expect("write skill");
}

#[tokio::test]
async fn create_local_skill_rejects_existing_directory() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let base = tmp.path().to_string_lossy().to_string();

    let first = create_local_skill(
        "File Organizer".to_string(),
        "organize files".to_string(),
        "需要整理大量文件时".to_string(),
        Some(base.clone()),
    )
    .await
    .expect("first create should succeed");

    assert!(Path::new(&first).join("SKILL.md").exists());

    let err = create_local_skill(
        "File Organizer".to_string(),
        "organize files".to_string(),
        "需要整理大量文件时".to_string(),
        Some(base),
    )
    .await
    .expect_err("second create should fail due to conflict");

    assert!(
        err.contains("技能目录已存在"),
        "expected conflict error, got: {}",
        err
    );
}

#[tokio::test]
async fn render_local_skill_preview_uses_template_and_returns_save_path() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let base = tmp.path().to_string_lossy().to_string();

    let preview = render_local_skill_preview(
        "My Skill".to_string(),
        "用于整理文件".to_string(),
        "需要整理大量文件时".to_string(),
        Some(base),
    )
    .await
    .expect("render preview");

    assert!(preview.markdown.contains("name: My Skill"));
    assert!(preview
        .markdown
        .contains("description: Use when 用于整理文件"));
    assert!(preview.markdown.contains("## Workflow"));
    assert!(preview.markdown.contains("## Quality Checklist"));

    let save_path = Path::new(&preview.save_path);
    assert!(save_path.ends_with("my-skill"));
}

#[tokio::test]
async fn render_local_skill_preview_has_default_values_for_empty_input() {
    let preview = render_local_skill_preview("".to_string(), "".to_string(), "".to_string(), None)
        .await
        .expect("render preview with defaults");

    assert!(preview.markdown.contains("name: expert-skill"));
    assert!(preview
        .markdown
        .contains("Use when 需要在特定任务场景中提供稳定执行能力"));
    assert!(preview.save_path.contains(".workclaw"));
    assert!(Path::new(&preview.save_path).ends_with("expert-skill"));
}

#[tokio::test]
async fn import_local_skill_rejects_duplicate_display_name() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let dir1 = tempfile::tempdir().expect("create temp dir1");
    let dir2 = tempfile::tempdir().expect("create temp dir2");
    let skill_dir1 = dir1.path().join("skill-a");
    let skill_dir2 = dir2.path().join("skill-b");
    std::fs::create_dir_all(&skill_dir1).expect("create dir1");
    std::fs::create_dir_all(&skill_dir2).expect("create dir2");

    let skill_md = "---\nname: Duplicate Name\ndescription: test\n---\n\ncontent";
    std::fs::write(skill_dir1.join("SKILL.md"), skill_md).expect("write skill1");
    std::fs::write(skill_dir2.join("SKILL.md"), skill_md).expect("write skill2");

    runtime_lib::commands::skills::import_local_skill_to_pool(
        skill_dir1.to_string_lossy().to_string(),
        &pool,
        &[],
    )
    .await
    .expect("first import should pass");

    let err = runtime_lib::commands::skills::import_local_skill_to_pool(
        skill_dir2.to_string_lossy().to_string(),
        &pool,
        &[],
    )
    .await
    .err()
    .expect("duplicate display name should fail");

    assert!(
        err.contains("DUPLICATE_SKILL_NAME:Duplicate Name"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn import_local_skills_imports_selected_skill_directory() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let dir = tempfile::tempdir().expect("create temp dir");
    let skill_dir = dir.path().join("writer");
    write_skill(&skill_dir, "Writer", "content");

    let result = import_local_skills_to_pool(skill_dir.to_string_lossy().to_string(), &pool, &[])
        .await
        .expect("single directory import should succeed");

    assert_eq!(result.installed.len(), 1);
    assert!(result.failed.is_empty());
    assert_eq!(result.installed[0].manifest.name, "Writer");
}

#[tokio::test]
async fn import_local_skills_imports_multiple_skills_from_root_directory() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let dir = tempfile::tempdir().expect("create temp dir");
    let skills_root = dir.path().join("skills");
    write_skill(&skills_root.join("writer"), "Writer", "content");
    write_skill(&skills_root.join("planner"), "Planner", "content");

    let result = import_local_skills_to_pool(skills_root.to_string_lossy().to_string(), &pool, &[])
        .await
        .expect("root directory import should succeed");

    assert_eq!(result.installed.len(), 2);
    assert!(result.failed.is_empty());
    let names = result
        .installed
        .iter()
        .map(|item| item.manifest.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["Planner".to_string(), "Writer".to_string()]);
}

#[tokio::test]
async fn import_local_skills_discovers_nested_skills_one_level_deep() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let dir = tempfile::tempdir().expect("create temp dir");
    let skills_root = dir.path().join("skills");
    write_skill(
        &skills_root.join("group-a").join("writer"),
        "Writer",
        "content",
    );
    write_skill(
        &skills_root.join("group-b").join("planner"),
        "Planner",
        "content",
    );

    let result = import_local_skills_to_pool(skills_root.to_string_lossy().to_string(), &pool, &[])
        .await
        .expect("nested root directory import should succeed");

    assert_eq!(result.installed.len(), 2);
    assert!(result.failed.is_empty());
}

#[tokio::test]
async fn import_local_skills_ignores_directories_deeper_than_one_extra_level() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let dir = tempfile::tempdir().expect("create temp dir");
    let skills_root = dir.path().join("skills");
    write_skill(
        &skills_root.join("group-a").join("deep").join("writer"),
        "Writer",
        "content",
    );

    let err = import_local_skills_to_pool(skills_root.to_string_lossy().to_string(), &pool, &[])
        .await
        .expect_err("deeper directories should not be discovered");

    assert!(
        err.contains("未找到"),
        "expected not found error for over-deep skill directories, got: {err}"
    );
}

#[tokio::test]
async fn import_local_skills_reports_partial_success_without_blocking_other_skills() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let dir = tempfile::tempdir().expect("create temp dir");
    let skills_root = dir.path().join("skills");
    write_skill(&skills_root.join("writer-a"), "Writer", "content");
    write_skill(&skills_root.join("writer-b"), "Writer", "content");

    let result = import_local_skills_to_pool(skills_root.to_string_lossy().to_string(), &pool, &[])
        .await
        .expect("partial success import should still return success");

    assert_eq!(result.installed.len(), 1);
    assert_eq!(result.failed.len(), 1);
    assert!(
        result.failed[0]
            .error
            .contains("DUPLICATE_SKILL_NAME:Writer"),
        "unexpected failure reason: {}",
        result.failed[0].error
    );
}

#[tokio::test]
async fn imported_openclaw_style_skill_produces_user_invocable_command_spec() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let dir = tempfile::tempdir().expect("create temp dir");
    let skill_dir = dir.path().join("pm-summary");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: PM Summary
description: Summarize PM updates
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

Run the standard PM summary workflow.
"#,
    )
    .expect("write skill");

    let result = import_local_skills_to_pool(skill_dir.to_string_lossy().to_string(), &pool, &[])
        .await
        .expect("import should succeed");
    assert_eq!(result.installed.len(), 1);

    let entries = load_workspace_skill_runtime_entries_with_pool(&pool)
        .await
        .expect("load runtime entries");
    let specs = build_workspace_skill_command_specs(&entries);

    assert!(entries.iter().any(|entry| {
        entry.name == "PM Summary"
            && entry.invocation.user_invocable
            && entry.invocation.disable_model_invocation
            && entry
                .command_dispatch
                .as_ref()
                .map(|dispatch| dispatch.tool_name.as_str())
                == Some("exec")
            && entry
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.primary_env.as_deref())
                == Some("OPENAI_API_KEY")
    }));
    assert!(specs.iter().any(|spec| {
        spec.name == "pm_summary"
            && spec.skill_name == "PM Summary"
            && spec
                .dispatch
                .as_ref()
                .map(|dispatch| dispatch.tool_name.as_str())
                == Some("exec")
    }));

    let manifests = sqlx::query_as::<_, (String,)>("SELECT manifest FROM installed_skills")
        .fetch_all(&pool)
        .await
        .expect("load manifests");
    assert_eq!(manifests.len(), 1);
    let manifest: skillpack_rs::SkillManifest =
        serde_json::from_str(&manifests[0].0).expect("parse manifest json");
    assert_eq!(manifest.name, "PM Summary");
    assert_eq!(manifest.version, "local");
    assert!(manifest.created_at <= Utc::now());
}


#[tokio::test]
async fn imported_prompt_following_openclaw_skill_still_produces_user_command_spec() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let dir = tempfile::tempdir().expect("create temp dir");
    let skill_dir = dir.path().join("pm-review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: PM Review
description: Review PM updates
user-invocable: true
metadata:
  {
    "openclaw":
      {
        "emoji": "📝",
        "requires": { "bins": ["python"] },
      },
  }
---

Read the PM updates and prepare a review summary for the user.
"#,
    )
    .expect("write skill");

    let result = import_local_skills_to_pool(skill_dir.to_string_lossy().to_string(), &pool, &[])
        .await
        .expect("import should succeed");
    assert_eq!(result.installed.len(), 1);

    let entries = load_workspace_skill_runtime_entries_with_pool(&pool)
        .await
        .expect("load runtime entries");
    let specs = build_workspace_skill_command_specs(&entries);

    let entry = entries
        .iter()
        .find(|entry| entry.name == "PM Review")
        .expect("pm review entry");
    assert!(entry.invocation.user_invocable);
    assert!(!entry.invocation.disable_model_invocation);
    assert!(entry.command_dispatch.is_none());
    assert_eq!(
        entry
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.emoji.as_deref()),
        Some("📝")
    );
    assert_eq!(
        entry
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.requires.as_ref())
            .map(|requires| requires.bins.clone())
            .unwrap_or_default(),
        vec!["python".to_string()]
    );

    let spec = specs
        .iter()
        .find(|spec| spec.skill_name == "PM Review")
        .expect("pm review command spec");
    assert_eq!(spec.name, "pm_review");
    assert!(spec.dispatch.is_none());
    assert_eq!(spec.description, "Review PM updates");
}

#[tokio::test]
async fn imported_hidden_non_dispatch_skill_does_not_produce_dead_user_command() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let dir = tempfile::tempdir().expect("create temp dir");
    let skill_dir = dir.path().join("hidden-skill");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: Hidden Prompt Skill
description: Hidden from model but not dispatchable
user-invocable: true
disable-model-invocation: true
---

This skill should not be exposed as a dead slash command.
"#,
    )
    .expect("write skill");

    import_local_skills_to_pool(skill_dir.to_string_lossy().to_string(), &pool, &[])
        .await
        .expect("import should succeed");

    let entries = load_workspace_skill_runtime_entries_with_pool(&pool)
        .await
        .expect("load runtime entries");
    let specs = build_workspace_skill_command_specs(&entries);

    assert!(entries.iter().any(|entry| {
        entry.name == "Hidden Prompt Skill"
            && entry.invocation.user_invocable
            && entry.invocation.disable_model_invocation
            && entry.command_dispatch.is_none()
    }));
    assert!(
        specs.iter().all(|spec| spec.skill_name != "Hidden Prompt Skill"),
        "non-dispatch hidden skills should not produce dead slash commands"
    );
}
