use runtime_lib::commands::packaging::{
    pack_industry_bundle, pack_skill, read_industry_bundle_manifest, read_skill_dir,
    scan_workclaw_dirs, unpack_industry_bundle, update_skill_dir_tags,
};

#[tokio::test]
async fn read_skill_dir_requires_skill_md() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let err = read_skill_dir(tmp.path().to_string_lossy().to_string())
        .await
        .expect_err("should fail without SKILL.md");
    assert!(err.contains("SKILL.md"));
}

#[tokio::test]
async fn read_skill_dir_returns_frontmatter_and_files() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let skill_md = tmp.path().join("SKILL.md");
    std::fs::write(
        &skill_md,
        "---\nname: test-skill\ndescription: desc\nversion: 1.0.0\n---\ncontent",
    )
    .expect("write SKILL.md");
    std::fs::write(tmp.path().join("notes.md"), "hello").expect("write notes");

    let info = read_skill_dir(tmp.path().to_string_lossy().to_string())
        .await
        .expect("read skill dir");
    assert!(info.files.iter().any(|f| f == "SKILL.md"));
    assert!(info.files.iter().any(|f| f == "notes.md"));
    assert_eq!(info.front_matter.name.as_deref(), Some("test-skill"));
}

#[tokio::test]
async fn pack_skill_creates_skillpack() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        tmp.path().join("SKILL.md"),
        "---\nname: test-skill\ndescription: desc\nversion: 1.0.0\n---\nDo work",
    )
    .expect("write SKILL.md");
    std::fs::write(tmp.path().join("extra.txt"), "file").expect("write file");

    let output = tmp.path().join("out.skillpack");
    pack_skill(
        tmp.path().to_string_lossy().to_string(),
        "test-skill".to_string(),
        "desc".to_string(),
        "1.0.0".to_string(),
        "author".to_string(),
        "alice".to_string(),
        "gpt-4o".to_string(),
        output.to_string_lossy().to_string(),
    )
    .await
    .expect("pack succeeds");

    assert!(output.exists());
}

#[tokio::test]
async fn scan_workclaw_dirs_reads_skill_tags() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let root = tmp.path().join("skills");
    std::fs::create_dir_all(&root).expect("create root");

    let teacher = root.join("teacher-helper");
    std::fs::create_dir_all(&teacher).expect("create teacher skill");
    std::fs::write(
        teacher.join("SKILL.md"),
        "---\nname: Teacher Helper\ndescription: help teachers\ntags:\n  - 教师\n  - 备课\n---\ncontent",
    )
    .expect("write teacher skill");

    let math = root.join("math-writer");
    std::fs::create_dir_all(&math).expect("create math skill");
    std::fs::write(
        math.join("SKILL.md"),
        "---\nname: Math Writer\ndescription: write math papers\ntags: 教师, 数学\n---\ncontent",
    )
    .expect("write math skill");

    let list = scan_workclaw_dirs(root.to_string_lossy().to_string())
        .await
        .expect("scan skill dirs");

    assert_eq!(list.len(), 2);
    let teacher_row = list
        .iter()
        .find(|row| row.slug == "teacher-helper")
        .expect("teacher row");
    assert!(teacher_row.tags.iter().any(|t| t == "教师"));
    assert!(teacher_row.tags.iter().any(|t| t == "备课"));
}

#[tokio::test]
async fn update_skill_dir_tags_rewrites_frontmatter() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let skill = tmp.path().join("teacher-helper");
    std::fs::create_dir_all(&skill).expect("create skill dir");
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: Teacher Helper\ndescription: helper\n---\nbody",
    )
    .expect("write skill");

    update_skill_dir_tags(
        skill.to_string_lossy().to_string(),
        vec!["教师".to_string(), "课堂".to_string()],
    )
    .await
    .expect("update tags");

    let content = std::fs::read_to_string(skill.join("SKILL.md")).expect("read updated skill");
    assert!(content.contains("tags:"));
    assert!(content.contains("- 教师"));
    assert!(content.contains("- 课堂"));
}

#[tokio::test]
async fn pack_and_unpack_industry_bundle_roundtrip() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let root = tmp.path().join("skills");
    std::fs::create_dir_all(&root).expect("create root");

    let teacher = root.join("teacher-helper");
    std::fs::create_dir_all(&teacher).expect("create teacher skill");
    std::fs::write(
        teacher.join("SKILL.md"),
        "---\nname: Teacher Helper\ndescription: help teachers\nversion: 1.0.0\ntags:\n  - 教师\n---\ncontent",
    )
    .expect("write teacher skill");
    std::fs::write(teacher.join("template.md"), "template").expect("write template");

    let grader = root.join("grader");
    std::fs::create_dir_all(&grader).expect("create grader skill");
    std::fs::write(
        grader.join("SKILL.md"),
        "---\nname: Auto Grader\ndescription: grade homework\nversion: 1.0.0\ntags:\n  - 教师\n  - 作业\n---\ncontent",
    )
    .expect("write grader skill");

    let bundle_path = tmp.path().join("teacher-suite.industrypack");
    pack_industry_bundle(
        vec![
            teacher.to_string_lossy().to_string(),
            grader.to_string_lossy().to_string(),
        ],
        "教师行业包".to_string(),
        "edu-teacher-suite".to_string(),
        "1.2.0".to_string(),
        "教师".to_string(),
        bundle_path.to_string_lossy().to_string(),
    )
    .await
    .expect("pack industry bundle");
    assert!(bundle_path.exists());

    let manifest = read_industry_bundle_manifest(bundle_path.to_string_lossy().to_string())
        .await
        .expect("read bundle manifest");
    assert_eq!(manifest.pack_id, "edu-teacher-suite");
    assert_eq!(manifest.version, "1.2.0");
    assert_eq!(manifest.skills.len(), 2);

    let unpack_root = tmp.path().join("imported");
    let unpacked = unpack_industry_bundle(
        bundle_path.to_string_lossy().to_string(),
        Some(unpack_root.to_string_lossy().to_string()),
    )
    .await
    .expect("unpack industry bundle");

    assert_eq!(unpacked.manifest.pack_id, "edu-teacher-suite");
    assert_eq!(unpacked.skill_dirs.len(), 2);
    for dir in unpacked.skill_dirs {
        let skill_md = std::path::Path::new(&dir).join("SKILL.md");
        assert!(skill_md.exists(), "missing extracted SKILL.md in {}", dir);
    }
}
