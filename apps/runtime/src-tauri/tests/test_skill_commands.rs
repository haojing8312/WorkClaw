mod helpers;

use runtime_lib::commands::skills::{create_local_skill, render_local_skill_preview};
use std::path::Path;

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
