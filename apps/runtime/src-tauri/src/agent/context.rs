use super::execution_caps::detect_execution_caps;
use super::types::ToolContext;
use anyhow::{anyhow, Result};
use std::path::PathBuf;

pub(crate) fn build_tool_context(
    session_id: Option<&str>,
    work_dir: Option<PathBuf>,
    allowed_tools: Option<&[String]>,
) -> Result<ToolContext> {
    let task_temp_dir = match session_id {
        Some(session_id) => Some(build_task_temp_dir(session_id)?),
        None => None,
    };
    Ok(ToolContext {
        work_dir,
        allowed_tools: allowed_tools.map(|tools| tools.to_vec()),
        session_id: session_id.map(str::to_string),
        task_temp_dir,
        execution_caps: Some(detect_execution_caps()),
        file_task_caps: None,
    })
}

pub(crate) fn build_task_temp_dir(session_id: &str) -> Result<PathBuf> {
    let temp_root = std::env::temp_dir();
    let session_slug: String = session_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let dir_name = format!("workclaw-task-{}", session_slug);
    let temp_dir = temp_root.join(dir_name);
    std::fs::create_dir_all(&temp_dir).map_err(|e| anyhow!("创建任务临时目录失败: {}", e))?;
    Ok(temp_dir)
}

#[cfg(test)]
mod tests {
    use super::build_tool_context;

    #[test]
    fn task_context_includes_session_and_caps() {
        let allowed_tools = vec!["read_file".to_string()];
        let ctx = build_tool_context(Some("session-abc"), None, Some(&allowed_tools))
            .expect("build context");

        assert_eq!(ctx.session_id.as_deref(), Some("session-abc"));
        assert_eq!(ctx.allowed_tools.as_deref(), Some(allowed_tools.as_slice()));
        assert!(ctx.task_temp_dir.is_some());
        assert!(ctx.execution_caps.is_some());
        assert!(ctx.file_task_caps.is_none());
    }
}
