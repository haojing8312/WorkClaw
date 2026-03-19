use crate::agent::types::{FileTaskCaps, ToolContext};
use anyhow::Result;
use std::path::{Path, PathBuf};

fn read_mode_from_extension(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("doc") | Some("docx") | Some("xls") | Some("xlsx") | Some("ppt") | Some("pptx")
        | Some("pdf") | Some("odt") | Some("ods") | Some("odp") | Some("zip") => {
            "binary_or_office"
        }
        _ => "text_direct",
    }
}

pub fn preflight_file_task(ctx: &ToolContext, path: &str) -> Result<FileTaskCaps> {
    let resolved = ctx.check_path(path)?;
    let exists = resolved.exists();
    let extension = resolved
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_string());

    let (read_mode, reason) = if exists {
        (read_mode_from_extension(&resolved).to_string(), None)
    } else {
        ("missing".to_string(), Some("file not found".to_string()))
    };

    Ok(FileTaskCaps {
        requested_path: Some(PathBuf::from(path)),
        resolved_path: Some(resolved),
        exists,
        extension,
        read_mode: Some(read_mode),
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::preflight_file_task;
    use crate::agent::ToolContext;
    use tempfile::tempdir;

    #[test]
    fn preflight_marks_txt_files_as_text_direct() {
        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("note.txt");
        std::fs::write(&file_path, "hello").expect("write text file");

        let ctx = ToolContext {
            work_dir: Some(dir.path().to_path_buf()),
            ..Default::default()
        };

        let caps = preflight_file_task(&ctx, "note.txt").expect("preflight");
        assert_eq!(caps.read_mode.as_deref(), Some("text_direct"));
        assert!(caps.exists);
        assert_eq!(caps.extension.as_deref(), Some("txt"));
        assert_eq!(caps.resolved_path.as_ref(), Some(&file_path));
    }

    #[test]
    fn preflight_marks_docx_files_as_binary_or_office() {
        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("report.docx");
        std::fs::write(&file_path, "fake docx").expect("write docx file");

        let ctx = ToolContext {
            work_dir: Some(dir.path().to_path_buf()),
            ..Default::default()
        };

        let caps = preflight_file_task(&ctx, "report.docx").expect("preflight");
        assert_eq!(caps.read_mode.as_deref(), Some("binary_or_office"));
        assert!(caps.exists);
        assert_eq!(caps.extension.as_deref(), Some("docx"));
        assert_eq!(caps.resolved_path.as_ref(), Some(&file_path));
    }

    #[test]
    fn preflight_marks_missing_files_as_missing() {
        let dir = tempdir().expect("create temp dir");
        let ctx = ToolContext {
            work_dir: Some(dir.path().to_path_buf()),
            ..Default::default()
        };

        let caps = preflight_file_task(&ctx, "missing.txt").expect("preflight");
        assert_eq!(caps.read_mode.as_deref(), Some("missing"));
        assert!(!caps.exists);
        assert_eq!(caps.extension.as_deref(), Some("txt"));
        assert_eq!(caps.reason.as_deref(), Some("file not found"));
    }
}
