use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const BOOTSTRAP_FILE_NAME: &str = "bootstrap-root.json";
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

fn is_recoverable_bootstrap_read_error(error: &RuntimeBootstrapError) -> bool {
    match error {
        RuntimeBootstrapError::Io(io_error) => io_error.kind() == std::io::ErrorKind::NotFound,
        RuntimeBootstrapError::Json(_) => true,
    }
}

pub fn bootstrap_file_path(bootstrap_dir: &Path) -> PathBuf {
    bootstrap_dir.join(BOOTSTRAP_FILE_NAME)
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
    resolve_runtime_bootstrap_location_with_env(std::env::var_os("APPDATA"), std::env::var_os("USERPROFILE"))
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

pub fn write_runtime_root_bootstrap(
    bootstrap_path: &Path,
    bootstrap: &RuntimeRootBootstrap,
) -> Result<(), RuntimeBootstrapError> {
    if let Some(parent) = bootstrap_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(bootstrap)?;
    fs::write(bootstrap_path, raw)?;
    Ok(())
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
    if bootstrap_path.exists() {
        return load_or_create_runtime_root_bootstrap(bootstrap_path, default_root);
    }

    if let Some(legacy_root) = legacy_root {
        if legacy_root.exists() {
            return Ok(default_runtime_root_bootstrap(legacy_root));
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
        let location = resolve_runtime_bootstrap_location_with_env(
            None,
            Some(OsString::from(r"C:\Users\me")),
        );

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
        let missing = RuntimeBootstrapError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        ));
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

        let discovered =
            discover_runtime_root_bootstrap(&bootstrap_path, None, &fallback_root)
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
