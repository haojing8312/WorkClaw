use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use zip::ZipArchive;

const MAX_PREVIEW_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorkspaceFileEntry {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorkspaceFilePreview {
    pub path: String,
    pub kind: String,
    pub source: Option<String>,
    pub size: u64,
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn is_text_extension(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "md" | "markdown" | "json" | "js" | "ts" | "tsx" | "jsx" | "css" | "html" | "htm" | "xml" | "yml" | "yaml"
    )
}

fn preview_kind_for_path(path: &Path) -> String {
    match path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "docx" => "docx".to_string(),
        "md" | "markdown" => "markdown".to_string(),
        "html" | "htm" => "html".to_string(),
        ext if is_text_extension(ext) => "text".to_string(),
        _ => "binary".to_string(),
    }
}

fn decode_xml_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn extract_docx_text(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("读取 DOCX 文件失败: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("解析 DOCX 文件失败: {}", e))?;
    let mut document = archive
        .by_name("word/document.xml")
        .map_err(|e| format!("读取 DOCX 文档内容失败: {}", e))?;
    let mut xml = String::new();
    document
        .read_to_string(&mut xml)
        .map_err(|e| format!("读取 DOCX 文本失败: {}", e))?;

    let with_breaks = xml
        .replace("<w:tab/>", "\t")
        .replace("<w:tab />", "\t")
        .replace("<w:br/>", "\n")
        .replace("<w:br />", "\n")
        .replace("</w:p>", "\n");

    let mut plain = String::with_capacity(with_breaks.len());
    let mut in_tag = false;
    for ch in with_breaks.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => plain.push(ch),
            _ => {}
        }
    }

    let decoded = decode_xml_entities(&plain);
    let lines = decoded
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    Ok(lines.join("\n"))
}

fn canonicalize_workspace(workspace: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(workspace);
    if !root.exists() {
        return Err("工作空间不存在".to_string());
    }
    if !root.is_dir() {
        return Err("工作空间不是目录".to_string());
    }
    root.canonicalize().map_err(|e| format!("解析工作空间失败: {}", e))
}

fn ensure_relative_path(relative_path: &str) -> Result<(), String> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err("仅允许相对路径".to_string());
    }
    if path.components().any(|component| matches!(component, Component::ParentDir)) {
        return Err("不允许越出工作空间".to_string());
    }
    Ok(())
}

fn collect_entries(root: &Path, current: &Path, entries: &mut Vec<WorkspaceFileEntry>) -> Result<(), String> {
    let mut children: Vec<_> = fs::read_dir(current)
        .map_err(|e| format!("读取目录失败: {}", e))?
        .filter_map(|item| item.ok())
        .collect();
    children.sort_by_key(|entry| entry.file_name().to_string_lossy().to_ascii_lowercase());

    for entry in children {
        let path = entry.path();
        let metadata = entry.metadata().map_err(|e| format!("读取文件信息失败: {}", e))?;
        let relative = path
            .strip_prefix(root)
            .map_err(|e| format!("解析相对路径失败: {}", e))?;
        let relative_text = normalize_relative_path(relative);
        let name = entry.file_name().to_string_lossy().to_string();

        if metadata.is_dir() {
            entries.push(WorkspaceFileEntry {
                path: relative_text.clone(),
                name,
                size: 0,
                kind: "directory".to_string(),
            });
            collect_entries(root, &path, entries)?;
        } else {
            entries.push(WorkspaceFileEntry {
                path: relative_text,
                name,
                size: metadata.len(),
                kind: preview_kind_for_path(&path),
            });
        }
    }

    Ok(())
}

pub fn list_workspace_files_within(workspace: &str) -> Result<Vec<WorkspaceFileEntry>, String> {
    let root = canonicalize_workspace(workspace)?;
    let mut entries = Vec::new();
    collect_entries(&root, &root, &mut entries)?;
    Ok(entries)
}

pub fn read_workspace_file_preview_within(
    workspace: &str,
    relative_path: &str,
) -> Result<WorkspaceFilePreview, String> {
    ensure_relative_path(relative_path)?;
    let root = canonicalize_workspace(workspace)?;
    let full_path = root.join(relative_path);
    let canonical = full_path
        .canonicalize()
        .map_err(|e| format!("读取文件失败: {}", e))?;
    if !canonical.starts_with(&root) {
        return Err("不允许越出工作空间".to_string());
    }

    let metadata = fs::metadata(&canonical).map_err(|e| format!("读取文件信息失败: {}", e))?;
    if metadata.is_dir() {
        return Ok(WorkspaceFilePreview {
            path: relative_path.to_string(),
            kind: "directory".to_string(),
            source: None,
            size: 0,
        });
    }

    let kind = preview_kind_for_path(&canonical);
    let source = if kind == "binary" {
        None
    } else if kind == "docx" {
        Some(extract_docx_text(&canonical)?)
    } else {
        let bytes = fs::read(&canonical).map_err(|e| format!("读取文件失败: {}", e))?;
        let truncated = &bytes[..bytes.len().min(MAX_PREVIEW_BYTES)];
        Some(String::from_utf8_lossy(truncated).to_string())
    };

    Ok(WorkspaceFilePreview {
        path: relative_path.to_string(),
        kind,
        source,
        size: metadata.len(),
    })
}

#[tauri::command]
pub async fn list_workspace_files(workspace: String) -> Result<Vec<WorkspaceFileEntry>, String> {
    list_workspace_files_within(&workspace)
}

#[tauri::command]
pub async fn read_workspace_file_preview(
    workspace: String,
    relative_path: String,
) -> Result<WorkspaceFilePreview, String> {
    read_workspace_file_preview_within(&workspace, &relative_path)
}
