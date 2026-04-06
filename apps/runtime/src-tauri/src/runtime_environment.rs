use crate::runtime_bootstrap::{
    discover_runtime_root_bootstrap, read_runtime_root_bootstrap,
    resolve_runtime_bootstrap_location, RuntimeBootstrapLocation, RuntimeRootBootstrap,
};
use crate::runtime_paths::{resolve_runtime_root, RuntimePaths};
use crate::runtime_root_migration::{execute_runtime_root_migration, RuntimeRootMigrationError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone)]
pub struct RuntimeEnvironment {
    pub bootstrap_location: RuntimeBootstrapLocation,
    pub bootstrap: RuntimeRootBootstrap,
    pub paths: RuntimePaths,
}

pub struct ManagedRuntimeEnvironment(pub Arc<RuntimeEnvironment>);

fn runtime_root_contains_managed_artifacts(candidate_root: &Path) -> bool {
    let candidate_paths = RuntimePaths::new(candidate_root.to_path_buf());
    [
        candidate_paths.database.db_path,
        candidate_paths.database.wal_path,
        candidate_paths.database.shm_path,
        candidate_paths.diagnostics.root,
        candidate_paths.cache_dir,
        candidate_paths.sessions_dir,
        candidate_paths.plugins.root,
        candidate_paths.plugins.cli_shim_dir,
        candidate_paths.plugins.state_dir,
        candidate_paths.plugins.skills_vendor_dir,
        candidate_paths.workspace_dir,
    ]
    .into_iter()
    .any(|path| path.exists())
}

fn resolve_legacy_runtime_root_candidate(
    legacy_app_data_dir: Option<PathBuf>,
    bootstrap_location: &RuntimeBootstrapLocation,
) -> Option<PathBuf> {
    let candidate_root = legacy_app_data_dir?;
    if !candidate_root.exists() {
        return None;
    }

    if candidate_root == bootstrap_location.bootstrap_dir
        && !runtime_root_contains_managed_artifacts(&candidate_root)
    {
        return None;
    }

    if runtime_root_contains_managed_artifacts(&candidate_root) {
        return Some(candidate_root);
    }

    None
}

fn finalize_pending_runtime_root_migration(
    bootstrap_path: &Path,
) -> Result<RuntimeRootBootstrap, String> {
    match execute_runtime_root_migration(bootstrap_path) {
        Ok(bootstrap) => Ok(bootstrap),
        Err(RuntimeRootMigrationError::SourceCleanupFailed { .. }) => {
            let bootstrap = read_runtime_root_bootstrap(bootstrap_path).map_err(|error| {
                format!("failed to reload runtime bootstrap after migration: {error}")
            })?;
            if bootstrap.pending_migration.is_none() {
                Ok(bootstrap)
            } else {
                Err(
                    "runtime root migration left pending state after source cleanup warning"
                        .to_string(),
                )
            }
        }
        Err(error) => Err(format!("failed to execute runtime root migration: {error}")),
    }
}

fn initialize_runtime_environment_with_inputs(
    legacy_app_data_dir: Option<PathBuf>,
    bootstrap_location: RuntimeBootstrapLocation,
    default_root: PathBuf,
) -> Result<RuntimeEnvironment, String> {
    let legacy_root =
        resolve_legacy_runtime_root_candidate(legacy_app_data_dir, &bootstrap_location);
    let mut bootstrap = discover_runtime_root_bootstrap(
        &bootstrap_location.bootstrap_path,
        legacy_root.as_deref(),
        &default_root,
    )
    .map_err(|error| format!("failed to discover runtime bootstrap: {error}"))?;

    if bootstrap.pending_migration.is_some() {
        bootstrap = finalize_pending_runtime_root_migration(&bootstrap_location.bootstrap_path)?;
    }

    let paths = RuntimePaths::new(PathBuf::from(&bootstrap.current_root));
    Ok(RuntimeEnvironment {
        bootstrap_location,
        bootstrap,
        paths,
    })
}

pub fn initialize_runtime_environment(app: &AppHandle) -> Result<RuntimeEnvironment, String> {
    initialize_runtime_environment_with_inputs(
        app.path().app_data_dir().ok(),
        resolve_runtime_bootstrap_location(),
        resolve_runtime_root(),
    )
}

pub fn runtime_paths_from_app(app: &AppHandle) -> Result<RuntimePaths, String> {
    if let Ok(environment) = runtime_environment_from_app(app) {
        return Ok(environment.paths.clone());
    }

    initialize_runtime_environment(app).map(|environment| environment.paths)
}

