use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const BOOTSTRAP_FILE_NAME: &str = "bootstrap-root.json";
pub const BOOTSTRAP_RECOVERY_FILE_NAME: &str = "bootstrap-root.recovery.json";
pub const BOOTSTRAP_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BootstrapMigrationStatus {
    Pending,
    InProgress,
    Failed,
    Completed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRootBootstrapMigration {
    pub from_root: String,
    pub to_root: String,
    pub status: BootstrapMigrationStatus,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRootBootstrapMigrationResult {
    pub from_root: String,
    pub to_root: String,
    pub status: BootstrapMigrationStatus,
    pub completed_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRootBootstrap {
    pub schema_version: u32,
    pub current_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_migration: Option<RuntimeRootBootstrapMigration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_migration_result: Option<RuntimeRootBootstrapMigrationResult>,
}

#[derive(Debug)]
pub enum RuntimeBootstrapError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBootstrapLocation {
    pub bootstrap_dir: PathBuf,
    pub bootstrap_path: PathBuf,
}

impl fmt::Display for RuntimeBootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RuntimeBootstrapError {}

impl From<std::io::Error> for RuntimeBootstrapError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for RuntimeBootstrapError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[cfg(test)]
thread_local! {
    static BOOTSTRAP_WRITE_FAILURE_PLAN: RefCell<VecDeque<usize>> = const { RefCell::new(VecDeque::new()) };
}

#[cfg(test)]
fn maybe_fail_bootstrap_write_for_tests() -> Result<(), RuntimeBootstrapError> {
    BOOTSTRAP_WRITE_FAILURE_PLAN.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(remaining_successful_writes) = slot.front_mut() {
            if *remaining_successful_writes == 0 {
                slot.pop_front();
                return Err(RuntimeBootstrapError::Io(std::io::Error::other(
                    "injected bootstrap write failure for tests",
                )));
            }

            *remaining_successful_writes -= 1;
        }

        Ok(())
    })
}

#[cfg(test)]
pub(crate) fn set_bootstrap_write_failure_after_calls_for_tests(
    remaining_successful_writes: Option<usize>,
) {
    let plan = remaining_successful_writes
        .into_iter()
        .collect::<VecDeque<_>>();
    BOOTSTRAP_WRITE_FAILURE_PLAN.with(|slot| *slot.borrow_mut() = plan);
}

#[cfg(test)]
pub(crate) fn set_bootstrap_write_failure_plan_for_tests(plan: &[usize]) {
    BOOTSTRAP_WRITE_FAILURE_PLAN.with(|slot| {
        *slot.borrow_mut() = plan.iter().copied().collect::<VecDeque<_>>();
    });
}

fn is_recoverable_bootstrap_read_error(error: &RuntimeBootstrapError) -> bool {
    match error {
        RuntimeBootstrapError::Io(io_error) => io_error.kind() == std::io::ErrorKind::NotFound,
        RuntimeBootstrapError::Json(_) => true,
    }
}

pub fn bootstrap_file_path(bootstrap_dir: &Path) -> PathBuf {
    bootstrap_dir.join(BOOTSTRAP_FILE_NAME)
}

pub fn bootstrap_recovery_file_path(bootstrap_dir: &Path) -> PathBuf {
    bootstrap_dir.join(BOOTSTRAP_RECOVERY_FILE_NAME)
}

fn build_runtime_bootstrap_location(base_dir: PathBuf) -> RuntimeBootstrapLocation {
    let bootstrap_dir = base_dir.join("dev.workclaw.runtime");
    let bootstrap_path = bootstrap_dir.join(BOOTSTRAP_FILE_NAME);
    RuntimeBootstrapLocation {
        bootstrap_dir,
        bootstrap_path,
    }
}

pub fn resolve_runtime_bootstrap_location() -> RuntimeBootstrapLocation {
    resolve_runtime_bootstrap_location_with_env(
        std::env::var_os("APPDATA"),
        std::env::var_os("USERPROFILE"),
    )
}

pub fn resolve_runtime_bootstrap_location_with_env(
    appdata: Option<OsString>,
    userprofile: Option<OsString>,
) -> RuntimeBootstrapLocation {
    if let Some(appdata) = appdata.filter(|value| !value.is_empty()) {
        return build_runtime_bootstrap_location(PathBuf::from(appdata));
    }

    if let Some(userprofile) = userprofile.filter(|value| !value.is_empty()) {
        return build_runtime_bootstrap_location(
            PathBuf::from(userprofile).join("AppData").join("Roaming"),
        );
    }

    build_runtime_bootstrap_location(std::env::temp_dir().join("WorkClaw"))
}

pub fn default_runtime_root_bootstrap(current_root: &Path) -> RuntimeRootBootstrap {
    RuntimeRootBootstrap {
        schema_version: BOOTSTRAP_SCHEMA_VERSION,
        current_root: current_root.to_string_lossy().to_string(),
        previous_root: None,
        pending_migration: None,
        last_migration_result: None,
    }
}

pub fn read_runtime_root_bootstrap(
    bootstrap_path: &Path,
) -> Result<RuntimeRootBootstrap, RuntimeBootstrapError> {
    let raw = fs::read_to_string(bootstrap_path)?;
    let bootstrap = serde_json::from_str(&raw)?;
    Ok(bootstrap)
}

fn write_runtime_root_bootstrap_file(
    bootstrap_file_path: &Path,
    bootstrap: &RuntimeRootBootstrap,
) -> Result<(), RuntimeBootstrapError> {
    #[cfg(test)]
    maybe_fail_bootstrap_write_for_tests()?;

    let parent_dir = bootstrap_file_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent_dir)?;

    let raw = serde_json::to_string_pretty(bootstrap)?;
    let mut temp_file = tempfile::NamedTempFile::new_in(parent_dir)?;
    temp_file.write_all(raw.as_bytes())?;
    temp_file.as_file_mut().sync_all()?;
    temp_file
        .persist(bootstrap_file_path)
        .map_err(|error| RuntimeBootstrapError::Io(error.error))?;
    Ok(())
}

