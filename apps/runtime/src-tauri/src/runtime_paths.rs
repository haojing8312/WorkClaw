use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

pub const DEFAULT_RUNTIME_ROOT_DIR_NAME: &str = crate::branding_generated::DEFAULT_RUNTIME_ROOT_DIR_NAME;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDatabasePaths {
    pub db_path: PathBuf,
    pub wal_path: PathBuf,
    pub shm_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDiagnosticsPaths {
    pub root: PathBuf,
    pub logs_dir: PathBuf,
    pub audit_dir: PathBuf,
    pub crashes_dir: PathBuf,
    pub exports_dir: PathBuf,
    pub state_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePluginPaths {
    pub root: PathBuf,
    pub cli_shim_dir: PathBuf,
    pub state_dir: PathBuf,
    pub fixture_dir: PathBuf,
    pub skills_vendor_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaths {
    pub root: PathBuf,
    pub database: RuntimeDatabasePaths,
    pub diagnostics: RuntimeDiagnosticsPaths,
    pub cache_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub transcripts_dir: PathBuf,
    pub memory_dir: PathBuf,
    pub employees_dir: PathBuf,
    pub skills_dir: PathBuf,
    pub market_skills_dir: PathBuf,
    pub plugins: RuntimePluginPaths,
    pub workspace_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimePathValidationError {
    NestedMigrationTarget {
        current_root: PathBuf,
        target_root: PathBuf,
    },
}

impl fmt::Display for RuntimePathValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NestedMigrationTarget {
                current_root,
                target_root,
            } => write!(
                f,
                "nested migration target is not allowed: {} -> {}",
                current_root.display(),
                target_root.display()
            ),
        }
    }
}

impl std::error::Error for RuntimePathValidationError {}

impl RuntimePaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let database = RuntimeDatabasePaths {
            db_path: root.join("workclaw.db"),
            wal_path: root.join("workclaw.db-wal"),
            shm_path: root.join("workclaw.db-shm"),
        };
        let diagnostics = RuntimeDiagnosticsPaths {
            root: root.join("diagnostics"),
            logs_dir: root.join("diagnostics").join("logs"),
            audit_dir: root.join("diagnostics").join("audit"),
            crashes_dir: root.join("diagnostics").join("crashes"),
            exports_dir: root.join("diagnostics").join("exports"),
            state_dir: root.join("diagnostics").join("state"),
        };
        let cache_dir = root.join("cache");
        let sessions_dir = root.join("sessions");
        let transcripts_dir = root.join("transcripts");
        let memory_dir = root.join("memory");
        let employees_dir = root.join("employees");
        let skills_dir = root.join("skills");
        let market_skills_dir = root.join("market-skills");
        let plugins = RuntimePluginPaths {
            root: root.join("openclaw-plugins"),
            cli_shim_dir: root.join("openclaw-cli-shim"),
            state_dir: root.join("openclaw-state"),
            fixture_dir: root.join("plugin-host-fixtures"),
            skills_vendor_dir: skills_dir.join("vendor"),
        };
        let workspace_dir = root.join("workspace");

        Self {
            root,
            database,
            diagnostics,
            cache_dir,
            sessions_dir,
            transcripts_dir,
            memory_dir,
            employees_dir,
            skills_dir,
            market_skills_dir,
            plugins,
            workspace_dir,
        }
    }
}

pub fn resolve_runtime_root() -> PathBuf {
    resolve_runtime_root_with_home_env(std::env::var_os("USERPROFILE"), std::env::var_os("HOME"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn resolve_runtime_root_with_env(
    _appdata: Option<OsString>,
    userprofile: Option<OsString>,
) -> PathBuf {
    resolve_runtime_root_with_home_env(userprofile, None)
}

pub fn resolve_runtime_root_with_home_env(
    userprofile: Option<OsString>,
    home: Option<OsString>,
) -> PathBuf {
    if let Some(userprofile) = userprofile.filter(|value| !value.is_empty()) {
        return PathBuf::from(userprofile).join(DEFAULT_RUNTIME_ROOT_DIR_NAME);
    }

    if let Some(home) = home.filter(|value| !value.is_empty()) {
        return PathBuf::from(home).join(DEFAULT_RUNTIME_ROOT_DIR_NAME);
    }

    std::env::temp_dir().join(DEFAULT_RUNTIME_ROOT_DIR_NAME)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn resolve_default_work_dir() -> PathBuf {
    resolve_runtime_root().join("workspace")
}

pub fn resolve_default_work_dir_with_home_env(
    userprofile: Option<OsString>,
    home: Option<OsString>,
) -> PathBuf {
    resolve_runtime_root_with_home_env(userprofile, home).join("workspace")
}

pub fn validate_migration_target(
    current_root: &Path,
    target_root: &Path,
) -> Result<(), RuntimePathValidationError> {
    if current_root == target_root {
        return Err(RuntimePathValidationError::NestedMigrationTarget {
            current_root: current_root.to_path_buf(),
            target_root: target_root.to_path_buf(),
        });
    }

    if current_root.starts_with(target_root) || target_root.starts_with(current_root) {
        return Err(RuntimePathValidationError::NestedMigrationTarget {
            current_root: current_root.to_path_buf(),
            target_root: target_root.to_path_buf(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_default_root_from_userprofile_environment() {
        let root = resolve_runtime_root_with_env(None, Some(OsString::from(r"C:\Users\me")));

        assert_eq!(root, PathBuf::from(r"C:\Users\me").join(".workclaw"));
    }

    #[test]
    fn derives_all_paths_under_one_root() {
        let paths = RuntimePaths::new(PathBuf::from(r"D:\WorkClawData"));

        assert!(paths.database.db_path.starts_with(&paths.root));
        assert!(paths.database.wal_path.starts_with(&paths.root));
        assert!(paths.database.shm_path.starts_with(&paths.root));
        assert!(paths.diagnostics.root.starts_with(&paths.root));
        assert!(paths.cache_dir.starts_with(&paths.root));
        assert!(paths.sessions_dir.starts_with(&paths.root));
        assert!(paths.transcripts_dir.starts_with(&paths.root));
        assert!(paths.memory_dir.starts_with(&paths.root));
        assert!(paths.employees_dir.starts_with(&paths.root));
        assert!(paths.skills_dir.starts_with(&paths.root));
        assert!(paths.market_skills_dir.starts_with(&paths.root));
        assert!(paths.plugins.root.starts_with(&paths.root));
        assert!(paths.plugins.fixture_dir.starts_with(&paths.root));
        assert!(paths.workspace_dir.starts_with(&paths.root));
    }

    #[test]
    fn default_workspace_is_root_workspace() {
        let paths = RuntimePaths::new(PathBuf::from(r"D:\WorkClawData"));

        assert_eq!(paths.workspace_dir, PathBuf::from(r"D:\WorkClawData").join("workspace"));
    }

    #[test]
    fn rejects_nested_migration_targets() {
        let current_root = PathBuf::from(r"D:\WorkClawData");
        let nested_target = current_root.join("child");

        let result = validate_migration_target(&current_root, &nested_target);

        assert!(result.is_err());
    }
}
