use crate::runtime_bootstrap::{
    write_runtime_root_bootstrap_pending_migration, BootstrapMigrationStatus,
    RuntimeBootstrapError, RuntimeRootBootstrap, RuntimeRootBootstrapMigration,
    RuntimeRootBootstrapMigrationResult,
};
use crate::runtime_paths::{self, RuntimePathValidationError};
#[cfg(test)]
use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum RuntimeRootMigrationError {
    EmptyTargetRoot,
    TargetRootNotWritable {
        target_root: PathBuf,
        reason: String,
    },
    NestedTarget(RuntimePathValidationError),
    Bootstrap(RuntimeBootstrapError),
    PendingMigrationAlreadyScheduled {
        target_root: PathBuf,
    },
    NoPendingMigration,
    MigrationExecutionFailed {
        path: PathBuf,
        reason: String,
    },
    SourceCleanupFailed {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for RuntimeRootMigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTargetRoot => write!(f, "migration target root cannot be empty"),
            Self::TargetRootNotWritable {
                target_root,
                reason,
            } => {
                write!(
                    f,
                    "migration target root is not writable: {} ({reason})",
                    target_root.display()
                )
            }
            Self::NestedTarget(error) => write!(f, "{error}"),
            Self::Bootstrap(error) => write!(f, "{error}"),
            Self::PendingMigrationAlreadyScheduled { target_root } => write!(
                f,
                "migration is already pending for bootstrap target root {}",
                target_root.display()
            ),
            Self::NoPendingMigration => write!(f, "no pending runtime root migration is scheduled"),
            Self::MigrationExecutionFailed { path, reason } => {
                write!(
                    f,
                    "failed to migrate managed path {}: {reason}",
                    path.display()
                )
            }
            Self::SourceCleanupFailed { path, reason } => {
                write!(
                    f,
                    "runtime root migration completed but failed to clean up previous managed path {}: {reason}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for RuntimeRootMigrationError {}

impl From<RuntimeBootstrapError> for RuntimeRootMigrationError {
    fn from(value: RuntimeBootstrapError) -> Self {
        Self::Bootstrap(value)
    }
}

impl From<RuntimePathValidationError> for RuntimeRootMigrationError {
    fn from(value: RuntimePathValidationError) -> Self {
        Self::NestedTarget(value)
    }
}

fn migration_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos())
}

fn probe_directory_writable(directory: &Path) -> Result<(), RuntimeRootMigrationError> {
    let probe_name = format!(
        ".workclaw-root-migration-probe-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let probe_path = directory.join(probe_name);
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
    {
        Ok(file) => {
            drop(file);
            let _ = fs::remove_file(&probe_path);
            Ok(())
        }
        Err(error) => Err(RuntimeRootMigrationError::TargetRootNotWritable {
            target_root: directory.to_path_buf(),
            reason: error.to_string(),
        }),
    }
}

fn validate_target_root_writable(target_root: &Path) -> Result<(), RuntimeRootMigrationError> {
    let writable_directory = if target_root.exists() {
        if !target_root.is_dir() {
            return Err(RuntimeRootMigrationError::TargetRootNotWritable {
                target_root: target_root.to_path_buf(),
                reason: "target root already exists as a file".to_string(),
            });
        }
        target_root
    } else {
        target_root
            .parent()
            .ok_or_else(|| RuntimeRootMigrationError::TargetRootNotWritable {
                target_root: target_root.to_path_buf(),
                reason: "target root has no writable parent directory".to_string(),
            })?
    };

    if !writable_directory.exists() {
        return Err(RuntimeRootMigrationError::TargetRootNotWritable {
            target_root: target_root.to_path_buf(),
            reason: "target root parent directory does not exist".to_string(),
        });
    }

    probe_directory_writable(writable_directory)
}

fn build_runtime_paths(root: &str) -> runtime_paths::RuntimePaths {
    runtime_paths::RuntimePaths::new(PathBuf::from(root))
}

fn managed_runtime_paths(source_root: &str, target_root: &str) -> Vec<(PathBuf, PathBuf, bool)> {
    let source = build_runtime_paths(source_root);
    let target = build_runtime_paths(target_root);
    vec![
        (source.database.db_path, target.database.db_path, false),
        (source.database.wal_path, target.database.wal_path, false),
        (source.database.shm_path, target.database.shm_path, false),
        (source.diagnostics.root, target.diagnostics.root, true),
        (source.cache_dir, target.cache_dir, true),
        (source.sessions_dir, target.sessions_dir, true),
        (source.plugins.root, target.plugins.root, true),
        (
            source.plugins.cli_shim_dir,
            target.plugins.cli_shim_dir,
            true,
        ),
        (source.plugins.state_dir, target.plugins.state_dir, true),
        (
            source.plugins.skills_vendor_dir,
            target.plugins.skills_vendor_dir,
            true,
        ),
        (source.workspace_dir, target.workspace_dir, true),
    ]
}

#[derive(Debug)]
struct ManagedPathCleanupError {
    path: PathBuf,
    reason: String,
}

#[cfg(test)]
thread_local! {
    static MANAGED_PATH_CLEANUP_FAILURE_AFTER_CALLS: RefCell<Option<usize>> = const { RefCell::new(None) };
}

#[cfg(test)]
fn maybe_fail_managed_path_cleanup_for_tests(path: &Path) -> Result<(), ManagedPathCleanupError> {
    MANAGED_PATH_CLEANUP_FAILURE_AFTER_CALLS.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(remaining_successful_calls) = slot.as_mut() {
            if *remaining_successful_calls == 0 {
                *slot = None;
                return Err(ManagedPathCleanupError {
                    path: path.to_path_buf(),
                    reason: "injected managed path cleanup failure for tests".to_string(),
                });
            }

            *remaining_successful_calls -= 1;
        }

        Ok(())
    })
}

#[cfg(test)]
pub(crate) fn set_managed_path_cleanup_failure_after_calls_for_tests(
    remaining_successful_calls: Option<usize>,
) {
    MANAGED_PATH_CLEANUP_FAILURE_AFTER_CALLS
        .with(|slot| *slot.borrow_mut() = remaining_successful_calls);
}

fn copy_managed_path(
    source: &Path,
    target: &Path,
    is_directory: bool,
) -> Result<(), RuntimeRootMigrationError> {
    if !source.exists() {
        return Ok(());
    }

    if is_directory {
        if target.exists() && !target.is_dir() {
            return Err(RuntimeRootMigrationError::MigrationExecutionFailed {
                path: target.to_path_buf(),
                reason: "target path already exists as a file".to_string(),
            });
        }
        fs::create_dir_all(target).map_err(|error| {
            RuntimeRootMigrationError::MigrationExecutionFailed {
                path: target.to_path_buf(),
                reason: error.to_string(),
            }
        })?;

        for entry in fs::read_dir(source).map_err(|error| {
            RuntimeRootMigrationError::MigrationExecutionFailed {
                path: source.to_path_buf(),
                reason: error.to_string(),
            }
        })? {
            let entry =
                entry.map_err(
                    |error| RuntimeRootMigrationError::MigrationExecutionFailed {
                        path: source.to_path_buf(),
                        reason: error.to_string(),
                    },
                )?;
            let child_source = entry.path();
            let child_target = target.join(entry.file_name());
            let child_is_directory = entry
                .file_type()
                .map_err(
                    |error| RuntimeRootMigrationError::MigrationExecutionFailed {
                        path: child_source.clone(),
                        reason: error.to_string(),
                    },
                )?
                .is_dir();
            copy_managed_path(&child_source, &child_target, child_is_directory)?;
        }
        return Ok(());
    }

    if target.exists() && target.is_dir() {
        return Err(RuntimeRootMigrationError::MigrationExecutionFailed {
            path: target.to_path_buf(),
            reason: "target path already exists as a directory".to_string(),
        });
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            RuntimeRootMigrationError::MigrationExecutionFailed {
                path: parent.to_path_buf(),
                reason: error.to_string(),
            }
        })?;
    }

    fs::copy(source, target).map_err(|error| {
        RuntimeRootMigrationError::MigrationExecutionFailed {
            path: source.to_path_buf(),
            reason: error.to_string(),
        }
    })?;
    Ok(())
}