pub fn write_runtime_root_bootstrap(
    bootstrap_path: &Path,
    bootstrap: &RuntimeRootBootstrap,
) -> Result<(), RuntimeBootstrapError> {
    write_runtime_root_bootstrap_file(bootstrap_path, bootstrap)
}

pub fn write_runtime_root_bootstrap_recovery(
    bootstrap_path: &Path,
    bootstrap: &RuntimeRootBootstrap,
) -> Result<(), RuntimeBootstrapError> {
    let recovery_path =
        bootstrap_recovery_file_path(bootstrap_path.parent().unwrap_or_else(|| Path::new(".")));
    write_runtime_root_bootstrap_file(&recovery_path, bootstrap)
}

pub fn clear_runtime_root_bootstrap_recovery(
    bootstrap_path: &Path,
) -> Result<(), RuntimeBootstrapError> {
    let recovery_path =
        bootstrap_recovery_file_path(bootstrap_path.parent().unwrap_or_else(|| Path::new(".")));
    match fs::remove_file(&recovery_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(RuntimeBootstrapError::Io(error)),
    }
}

pub fn write_runtime_root_bootstrap_pending_migration(
    bootstrap_path: &Path,
    bootstrap: &mut RuntimeRootBootstrap,
    pending_migration: RuntimeRootBootstrapMigration,
) -> Result<(), RuntimeBootstrapError> {
    bootstrap.previous_root = Some(bootstrap.current_root.clone());
    bootstrap.pending_migration = Some(pending_migration);
    bootstrap.last_migration_result = None;
    write_runtime_root_bootstrap(bootstrap_path, bootstrap)
}

pub fn load_or_create_runtime_root_bootstrap(
    bootstrap_path: &Path,
    default_root: &Path,
) -> Result<RuntimeRootBootstrap, RuntimeBootstrapError> {
    match read_runtime_root_bootstrap(bootstrap_path) {
        Ok(bootstrap) => Ok(bootstrap),
        Err(error) if is_recoverable_bootstrap_read_error(&error) => {
            let bootstrap = default_runtime_root_bootstrap(default_root);
            write_runtime_root_bootstrap(bootstrap_path, &bootstrap)?;
            Ok(bootstrap)
        }
        Err(error) => Err(error),
    }
}

