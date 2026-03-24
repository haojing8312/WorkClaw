mod helpers;

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

    let result =
        import_local_skills_to_pool(skills_root.to_string_lossy().to_string(), &pool, &[])
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
    write_skill(&skills_root.join("group-a").join("writer"), "Writer", "content");
    write_skill(&skills_root.join("group-b").join("planner"), "Planner", "content");

    let result =
        import_local_skills_to_pool(skills_root.to_string_lossy().to_string(), &pool, &[])
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

    let result =
        import_local_skills_to_pool(skills_root.to_string_lossy().to_string(), &pool, &[])
            .await
            .expect("partial success import should still return success");

    assert_eq!(result.installed.len(), 1);
    assert_eq!(result.failed.len(), 1);
    assert!(
        result.failed[0].error.contains("DUPLICATE_SKILL_NAME:Writer"),
        "unexpected failure reason: {}",
        result.failed[0].error
    );
}
