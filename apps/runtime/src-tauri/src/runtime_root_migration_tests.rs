use super::*;
use crate::runtime_bootstrap::{
    bootstrap_recovery_file_path, default_runtime_root_bootstrap, discover_runtime_root_bootstrap,
    read_runtime_root_bootstrap, set_bootstrap_write_failure_after_calls_for_tests,
    set_bootstrap_write_failure_plan_for_tests, write_runtime_root_bootstrap,
    BootstrapMigrationStatus, RuntimeRootBootstrap, RuntimeRootBootstrapMigration,
};
use crate::runtime_paths::RuntimePaths;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::process::Command;

fn make_bootstrap_path() -> (tempfile::TempDir, PathBuf) {
    set_bootstrap_write_failure_after_calls_for_tests(None);
    set_managed_path_cleanup_failure_after_calls_for_tests(None);
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let bootstrap_path = temp_dir.path().join("bootstrap-root.json");
    (temp_dir, bootstrap_path)
}

fn seed_runtime_tree(root: &Path) {
    let paths = RuntimePaths::new(root.to_path_buf());

    fs::create_dir_all(&paths.root).expect("create root");
    fs::write(paths.database.db_path, "db").expect("write db");
    fs::write(paths.database.wal_path, "wal").expect("write wal");
    fs::write(paths.database.shm_path, "shm").expect("write shm");

    fs::create_dir_all(&paths.diagnostics.logs_dir).expect("create diagnostics logs");
    fs::write(
        paths.diagnostics.logs_dir.join("runtime-2026-04-06.jsonl"),
        "log",
    )
    .expect("write diagnostics log");
    fs::create_dir_all(&paths.diagnostics.audit_dir).expect("create diagnostics audit");
    fs::write(
        paths.diagnostics.audit_dir.join("audit-2026-04-06.jsonl"),
        "audit",
    )
    .expect("write audit log");

    fs::create_dir_all(&paths.cache_dir).expect("create cache");
    fs::write(paths.cache_dir.join("runtime-cache.bin"), "cache").expect("write cache");

    fs::create_dir_all(&paths.sessions_dir).expect("create sessions");
    fs::create_dir_all(paths.sessions_dir.join("session-1")).expect("create session dir");
    fs::write(
        paths.sessions_dir.join("session-1").join("journal.json"),
        "journal",
    )
    .expect("write session journal");

    fs::create_dir_all(&paths.transcripts_dir).expect("create transcripts");
    fs::write(
        paths.transcripts_dir.join("session-1.ndjson"),
        "{\"event\":\"token\"}",
    )
    .expect("write transcript");

    fs::create_dir_all(&paths.memory_dir).expect("create memory");
    fs::create_dir_all(
        paths
            .memory_dir
            .join("employees")
            .join("pm")
            .join("skills")
            .join("skill-alpha"),
    )
    .expect("create employee memory");
    fs::write(
        paths
            .memory_dir
            .join("employees")
            .join("pm")
            .join("skills")
            .join("skill-alpha")
            .join("MEMORY.md"),
        "memory",
    )
    .expect("write employee memory");

    fs::create_dir_all(&paths.employees_dir).expect("create employees");
    fs::create_dir_all(paths.employees_dir.join("pm")).expect("create employee profile");
    fs::write(paths.employees_dir.join("pm").join("profile.md"), "profile")
        .expect("write employee profile");

    fs::create_dir_all(&paths.skills_dir).expect("create skills");
    fs::create_dir_all(paths.skills_dir.join("local-skill")).expect("create local skill dir");
    fs::write(paths.skills_dir.join("local-skill").join("SKILL.md"), "skill")
        .expect("write local skill");

    fs::create_dir_all(&paths.market_skills_dir).expect("create market skills");
    fs::create_dir_all(paths.market_skills_dir.join("bundle-a")).expect("create market bundle");
    fs::write(paths.market_skills_dir.join("bundle-a").join("SKILL.md"), "market")
        .expect("write market skill");

    fs::create_dir_all(&paths.plugins.root).expect("create plugins root");
    fs::create_dir_all(paths.plugins.root.join("plugin-a")).expect("create plugin dir");
    fs::write(
        paths.plugins.root.join("plugin-a").join("manifest.json"),
        "plugin",
    )
    .expect("write plugin manifest");
    fs::create_dir_all(&paths.plugins.state_dir).expect("create plugin state");
    fs::write(paths.plugins.state_dir.join("registry.json"), "state").expect("write plugin state");
    fs::create_dir_all(&paths.plugins.cli_shim_dir).expect("create plugin cli shim");
    fs::write(paths.plugins.cli_shim_dir.join("shim.json"), "shim").expect("write cli shim");
    fs::create_dir_all(&paths.plugins.fixture_dir).expect("create plugin fixtures");
    fs::write(
        paths.plugins.fixture_dir.join("openclaw-lark.json"),
        "fixture",
    )
    .expect("write plugin fixture");
    fs::create_dir_all(&paths.plugins.skills_vendor_dir).expect("create skills vendor");
    fs::create_dir_all(paths.plugins.skills_vendor_dir.join("skill-a"))
        .expect("create vendored skill dir");
    fs::write(
        paths
            .plugins
            .skills_vendor_dir
            .join("skill-a")
            .join("SKILL.md"),
        "skill",
    )
    .expect("write vendored skill");

    fs::create_dir_all(&paths.workspace_dir).expect("create workspace");
    fs::write(paths.workspace_dir.join("notes.txt"), "notes").expect("write workspace file");
}

