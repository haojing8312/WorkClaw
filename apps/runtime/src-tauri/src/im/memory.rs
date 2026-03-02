use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub category: String,
    pub content: String,
    pub confirmed: bool,
    pub source_msg_id: String,
    pub author_role: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureResult {
    pub session_written: bool,
    pub long_term_written: bool,
}

pub fn memory_paths(root: &Path, thread_id: &str, role_id: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let daily = root.join("daily").join(format!("{}.md", date));
    let session = root.join("sessions").join(format!("{}.md", thread_id));
    let role = root.join("roles").join(role_id).join("MEMORY.md");
    let org = root.join("org").join("CASEBOOK.md");
    (daily, session, role, org)
}

pub fn capture_entry(root: &Path, thread_id: &str, role_id: &str, entry: &MemoryEntry) -> Result<CaptureResult> {
    let (daily, session, role, org) = memory_paths(root, thread_id, role_id);
    ensure_parent_dirs(&[&daily, &session, &role, &org])?;

    let now = Utc::now().to_rfc3339();
    let line = format!(
        "- [{}] [{}] {} (source={}, role={}, confidence={:.2})\n",
        now,
        entry.category,
        entry.content,
        entry.source_msg_id,
        entry.author_role,
        entry.confidence
    );

    append_line(&daily, &line)?;
    append_line(&session, &line)?;

    let long_term_allowed = entry.confirmed && entry.confidence >= 0.7;
    if long_term_allowed {
        append_line(&role, &line)?;
        if entry.category == "fact" || entry.category == "decision" || entry.category == "rule" {
            append_line(&org, &line)?;
        }
    }

    Ok(CaptureResult {
        session_written: true,
        long_term_written: long_term_allowed,
    })
}

pub fn recall_context(root: &Path, thread_id: &str, role_id: &str) -> Result<String> {
    let (_daily, session, role, org) = memory_paths(root, thread_id, role_id);
    let mut parts = Vec::new();

    for path in [&role, &session, &org] {
        if path.exists() {
            parts.push(fs::read_to_string(path)?);
        }
    }

    Ok(parts.join("\n\n"))
}

fn ensure_parent_dirs(paths: &[&PathBuf]) -> Result<()> {
    for path in paths {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn append_line(path: &Path, line: &str) -> Result<()> {
    if path.exists() {
        let mut content = fs::read_to_string(path)?;
        content.push_str(line);
        fs::write(path, content)?;
    } else {
        fs::write(path, line)?;
    }
    Ok(())
}

