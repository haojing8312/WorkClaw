use std::path::{Component, Path};

const SENSITIVE_DIR_NAMES: &[&str] = &[".ssh", ".aws", ".azure", ".kube", ".gnupg", ".git"];

const SENSITIVE_FILE_NAMES: &[&str] = &[
    ".bashrc",
    ".bash_profile",
    ".zshrc",
    ".profile",
    "credentials",
    "known_hosts",
    "authorized_keys",
];

const SENSITIVE_EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx"];

pub(crate) fn is_sensitive_path(path: &Path) -> bool {
    let mut inside_sensitive_dir = false;
    let mut segments = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            continue;
        };
        let segment = value.to_string_lossy().to_ascii_lowercase();
        segments.push(segment.clone());
        if SENSITIVE_DIR_NAMES.iter().any(|name| segment == *name) {
            inside_sensitive_dir = true;
            break;
        }
    }
    if inside_sensitive_dir {
        return true;
    }

    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    let file_name = file_name.to_ascii_lowercase();
    if file_name == ".env" || file_name.starts_with(".env.") {
        return true;
    }
    if is_powershell_profile_name(&file_name) {
        return true;
    }
    if is_windows_startup_path(&segments) {
        return true;
    }
    if SENSITIVE_FILE_NAMES.iter().any(|name| file_name == *name) {
        return true;
    }

    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            SENSITIVE_EXTENSIONS
                .iter()
                .any(|sensitive| ext == *sensitive)
        })
        .unwrap_or(false)
}

fn is_powershell_profile_name(file_name: &str) -> bool {
    file_name == "profile.ps1" || file_name.ends_with("_profile.ps1")
}

fn is_windows_startup_path(segments: &[String]) -> bool {
    segments.windows(3).any(|window| {
        window[0] == "start menu" && window[1] == "programs" && window[2] == "startup"
    })
}

#[cfg(test)]
mod tests {
    use super::is_sensitive_path;
    use std::path::Path;

    #[test]
    fn ordinary_config_file_is_not_sensitive_by_name_only() {
        assert!(!is_sensitive_path(Path::new(
            "C:/workspaces/project/config"
        )));
    }

    #[test]
    fn powershell_profile_file_is_sensitive() {
        assert!(is_sensitive_path(Path::new(
            "C:/Users/alice/Documents/PowerShell/Microsoft.PowerShell_profile.ps1"
        )));
    }

    #[test]
    fn windows_startup_folder_path_is_sensitive() {
        assert!(is_sensitive_path(Path::new(
            "C:/Users/alice/AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Startup/launch.bat"
        )));
    }
}
