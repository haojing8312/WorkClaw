use crate::agent::types::{Tool, ToolContext};
use crate::commands::clawhub::{download_github_skill_repo_to_dir, workspace_import_base_dir};
use anyhow::{anyhow, Result};
use runtime_executor_core::truncate_tool_output;
use serde_json::{json, Value};

const OUTPUT_MAX_CHARS: usize = 30_000;

pub struct GithubRepoDownloadTool;

impl GithubRepoDownloadTool {
    pub fn new() -> Self {
        Self
    }

    fn block_on<T, F>(&self, fut: F) -> Result<T>
    where
        F: std::future::Future<Output = std::result::Result<T, String>>,
    {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| anyhow!("构建运行时失败: {}", e))?;
        rt.block_on(fut).map_err(|e| anyhow!(e))
    }
}

impl Tool for GithubRepoDownloadTool {
    fn name(&self) -> &str {
        "github_repo_download"
    }

    fn description(&self) -> &str {
        "将 GitHub 技能仓库下载到当前工作目录下的 .workclaw-imports，并返回本地目录路径与检测到的技能目录。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "repo_url": {
                    "type": "string",
                    "description": "GitHub 仓库地址"
                },
                "repo_slug": {
                    "type": "string",
                    "description": "可选：仓库短名，例如 obra/superpowers"
                }
            },
            "required": ["repo_url"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let repo_url = input["repo_url"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 repo_url 参数"))?
            .trim()
            .to_string();
        if repo_url.is_empty() {
            return Err(anyhow!("repo_url 不能为空"));
        }

        let workspace = ctx
            .work_dir
            .as_ref()
            .ok_or_else(|| anyhow!("当前会话未设置工作目录，无法下载到工作空间"))?
            .to_string_lossy()
            .to_string();
        let repo_slug = input["repo_slug"].as_str().unwrap_or("").trim().to_string();
        let base_dir = workspace_import_base_dir(&workspace);

        let result = self.block_on(download_github_skill_repo_to_dir(
            &repo_url, &repo_slug, &base_dir,
        ))?;

        let payload = json!({
            "source": "github",
            "repo_url": repo_url,
            "repo_slug": repo_slug,
            "workspace": workspace,
            "repo_dir": result.repo_dir,
            "detected_skills": result.detected_skills,
        });
        let rendered =
            serde_json::to_string_pretty(&payload).map_err(|e| anyhow!("序列化结果失败: {}", e))?;
        Ok(truncate_tool_output(&rendered, OUTPUT_MAX_CHARS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use tempfile::tempdir;
    use zip::write::FileOptions;

    fn build_skill_repo_zip() -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            let options = FileOptions::default();
            writer
                .add_directory("repo-main/skills/brainstorming/", options)
                .expect("add brainstorming dir");
            writer
                .start_file("repo-main/skills/brainstorming/SKILL.md", options)
                .expect("start brainstorming skill");
            writer
                .write_all(b"---\nname: brainstorming\n---\n")
                .expect("write brainstorming skill");
            writer.finish().expect("finish zip");
        }
        cursor.into_inner()
    }

    fn spawn_download_server(zip_bytes: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("local addr");
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 16 * 1024];
            let _ = socket.read(&mut buf).expect("read request");
            let headers = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/zip\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                zip_bytes.len()
            );
            socket.write_all(headers.as_bytes()).expect("write headers");
            socket.write_all(&zip_bytes).expect("write body");
        });
        (format!("http://{}", addr), handle)
    }

    #[test]
    fn github_repo_download_tool_downloads_into_workspace_and_returns_json() {
        let workspace = tempdir().expect("workspace tempdir");
        let (base_url, server) = spawn_download_server(build_skill_repo_zip());
        std::env::set_var("CLAWHUB_API_BASE", &base_url);

        let tool = GithubRepoDownloadTool::new();
        let output = tool
            .execute(
                json!({
                    "repo_url": "https://github.com/obra/superpowers",
                    "repo_slug": "obra/superpowers"
                }),
                &ToolContext {
                    work_dir: Some(workspace.path().to_path_buf()),
                    path_access: Default::default(),
                    allowed_tools: None,
                    session_id: None,
                    task_temp_dir: None,
                    execution_caps: None,
                    file_task_caps: None,
                },
            )
            .expect("tool execution should succeed");

        server.join().expect("server join");
        let payload: Value = serde_json::from_str(&output).expect("valid json output");
        let repo_dir = payload["repo_dir"].as_str().expect("repo_dir as str");
        let detected = payload["detected_skills"]
            .as_array()
            .expect("detected_skills as array");

        assert_eq!(payload["source"], "github");
        assert_eq!(
            payload["workspace"].as_str(),
            Some(workspace.path().to_string_lossy().as_ref())
        );
        assert!(
            repo_dir.contains(".workclaw-imports"),
            "repo_dir should be inside workspace imports: {repo_dir}"
        );
        assert!(
            repo_dir.starts_with(workspace.path().to_string_lossy().as_ref()),
            "repo_dir should stay within workspace: {repo_dir}"
        );
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0]["name"], "brainstorming");

        std::env::remove_var("CLAWHUB_API_BASE");
    }

    #[test]
    fn github_repo_download_requires_workspace() {
        let tool = GithubRepoDownloadTool::new();
        let err = tool
            .execute(
                json!({
                    "repo_url": "https://github.com/obra/superpowers"
                }),
                &ToolContext {
                    work_dir: None,
                    path_access: Default::default(),
                    allowed_tools: None,
                    session_id: None,
                    task_temp_dir: None,
                    execution_caps: None,
                    file_task_caps: None,
                },
            )
            .expect_err("workspace should be required");

        assert!(err.to_string().contains("当前会话未设置工作目录"));
    }
}