fn assert_runtime_tree_exists(root: &Path) {
    let paths = RuntimePaths::new(root.to_path_buf());

    assert!(paths.database.db_path.exists());
    assert!(paths.database.wal_path.exists());
    assert!(paths.database.shm_path.exists());
    assert!(paths
        .diagnostics
        .logs_dir
        .join("runtime-2026-04-06.jsonl")
        .exists());
    assert!(paths
        .diagnostics
        .audit_dir
        .join("audit-2026-04-06.jsonl")
        .exists());
    assert!(paths.cache_dir.join("runtime-cache.bin").exists());
    assert!(paths
        .sessions_dir
        .join("session-1")
        .join("journal.json")
        .exists());
    assert!(paths.transcripts_dir.join("session-1.ndjson").exists());
    assert!(paths
        .memory_dir
        .join("employees")
        .join("pm")
        .join("skills")
        .join("skill-alpha")
        .join("MEMORY.md")
        .exists());
    assert!(paths.employees_dir.join("pm").join("profile.md").exists());
    assert!(paths.skills_dir.join("local-skill").join("SKILL.md").exists());
    assert!(paths.market_skills_dir.join("bundle-a").join("SKILL.md").exists());
    assert!(paths
        .plugins
        .root
        .join("plugin-a")
        .join("manifest.json")
        .exists());
    assert!(paths.plugins.state_dir.join("registry.json").exists());
    assert!(paths.plugins.cli_shim_dir.join("shim.json").exists());
    assert!(paths
        .plugins
        .fixture_dir
        .join("openclaw-lark.json")
        .exists());
    assert!(paths
        .plugins
        .skills_vendor_dir
        .join("skill-a")
        .join("SKILL.md")
        .exists());
    assert!(paths.workspace_dir.join("notes.txt").exists());
}