pub fn runtime_environment_from_app(app: &AppHandle) -> Result<Arc<RuntimeEnvironment>, String> {
    if let Some(environment) = app.try_state::<ManagedRuntimeEnvironment>() {
        return Ok(Arc::clone(&environment.0));
    }

    Ok(Arc::new(initialize_runtime_environment(app)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_bootstrap::{
        default_runtime_root_bootstrap, write_runtime_root_bootstrap,
        write_runtime_root_bootstrap_pending_migration, BootstrapMigrationStatus,
        RuntimeRootBootstrapMigration,
    };
    use crate::runtime_root_migration::{
        schedule_runtime_root_migration, set_managed_path_cleanup_failure_after_calls_for_tests,
    };
    use sqlx::{Row, SqlitePool};
    use std::fs;
    use std::path::Path;

    fn make_bootstrap_location(root: &Path) -> RuntimeBootstrapLocation {
        let bootstrap_dir = root.join("bootstrap-store");
        RuntimeBootstrapLocation {
            bootstrap_path: bootstrap_dir.join("bootstrap-root.json"),
            bootstrap_dir,
        }
    }

    fn sqlite_url(path: &Path) -> String {
        format!("sqlite://{}?mode=rwc", path.to_string_lossy())
    }

    fn seed_runtime_database(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create db parent");
        }

        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect(&sqlite_url(path))
                .await
                .expect("connect runtime database");

            for statement in [
                "CREATE TABLE sessions (id TEXT PRIMARY KEY)",
                "CREATE TABLE model_configs (id TEXT PRIMARY KEY)",
                "CREATE TABLE provider_configs (id TEXT PRIMARY KEY)",
            ] {
                sqlx::query(statement)
                    .execute(&pool)
                    .await
                    .expect("create table");
            }

            for session_id in ["sess-1", "sess-2", "sess-3", "sess-4", "sess-5", "sess-6"] {
                sqlx::query("INSERT INTO sessions (id) VALUES (?)")
                    .bind(session_id)
                    .execute(&pool)
                    .await
                    .expect("insert session");
            }

            for model_id in ["model-a", "model-b"] {
                sqlx::query("INSERT INTO model_configs (id) VALUES (?)")
                    .bind(model_id)
                    .execute(&pool)
                    .await
                    .expect("insert model config");
            }

            sqlx::query("INSERT INTO provider_configs (id) VALUES (?)")
                .bind("provider-a")
                .execute(&pool)
                .await
                .expect("insert provider config");

            pool.close().await;
        });
    }

    fn read_table_count(path: &Path, table: &str) -> i64 {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect(&sqlite_url(path))
                .await
                .expect("connect runtime database");
            let count = sqlx::query(&format!("SELECT COUNT(*) AS count FROM {table}"))
                .fetch_one(&pool)
                .await
                .expect("fetch count")
                .get::<i64, _>("count");
            pool.close().await;
            count
        })
    }

    #[test]
    fn ignores_bootstrap_store_without_legacy_runtime_artifacts() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bootstrap_location = make_bootstrap_location(temp_dir.path());
        fs::create_dir_all(&bootstrap_location.bootstrap_dir).expect("create bootstrap dir");
        let default_root = temp_dir.path().join(".workclaw");

        let environment = initialize_runtime_environment_with_inputs(
            Some(bootstrap_location.bootstrap_dir.clone()),
            bootstrap_location,
            default_root.clone(),
        )
        .expect("initialize runtime environment");

        assert_eq!(environment.paths.root, default_root);
        assert_eq!(
            environment.bootstrap.current_root,
            default_root.to_string_lossy()
        );
    }

    #[test]
    fn accepts_completed_migration_when_source_cleanup_reports_warning() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bootstrap_location = make_bootstrap_location(temp_dir.path());
        let legacy_root = temp_dir.path().join("legacy-root");
        let target_root = temp_dir.path().join("new-root");
        let default_root = temp_dir.path().join(".workclaw");

        fs::create_dir_all(legacy_root.join("workspace")).expect("create legacy workspace");
        fs::write(legacy_root.join("workclaw.db"), "db").expect("write db");
        fs::write(legacy_root.join("workspace").join("notes.txt"), "notes")
            .expect("write workspace file");

        let mut bootstrap = default_runtime_root_bootstrap(&legacy_root);
        write_runtime_root_bootstrap(&bootstrap_location.bootstrap_path, &bootstrap)
            .expect("seed bootstrap");
        write_runtime_root_bootstrap_pending_migration(
            &bootstrap_location.bootstrap_path,
            &mut bootstrap,
            RuntimeRootBootstrapMigration {
                from_root: legacy_root.to_string_lossy().to_string(),
                to_root: target_root.to_string_lossy().to_string(),
                status: BootstrapMigrationStatus::Pending,
                created_at: "2026-04-06T10:00:00Z".to_string(),
                last_error: None,
            },
        )
        .expect("schedule migration");

        set_managed_path_cleanup_failure_after_calls_for_tests(Some(0));
        let environment = initialize_runtime_environment_with_inputs(
            Some(legacy_root.clone()),
            bootstrap_location,
            default_root,
        )
        .expect("initialize runtime environment");

        assert_eq!(environment.paths.root, target_root);
        assert!(environment.bootstrap.pending_migration.is_none());
        assert_eq!(
            environment
                .bootstrap
                .last_migration_result
                .as_ref()
                .map(|result| result.status),
            Some(BootstrapMigrationStatus::Completed)
        );
    }

    #[test]
    fn persists_legacy_root_bootstrap_before_scheduling_migration() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bootstrap_location = make_bootstrap_location(temp_dir.path());
        let legacy_root = bootstrap_location.bootstrap_dir.clone();
        let default_root = temp_dir.path().join(".workclaw");
        let target_root = temp_dir.path().join("new-root");

        fs::create_dir_all(legacy_root.join("sessions")).expect("create legacy sessions");
        fs::write(legacy_root.join("workclaw.db"), "db").expect("write legacy db");
        fs::write(legacy_root.join("sessions").join("session.json"), "{}")
            .expect("write legacy session");

        let environment = initialize_runtime_environment_with_inputs(
            Some(legacy_root.clone()),
            bootstrap_location.clone(),
            default_root,
        )
        .expect("initialize runtime environment");

        assert_eq!(environment.paths.root, legacy_root);

        let bootstrap = read_runtime_root_bootstrap(&bootstrap_location.bootstrap_path)
            .expect("bootstrap should be persisted after legacy discovery");
        assert_eq!(bootstrap.current_root, legacy_root.to_string_lossy());

        schedule_runtime_root_migration(&bootstrap_location.bootstrap_path, &target_root)
            .expect("schedule migration");

        let scheduled_bootstrap = read_runtime_root_bootstrap(&bootstrap_location.bootstrap_path)
            .expect("read scheduled bootstrap");
        assert_eq!(
            scheduled_bootstrap
                .pending_migration
                .as_ref()
                .map(|pending| pending.from_root.as_str()),
            Some(legacy_root.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn migrates_legacy_appdata_runtime_data_into_new_root() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bootstrap_location = make_bootstrap_location(temp_dir.path());
        let legacy_root = bootstrap_location.bootstrap_dir.clone();
        let default_root = temp_dir.path().join(".workclaw");
        let target_root = temp_dir.path().join("appdata-root").join(".workclaw");
        fs::create_dir_all(target_root.parent().expect("target parent"))
            .expect("create target root parent");

        let legacy_paths = crate::runtime_paths::RuntimePaths::new(legacy_root.clone());
        seed_runtime_database(&legacy_paths.database.db_path);
        fs::create_dir_all(legacy_paths.sessions_dir.join("session-1"))
            .expect("create legacy sessions");
        fs::write(
            legacy_paths.sessions_dir.join("session-1").join("journal.json"),
            "{}",
        )
        .expect("write legacy session journal");
        fs::create_dir_all(legacy_paths.plugins.root.join("plugin-a"))
            .expect("create legacy plugin dir");
        fs::write(
            legacy_paths.plugins.root.join("plugin-a").join("manifest.json"),
            "{}",
        )
        .expect("write legacy plugin manifest");
        fs::create_dir_all(&legacy_paths.plugins.state_dir).expect("create legacy plugin state");
        fs::write(legacy_paths.plugins.state_dir.join("registry.json"), "{}")
            .expect("write legacy plugin state");

        let initial_environment = initialize_runtime_environment_with_inputs(
            Some(legacy_root.clone()),
            bootstrap_location.clone(),
            default_root,
        )
        .expect("initialize runtime environment");
        assert_eq!(initial_environment.paths.root, legacy_root);

        schedule_runtime_root_migration(&bootstrap_location.bootstrap_path, &target_root)
            .expect("schedule migration");

        let migrated_environment = initialize_runtime_environment_with_inputs(
            Some(legacy_root.clone()),
            bootstrap_location.clone(),
            temp_dir.path().join("ignored-default"),
        )
        .expect("finalize migration through environment initialization");

        let target_paths = crate::runtime_paths::RuntimePaths::new(target_root.clone());
        assert_eq!(migrated_environment.paths.root, target_root);
        assert_eq!(read_table_count(&target_paths.database.db_path, "sessions"), 6);
        assert_eq!(read_table_count(&target_paths.database.db_path, "model_configs"), 2);
        assert_eq!(read_table_count(&target_paths.database.db_path, "provider_configs"), 1);
        assert!(target_paths
            .sessions_dir
            .join("session-1")
            .join("journal.json")
            .exists());
        assert!(target_paths
            .plugins
            .root
            .join("plugin-a")
            .join("manifest.json")
            .exists());
        assert!(target_paths.plugins.state_dir.join("registry.json").exists());
    }
}