pub fn discover_runtime_root_bootstrap(
    bootstrap_path: &Path,
    legacy_root: Option<&Path>,
    default_root: &Path,
) -> Result<RuntimeRootBootstrap, RuntimeBootstrapError> {
    let recovery_path =
        bootstrap_recovery_file_path(bootstrap_path.parent().unwrap_or_else(|| Path::new(".")));
    let recovery_bootstrap = if recovery_path.exists() {
        match read_runtime_root_bootstrap(&recovery_path) {
            Ok(bootstrap) => Some(bootstrap),
            Err(error) if is_recoverable_bootstrap_read_error(&error) => {
                let _ = fs::remove_file(&recovery_path);
                None
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };

    if bootstrap_path.exists() {
        match read_runtime_root_bootstrap(bootstrap_path) {
            Ok(bootstrap) => {
                let should_apply_recovery = recovery_bootstrap.is_some()
                    && matches!(
                        bootstrap
                            .pending_migration
                            .as_ref()
                            .map(|pending| pending.status),
                        Some(BootstrapMigrationStatus::InProgress)
                    );

                if should_apply_recovery {
                    let recovery_bootstrap = recovery_bootstrap.expect("recovery bootstrap");
                    write_runtime_root_bootstrap(bootstrap_path, &recovery_bootstrap)?;
                    clear_runtime_root_bootstrap_recovery(bootstrap_path)?;
                    return Ok(recovery_bootstrap);
                }

                if recovery_bootstrap.is_some() {
                    let _ = clear_runtime_root_bootstrap_recovery(bootstrap_path);
                }

                return Ok(bootstrap);
            }
            Err(error) if is_recoverable_bootstrap_read_error(&error) => {
                if let Some(recovery_bootstrap) = recovery_bootstrap {
                    write_runtime_root_bootstrap(bootstrap_path, &recovery_bootstrap)?;
                    clear_runtime_root_bootstrap_recovery(bootstrap_path)?;
                    return Ok(recovery_bootstrap);
                }
            }
            Err(error) => return Err(error),
        }
    } else if let Some(recovery_bootstrap) = recovery_bootstrap {
        write_runtime_root_bootstrap(bootstrap_path, &recovery_bootstrap)?;
        clear_runtime_root_bootstrap_recovery(bootstrap_path)?;
        return Ok(recovery_bootstrap);
    }

    if let Some(legacy_root) = legacy_root {
        if legacy_root.exists() {
            let bootstrap = default_runtime_root_bootstrap(legacy_root);
            write_runtime_root_bootstrap(bootstrap_path, &bootstrap)?;
            return Ok(bootstrap);
        }
    }

    load_or_create_runtime_root_bootstrap(bootstrap_path, default_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn resolves_stable_bootstrap_location_from_appdata_environment() {
        let location = resolve_runtime_bootstrap_location_with_env(
            Some(OsString::from(r"C:\Users\me\AppData\Roaming")),
            Some(OsString::from(r"C:\Users\me")),
        );

        assert_eq!(
            location.bootstrap_dir,
            PathBuf::from(r"C:\Users\me\AppData\Roaming").join("dev.workclaw.runtime")
        );
        assert_eq!(
            location.bootstrap_path,
            PathBuf::from(r"C:\Users\me\AppData\Roaming")
                .join("dev.workclaw.runtime")
                .join(BOOTSTRAP_FILE_NAME)
        );
    }

    #[test]
    fn resolves_stable_bootstrap_location_from_userprofile_when_appdata_is_missing() {
        let location =
            resolve_runtime_bootstrap_location_with_env(None, Some(OsString::from(r"C:\Users\me")));

        let expected_dir = PathBuf::from(r"C:\Users\me")
            .join("AppData")
            .join("Roaming")
            .join("dev.workclaw.runtime");
        assert_eq!(location.bootstrap_dir, expected_dir);
        assert_eq!(
            location.bootstrap_path,
            PathBuf::from(r"C:\Users\me")
                .join("AppData")
                .join("Roaming")
                .join("dev.workclaw.runtime")
                .join(BOOTSTRAP_FILE_NAME)
        );
    }

    #[test]
    fn only_not_found_and_json_errors_are_recoverable() {
        let missing =
            RuntimeBootstrapError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
        let denied = RuntimeBootstrapError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));
        let malformed = RuntimeBootstrapError::Json(
            serde_json::from_str::<RuntimeRootBootstrap>("{ invalid json")
                .expect_err("invalid json should fail"),
        );

        assert!(is_recoverable_bootstrap_read_error(&missing));
        assert!(!is_recoverable_bootstrap_read_error(&denied));
        assert!(is_recoverable_bootstrap_read_error(&malformed));
    }

    #[test]
    fn creates_default_bootstrap_when_file_is_missing() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bootstrap_path = temp_dir.path().join("bootstrap-root.json");
        let default_root = PathBuf::from(r"D:\WorkClawData");

        let bootstrap = load_or_create_runtime_root_bootstrap(&bootstrap_path, &default_root)
            .expect("load or create bootstrap");

        assert_eq!(bootstrap.current_root, default_root.to_string_lossy());
        assert!(bootstrap.pending_migration.is_none());
        assert!(bootstrap.last_migration_result.is_none());
        assert_eq!(bootstrap.schema_version, 1);
        assert!(bootstrap_path.exists());
    }

    #[test]
    fn reads_existing_bootstrap_file_with_current_root() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bootstrap_path = temp_dir.path().join("bootstrap-root.json");
        let bootstrap = RuntimeRootBootstrap {
            schema_version: 1,
            current_root: r"E:\custom-workclaw".to_string(),
            previous_root: Some(r"C:\Users\me\AppData\Roaming\dev.workclaw.runtime".to_string()),
            pending_migration: Some(RuntimeRootBootstrapMigration {
                from_root: r"C:\Users\me\AppData\Roaming\dev.workclaw.runtime".to_string(),
                to_root: r"E:\custom-workclaw".to_string(),
                status: BootstrapMigrationStatus::Pending,
                created_at: "2026-04-06T10:00:00Z".to_string(),
                last_error: None,
            }),
            last_migration_result: Some(RuntimeRootBootstrapMigrationResult {
                from_root: r"C:\Users\me\AppData\Roaming\dev.workclaw.runtime".to_string(),
                to_root: r"E:\custom-workclaw".to_string(),
                status: BootstrapMigrationStatus::Completed,
                completed_at: "2026-04-06T10:05:00Z".to_string(),
                message: Some("completed".to_string()),
            }),
        };

        write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("write bootstrap");

        let loaded = read_runtime_root_bootstrap(&bootstrap_path).expect("read bootstrap");

        assert_eq!(loaded, bootstrap);
    }

    #[test]
    fn rejects_malformed_bootstrap_content_and_falls_back_safely() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bootstrap_path = temp_dir.path().join("bootstrap-root.json");
        std::fs::write(&bootstrap_path, "{ this is not valid json")
            .expect("write invalid bootstrap");

        let read_result = read_runtime_root_bootstrap(&bootstrap_path);
        assert!(read_result.is_err());

        let fallback_root = PathBuf::from(r"D:\WorkClawData");
        let loaded = load_or_create_runtime_root_bootstrap(&bootstrap_path, &fallback_root)
            .expect("fallback bootstrap");

        assert_eq!(loaded.current_root, fallback_root.to_string_lossy());
    }

    #[test]
    fn discovery_falls_back_safely_when_existing_bootstrap_is_malformed() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bootstrap_path = temp_dir.path().join("bootstrap-root.json");
        std::fs::write(&bootstrap_path, "{ this is not valid json")
            .expect("write invalid bootstrap");
        let fallback_root = PathBuf::from(r"D:\WorkClawData");

        let discovered = discover_runtime_root_bootstrap(&bootstrap_path, None, &fallback_root)
            .expect("discover fallback bootstrap");

        assert_eq!(discovered.current_root, fallback_root.to_string_lossy());
    }

    #[test]
    fn discovery_prefers_bootstrap_over_legacy_directories() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bootstrap_path = temp_dir.path().join("bootstrap-root.json");
        let legacy_root = temp_dir.path().join("legacy-runtime");
        std::fs::create_dir_all(&legacy_root).expect("create legacy root");

        let bootstrap = RuntimeRootBootstrap {
            schema_version: 1,
            current_root: r"E:\preferred-root".to_string(),
            previous_root: None,
            pending_migration: None,
            last_migration_result: None,
        };
        write_runtime_root_bootstrap(&bootstrap_path, &bootstrap).expect("write bootstrap");

        let discovered = discover_runtime_root_bootstrap(
            &bootstrap_path,
            Some(&legacy_root),
            PathBuf::from(r"D:\fallback").as_path(),
        )
        .expect("discover bootstrap");

        assert_eq!(discovered.current_root, bootstrap.current_root);
    }
}