fn remove_managed_path(path: &Path, _is_directory: bool) -> Result<(), ManagedPathCleanupError> {
    if !path.exists() {
        return Ok(());
    }

    #[cfg(test)]
    maybe_fail_managed_path_cleanup_for_tests(path)?;

    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|error| ManagedPathCleanupError {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    } else {
        fs::remove_file(path).map_err(|error| ManagedPathCleanupError {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    }

    Ok(())
}

fn remove_managed_runtime_paths(
    paths: &[(PathBuf, PathBuf, bool)],
    target_side: bool,
) -> Result<(), ManagedPathCleanupError> {
    for (source, target, is_directory) in paths.iter().rev() {
        let path = if target_side { target } else { source };
        remove_managed_path(path, *is_directory)?;
    }

    Ok(())
}

fn mark_migration_in_progress(bootstrap: &mut RuntimeRootBootstrap) {
    if let Some(pending) = bootstrap.pending_migration.as_mut() {
        pending.status = BootstrapMigrationStatus::InProgress;
        pending.last_error = None;
    }
}

fn record_migration_completion(
    bootstrap: &mut RuntimeRootBootstrap,
    from_root: &str,
    to_root: &str,
) {
    bootstrap.current_root = to_root.to_string();
    bootstrap.previous_root = Some(from_root.to_string());
    bootstrap.pending_migration = None;
    bootstrap.last_migration_result = Some(RuntimeRootBootstrapMigrationResult {
        from_root: from_root.to_string(),
        to_root: to_root.to_string(),
        status: BootstrapMigrationStatus::Completed,
        completed_at: migration_timestamp(),
        message: Some("runtime root migration completed".to_string()),
    });
}

fn update_last_migration_message(bootstrap: &mut RuntimeRootBootstrap, message: String) {
    if let Some(result) = bootstrap.last_migration_result.as_mut() {
        result.message = Some(message);
    }
}

fn record_migration_rollback(
    bootstrap: &mut RuntimeRootBootstrap,
    from_root: &str,
    to_root: &str,
    reason: &str,
) {
    bootstrap.current_root = from_root.to_string();
    bootstrap.previous_root = None;
    bootstrap.pending_migration = None;
    bootstrap.last_migration_result = Some(RuntimeRootBootstrapMigrationResult {
        from_root: from_root.to_string(),
        to_root: to_root.to_string(),
        status: BootstrapMigrationStatus::RolledBack,
        completed_at: migration_timestamp(),
        message: Some(reason.to_string()),
    });
}

pub fn schedule_runtime_root_migration(
    bootstrap_path: &Path,
    target_root: &Path,
) -> Result<RuntimeRootBootstrap, RuntimeRootMigrationError> {
    if target_root.as_os_str().is_empty() {
        return Err(RuntimeRootMigrationError::EmptyTargetRoot);
    }

    let default_root = runtime_paths::resolve_runtime_root();
    let mut bootstrap = crate::runtime_bootstrap::discover_runtime_root_bootstrap(
        bootstrap_path,
        None,
        &default_root,
    )?;
    if bootstrap.pending_migration.is_some() {
        return Err(
            RuntimeRootMigrationError::PendingMigrationAlreadyScheduled {
                target_root: target_root.to_path_buf(),
            },
        );
    }

    let current_root = PathBuf::from(&bootstrap.current_root);
    runtime_paths::validate_migration_target(&current_root, target_root)?;
    validate_target_root_writable(target_root)?;

    let pending_migration = RuntimeRootBootstrapMigration {
        from_root: bootstrap.current_root.clone(),
        to_root: target_root.to_string_lossy().to_string(),
        status: BootstrapMigrationStatus::Pending,
        created_at: migration_timestamp(),
        last_error: None,
    };

    write_runtime_root_bootstrap_pending_migration(
        bootstrap_path,
        &mut bootstrap,
        pending_migration,
    )?;

    Ok(bootstrap)
}

pub fn execute_runtime_root_migration(
    bootstrap_path: &Path,
) -> Result<RuntimeRootBootstrap, RuntimeRootMigrationError> {
    let mut bootstrap = crate::runtime_bootstrap::read_runtime_root_bootstrap(bootstrap_path)?;
    let pending = bootstrap
        .pending_migration
        .clone()
        .ok_or(RuntimeRootMigrationError::NoPendingMigration)?;

    let from_root = pending.from_root.clone();
    let target_root = pending.to_root.clone();
    let current_root = PathBuf::from(&from_root);
    let target_root_path = PathBuf::from(&target_root);

    runtime_paths::validate_migration_target(&current_root, &target_root_path)?;
    validate_target_root_writable(&target_root_path)?;

    mark_migration_in_progress(&mut bootstrap);
    crate::runtime_bootstrap::write_runtime_root_bootstrap(bootstrap_path, &bootstrap)?;

    let managed_paths = managed_runtime_paths(&from_root, &target_root);
    let copy_result: Result<(), RuntimeRootMigrationError> = (|| {
        for (source, target, is_directory) in &managed_paths {
            if source.exists() {
                copy_managed_path(source, target, *is_directory)?;
            }
        }
        Ok(())
    })();

    if let Err(error) = copy_result {
        let cleanup_message = remove_managed_runtime_paths(&managed_paths, true)
            .err()
            .map(|cleanup_error| {
                format!(
                    "{}; failed to clean partially migrated target data at {}: {}",
                    error,
                    cleanup_error.path.display(),
                    cleanup_error.reason
                )
            })
            .unwrap_or_else(|| error.to_string());
        record_migration_rollback(&mut bootstrap, &from_root, &target_root, &cleanup_message);
        crate::runtime_bootstrap::write_runtime_root_bootstrap(bootstrap_path, &bootstrap)?;
        return Err(error);
    }

    let mut rollback_recovery_bootstrap = bootstrap.clone();
    record_migration_rollback(
        &mut rollback_recovery_bootstrap,
        &from_root,
        &target_root,
        "runtime root migration rollback recovery is pending",
    );
    crate::runtime_bootstrap::write_runtime_root_bootstrap_recovery(
        bootstrap_path,
        &rollback_recovery_bootstrap,
    )?;

    record_migration_completion(&mut bootstrap, &from_root, &target_root);
    if let Err(error) =
        crate::runtime_bootstrap::write_runtime_root_bootstrap(bootstrap_path, &bootstrap)
    {
        let cleanup_message = remove_managed_runtime_paths(&managed_paths, true)
            .err()
            .map(|cleanup_error| {
                format!(
                    "failed to finalize runtime root migration: {error}; failed to clean migrated target data at {}: {}",
                    cleanup_error.path.display(),
                    cleanup_error.reason
                )
            })
            .unwrap_or_else(|| format!("failed to finalize runtime root migration: {error}"));
        record_migration_rollback(&mut bootstrap, &from_root, &target_root, &cleanup_message);
        let _ = crate::runtime_bootstrap::write_runtime_root_bootstrap_recovery(
            bootstrap_path,
            &bootstrap,
        );
        if let Err(rollback_write_error) =
            crate::runtime_bootstrap::write_runtime_root_bootstrap(bootstrap_path, &bootstrap)
        {
            return Err(RuntimeRootMigrationError::Bootstrap(
                RuntimeBootstrapError::Io(std::io::Error::other(format!(
                    "failed to finalize runtime root migration bootstrap after copying data: {error}; rollback bootstrap write also failed: {rollback_write_error}"
                ))),
            ));
        }

        let _ = crate::runtime_bootstrap::clear_runtime_root_bootstrap_recovery(bootstrap_path);
        return Err(RuntimeRootMigrationError::Bootstrap(error));
    }

    let _ = crate::runtime_bootstrap::clear_runtime_root_bootstrap_recovery(bootstrap_path);

    if let Err(cleanup_error) = remove_managed_runtime_paths(&managed_paths, false) {
        let cleanup_message = format!(
            "runtime root migration completed, but failed to remove previous managed path {}: {}",
            cleanup_error.path.display(),
            cleanup_error.reason
        );
        update_last_migration_message(&mut bootstrap, cleanup_message);
        crate::runtime_bootstrap::write_runtime_root_bootstrap(bootstrap_path, &bootstrap)?;
        return Err(RuntimeRootMigrationError::SourceCleanupFailed {
            path: cleanup_error.path,
            reason: cleanup_error.reason,
        });
    }

    Ok(bootstrap)
}

#[cfg(test)]
#[path = "runtime_root_migration_tests.rs"]
mod tests;