#[cfg(target_os = "windows")]
fn create_junction(link: &Path, target: &Path) {
    fs::create_dir_all(link.parent().expect("junction parent")).expect("create junction parent");
    let output = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link.to_string_lossy(),
            &target.to_string_lossy(),
        ])
        .output()
        .expect("run mklink");
    assert!(
        output.status.success(),
        "mklink failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn schedule_migration_records_pending_migration() {
    let (_temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = PathBuf::from(r"D:\WorkClawData");
    let target_root = PathBuf::from(r"E:\WorkClawData");
    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");

    let scheduled =
        schedule_runtime_root_migration(&bootstrap_path, &target_root).expect("schedule migration");

    assert_eq!(scheduled.current_root, current_root.to_string_lossy());
    let pending = scheduled.pending_migration.expect("pending migration");
    assert_eq!(pending.from_root, current_root.to_string_lossy());
    assert_eq!(pending.to_root, target_root.to_string_lossy());
    assert_eq!(pending.status, BootstrapMigrationStatus::Pending);

    let persisted = read_runtime_root_bootstrap(&bootstrap_path).expect("read bootstrap");
    assert_eq!(persisted.pending_migration, Some(pending));
}

#[test]
fn schedule_migration_recovers_from_malformed_bootstrap() {
    let (temp_dir, bootstrap_path) = make_bootstrap_path();
    std::fs::write(&bootstrap_path, "{ this is not valid json").expect("write malformed bootstrap");
    let target_root = temp_dir.path().join("scheduled-target");
    let expected_current_root = runtime_paths::resolve_runtime_root();

    let scheduled =
        schedule_runtime_root_migration(&bootstrap_path, &target_root).expect("schedule migration");

    assert_eq!(
        scheduled.current_root,
        expected_current_root.to_string_lossy()
    );
    assert!(scheduled.pending_migration.is_some());

    let persisted = read_runtime_root_bootstrap(&bootstrap_path).expect("read bootstrap");
    assert!(persisted.pending_migration.is_some());
}

#[test]
fn schedule_migration_rejects_empty_target_root() {
    let (_temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = PathBuf::from(r"D:\WorkClawData");
    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");

    let result = schedule_runtime_root_migration(&bootstrap_path, &PathBuf::new());

    assert!(matches!(
        result,
        Err(RuntimeRootMigrationError::EmptyTargetRoot)
    ));
}

#[test]
fn schedule_migration_rejects_non_writable_target_root() {
    let (_temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = PathBuf::from(r"D:\WorkClawData");
    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");

    let non_writable_target = bootstrap_path.with_file_name("target-root.txt");
    fs::write(&non_writable_target, "locked").expect("seed file target");

    let result = schedule_runtime_root_migration(&bootstrap_path, &non_writable_target);

    assert!(matches!(
        result,
        Err(RuntimeRootMigrationError::TargetRootNotWritable { .. })
    ));
}

#[test]
fn schedule_migration_rejects_nested_target_roots() {
    let (_temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = PathBuf::from(r"D:\WorkClawData");
    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");

    let nested_target = current_root.join("child");
    let result = schedule_runtime_root_migration(&bootstrap_path, &nested_target);

    assert!(matches!(
        result,
        Err(RuntimeRootMigrationError::NestedTarget(_))
    ));
}

#[test]
fn schedule_migration_rejects_second_pending_schedule() {
    let (_temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = PathBuf::from(r"D:\WorkClawData");
    let target_root = PathBuf::from(r"E:\WorkClawData");
    let bootstrap = RuntimeRootBootstrap {
        pending_migration: Some(RuntimeRootBootstrapMigration {
            from_root: current_root.to_string_lossy().to_string(),
            to_root: r"F:\WorkClawData".to_string(),
            status: BootstrapMigrationStatus::Pending,
            created_at: "2026-04-06T10:00:00Z".to_string(),
            last_error: None,
        }),
        ..default_runtime_root_bootstrap(&current_root)
    };
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");

    let result = schedule_runtime_root_migration(&bootstrap_path, &target_root);

    assert!(matches!(
        result,
        Err(RuntimeRootMigrationError::PendingMigrationAlreadyScheduled { .. })
    ));
}

#[test]
fn execute_migration_moves_managed_runtime_paths_and_records_completion_metadata() {
    let (temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = temp_dir.path().join("old-root");
    let target_root = temp_dir.path().join("new-root");
    fs::create_dir_all(&current_root).expect("create old root");
    fs::create_dir_all(target_root.parent().expect("target parent")).expect("create target parent");
    seed_runtime_tree(&current_root);

    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");
    schedule_runtime_root_migration(&bootstrap_path, &target_root).expect("schedule migration");

    let completed = execute_runtime_root_migration(&bootstrap_path).expect("execute migration");

    let persisted = read_runtime_root_bootstrap(&bootstrap_path).expect("read bootstrap");
    let current_root_text = current_root.to_string_lossy().to_string();
    let target_root_text = target_root.to_string_lossy().to_string();
    assert_eq!(completed.current_root, target_root.to_string_lossy());
    assert_eq!(persisted.current_root, target_root.to_string_lossy());
    assert_eq!(
        persisted.previous_root.as_deref(),
        Some(current_root_text.as_str())
    );
    assert!(persisted.pending_migration.is_none());
    let result = persisted
        .last_migration_result
        .expect("completion metadata");
    assert_eq!(result.from_root, current_root_text);
    assert_eq!(result.to_root, target_root_text);
    assert_eq!(result.status, BootstrapMigrationStatus::Completed);
    assert!(!result.completed_at.is_empty());

    assert_runtime_tree_exists(&target_root);
    assert!(!RuntimePaths::new(current_root.clone())
        .database
        .db_path
        .exists());
    assert!(!RuntimePaths::new(current_root.clone())
        .cache_dir
        .join("runtime-cache.bin")
        .exists());
    assert!(!RuntimePaths::new(current_root.clone())
        .sessions_dir
        .join("session-1")
        .join("journal.json")
        .exists());
    assert!(!RuntimePaths::new(current_root.clone())
        .transcripts_dir
        .join("session-1.ndjson")
        .exists());
    assert!(!RuntimePaths::new(current_root.clone())
        .employees_dir
        .join("pm")
        .join("profile.md")
        .exists());
    assert!(!RuntimePaths::new(current_root.clone())
        .plugins
        .fixture_dir
        .join("openclaw-lark.json")
        .exists());
    assert!(!RuntimePaths::new(current_root.clone())
        .workspace_dir
        .join("notes.txt")
        .exists());
}

#[cfg(target_os = "windows")]
#[test]
fn execute_migration_skips_plugin_fixture_reparse_points() {
    let (temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = temp_dir.path().join("old-root");
    let target_root = temp_dir.path().join("new-root");
    fs::create_dir_all(&current_root).expect("create old root");
    fs::create_dir_all(target_root.parent().expect("target parent")).expect("create target parent");
    seed_runtime_tree(&current_root);

    let source_paths = RuntimePaths::new(current_root.clone());
    let shared_node_modules = source_paths
        .plugins
        .root
        .join("plugin-a")
        .join("workspace")
        .join("node_modules");
    fs::create_dir_all(shared_node_modules.join("@scope").join("pkg")).expect("create node modules");
    fs::write(
        shared_node_modules
            .join("@scope")
            .join("pkg")
            .join("index.js"),
        "module.exports = true;",
    )
    .expect("write shared node module");
    let fixture_link = source_paths
        .plugins
        .fixture_dir
        .join("debug-openclaw-lark-runtime")
        .join("node_modules");
    create_junction(&fixture_link, &shared_node_modules);

    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");
    schedule_runtime_root_migration(&bootstrap_path, &target_root).expect("schedule migration");

    execute_runtime_root_migration(&bootstrap_path).expect("execute migration");

    let target_paths = RuntimePaths::new(target_root.clone());
    assert!(target_paths
        .plugins
        .root
        .join("plugin-a")
        .join("workspace")
        .join("node_modules")
        .join("@scope")
        .join("pkg")
        .join("index.js")
        .exists());
    assert!(!target_paths
        .plugins
        .fixture_dir
        .join("debug-openclaw-lark-runtime")
        .join("node_modules")
        .exists());
}

#[test]
fn execute_migration_restores_bootstrap_after_partial_copy_failure() {
    let (temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = temp_dir.path().join("old-root");
    let target_root = temp_dir.path().join("new-root");
    fs::create_dir_all(&current_root).expect("create old root");
    fs::create_dir_all(target_root.parent().expect("target parent")).expect("create target parent");
    seed_runtime_tree(&current_root);

    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");
    schedule_runtime_root_migration(&bootstrap_path, &target_root).expect("schedule migration");

    let target_workspace_file = RuntimePaths::new(target_root.clone()).workspace_dir;
    fs::create_dir_all(target_workspace_file.parent().expect("workspace parent"))
        .expect("create workspace parent");
    fs::write(&target_workspace_file, "conflict").expect("seed conflicting target file");

    let result = execute_runtime_root_migration(&bootstrap_path);

    assert!(result.is_err());

    let persisted = read_runtime_root_bootstrap(&bootstrap_path).expect("read bootstrap");
    let current_root_text = current_root.to_string_lossy().to_string();
    let target_root_text = target_root.to_string_lossy().to_string();
    assert_eq!(persisted.current_root, current_root.to_string_lossy());
    assert!(persisted.previous_root.is_none());
    assert!(persisted.pending_migration.is_none());
    let result = persisted.last_migration_result.expect("failure metadata");
    assert_eq!(result.from_root, current_root_text);
    assert_eq!(result.to_root, target_root_text);
    assert_eq!(result.status, BootstrapMigrationStatus::RolledBack);
    assert!(!result.completed_at.is_empty());

    assert_runtime_tree_exists(&current_root);
    assert!(!RuntimePaths::new(target_root.clone())
        .database
        .db_path
        .exists());
    assert!(!RuntimePaths::new(target_root.clone())
        .cache_dir
        .join("runtime-cache.bin")
        .exists());
    assert!(!RuntimePaths::new(target_root.clone())
        .sessions_dir
        .join("session-1")
        .join("journal.json")
        .exists());
    assert!(!RuntimePaths::new(target_root.clone())
        .employees_dir
        .join("pm")
        .join("profile.md")
        .exists());
    assert!(!RuntimePaths::new(target_root.clone())
        .workspace_dir
        .join("notes.txt")
        .exists());
}

#[test]
fn execute_migration_rolls_back_when_completion_bootstrap_write_fails() {
    let (temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = temp_dir.path().join("old-root");
    let target_root = temp_dir.path().join("new-root");
    fs::create_dir_all(&current_root).expect("create old root");
    fs::create_dir_all(target_root.parent().expect("target parent")).expect("create target parent");
    seed_runtime_tree(&current_root);

    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");
    schedule_runtime_root_migration(&bootstrap_path, &target_root).expect("schedule migration");
    set_bootstrap_write_failure_after_calls_for_tests(Some(2));

    let result = execute_runtime_root_migration(&bootstrap_path);

    assert!(matches!(
        result,
        Err(RuntimeRootMigrationError::Bootstrap(_))
    ));

    let persisted = read_runtime_root_bootstrap(&bootstrap_path).expect("read bootstrap");
    let migration_result = persisted.last_migration_result.expect("rollback metadata");
    assert_eq!(persisted.current_root, current_root.to_string_lossy());
    assert!(persisted.pending_migration.is_none());
    assert_eq!(
        migration_result.status,
        BootstrapMigrationStatus::RolledBack
    );
    assert!(migration_result
        .message
        .as_deref()
        .unwrap_or_default()
        .contains("failed to finalize runtime root migration"));

    assert_runtime_tree_exists(&current_root);
    assert!(!RuntimePaths::new(target_root.clone())
        .database
        .db_path
        .exists());
    assert!(!RuntimePaths::new(target_root.clone())
        .workspace_dir
        .join("notes.txt")
        .exists());
}

#[test]
fn execute_migration_reports_previous_root_cleanup_failures() {
    let (temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = temp_dir.path().join("old-root");
    let target_root = temp_dir.path().join("new-root");
    fs::create_dir_all(&current_root).expect("create old root");
    fs::create_dir_all(target_root.parent().expect("target parent")).expect("create target parent");
    seed_runtime_tree(&current_root);

    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");
    schedule_runtime_root_migration(&bootstrap_path, &target_root).expect("schedule migration");
    set_managed_path_cleanup_failure_after_calls_for_tests(Some(0));

    let result = execute_runtime_root_migration(&bootstrap_path);

    assert!(matches!(
        result,
        Err(RuntimeRootMigrationError::SourceCleanupFailed { .. })
    ));

    let persisted = read_runtime_root_bootstrap(&bootstrap_path).expect("read bootstrap");
    let current_root_text = current_root.to_string_lossy().to_string();
    let target_root_text = target_root.to_string_lossy().to_string();
    let migration_result = persisted
        .last_migration_result
        .expect("completion metadata");
    assert_eq!(persisted.current_root, target_root_text);
    assert_eq!(
        persisted.previous_root.as_deref(),
        Some(current_root_text.as_str())
    );
    assert!(persisted.pending_migration.is_none());
    assert_eq!(migration_result.status, BootstrapMigrationStatus::Completed);
    assert!(migration_result
        .message
        .as_deref()
        .unwrap_or_default()
        .contains("failed to remove previous managed path"));

    assert_runtime_tree_exists(&target_root);
    assert!(RuntimePaths::new(current_root.clone())
        .workspace_dir
        .join("notes.txt")
        .exists());
}

#[test]
fn discover_runtime_root_bootstrap_recovers_after_double_finalization_write_failure() {
    let (temp_dir, bootstrap_path) = make_bootstrap_path();
    let current_root = temp_dir.path().join("old-root");
    let target_root = temp_dir.path().join("new-root");
    fs::create_dir_all(&current_root).expect("create old root");
    fs::create_dir_all(target_root.parent().expect("target parent")).expect("create target parent");
    seed_runtime_tree(&current_root);

    let bootstrap = default_runtime_root_bootstrap(&current_root);
    write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("seed bootstrap");
    schedule_runtime_root_migration(&bootstrap_path, &target_root).expect("schedule migration");
    set_bootstrap_write_failure_plan_for_tests(&[2, 1]);

    let result = execute_runtime_root_migration(&bootstrap_path);

    assert!(matches!(
        result,
        Err(RuntimeRootMigrationError::Bootstrap(_))
    ));

    let unrecovered =
        read_runtime_root_bootstrap(&bootstrap_path).expect("read in-progress bootstrap");
    assert!(matches!(
        unrecovered
            .pending_migration
            .as_ref()
            .map(|pending| pending.status),
        Some(BootstrapMigrationStatus::InProgress)
    ));

    set_bootstrap_write_failure_after_calls_for_tests(None);
    let recovered = discover_runtime_root_bootstrap(&bootstrap_path, None, temp_dir.path())
        .expect("recover bootstrap");

    let recovered_result = recovered.last_migration_result.expect("rollback metadata");
    assert_eq!(recovered.current_root, current_root.to_string_lossy());
    assert!(recovered.pending_migration.is_none());
    assert_eq!(
        recovered_result.status,
        BootstrapMigrationStatus::RolledBack
    );
    assert!(!bootstrap_recovery_file_path(temp_dir.path()).exists());
    assert_runtime_tree_exists(&current_root);
    assert!(!RuntimePaths::new(target_root.clone())
        .database
        .db_path
        .exists());
}
