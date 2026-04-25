use crate::agent::tool_manifest::{ToolCategory, ToolMetadata, ToolSource};
use crate::agent::{Tool, ToolContext};
use crate::commands::chat_attachment_policy::{
    PHASE_ONE_MAX_IMAGE_BYTES, PHASE_ONE_MAX_TOTAL_IMAGE_BYTES,
};
use crate::commands::chat_attachments::{
    request_image_vision_summary_with_candidate, resolve_vision_route_candidate,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::fs;
use std::path::{Path, PathBuf};

pub struct VisionAnalyzeTool {
    pool: SqlitePool,
}

impl VisionAnalyzeTool {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn block_on<T, F>(&self, fut: F) -> Result<T>
    where
        F: std::future::Future<Output = std::result::Result<T, String>>,
    {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| anyhow!("构建 vision_analyze 运行时失败: {err}"))?;
        rt.block_on(fut).map_err(|err| anyhow!(err))
    }
}

impl Tool for VisionAnalyzeTool {
    fn name(&self) -> &str {
        "vision_analyze"
    }

    fn description(&self) -> &str {
        "Analyze local workspace images with the configured vision route. Use this when the user asks to inspect, describe, compare, or summarize image files in the current workspace. Do not use browser tools or shell scripts just to read local image contents."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["workspace_image_set", "local_paths"]
                        },
                        "selection": {
                            "type": "string",
                            "enum": ["all"]
                        },
                        "paths": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["type"]
                },
                "prompt": {
                    "type": "string",
                    "description": "What to analyze or describe for the selected images."
                },
                "batch_size": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 8,
                    "default": 4
                }
            },
            "required": ["target", "prompt"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let prompt = input
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("prompt is required"))?;
        let target = input
            .get("target")
            .ok_or_else(|| anyhow!("target is required"))?;
        let target_type = target
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("target.type is required"))?;

        let paths = match target_type {
            "workspace_image_set" => collect_workspace_images(ctx)?,
            "local_paths" => collect_local_paths(target, ctx)?,
            other => return Err(anyhow!("unsupported target.type: {other}")),
        };
        if paths.is_empty() {
            return Err(anyhow!(
                "VISION_NO_IMAGES: no image files matched the target"
            ));
        }

        self.block_on(analyze_image_paths(
            self.pool.clone(),
            paths,
            prompt,
            &input,
        ))
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            display_name: Some("Vision Analyze".to_string()),
            category: ToolCategory::Other,
            read_only: true,
            destructive: false,
            concurrency_safe: true,
            open_world: false,
            requires_approval: false,
            source: ToolSource::Runtime,
        }
    }
}

async fn analyze_image_paths(
    pool: SqlitePool,
    paths: Vec<PathBuf>,
    prompt: &str,
    input: &Value,
) -> std::result::Result<String, String> {
    let Some(candidate) = resolve_vision_route_candidate(&pool).await? else {
        return Err(
            "VISION_MODEL_NOT_CONFIGURED: no enabled vision route is configured".to_string(),
        );
    };
    let batch_size = input
        .get("batch_size")
        .and_then(Value::as_u64)
        .unwrap_or(4)
        .clamp(1, 8) as usize;
    let batches = build_image_batches(paths, batch_size)?;
    if batches.is_empty() {
        return Err("VISION_NO_IMAGES: no image files matched the target".to_string());
    }

    let mut sections = Vec::new();
    for (index, batch) in batches.iter().enumerate() {
        let names = batch
            .images
            .iter()
            .map(|image| image.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let batch_prompt = format!("{prompt}\n\n请按文件名逐张回答。本批图片文件名：{names}");
        let summary = request_image_vision_summary_with_candidate(
            &format!("workspace image batch {}", index + 1),
            &batch
                .images
                .iter()
                .map(|image| image.data_url.clone())
                .collect::<Vec<_>>(),
            &candidate,
            &batch_prompt,
        )
        .await?
        .unwrap_or_else(|| "视觉模型未返回可用文本。".to_string());
        sections.push(format!(
            "## Batch {}\n\nFiles: {}\n\n{}",
            index + 1,
            names,
            summary
        ));
    }

    if !batches.skipped.is_empty() {
        sections.push(format!("## Skipped\n\n{}", batches.skipped.join("\n")));
    }

    Ok(sections.join("\n\n"))
}

#[derive(Debug, Clone)]
struct ImageBatch {
    images: Vec<ImagePayload>,
}

#[derive(Debug, Clone)]
struct ImagePayload {
    name: String,
    data_url: String,
}

#[derive(Debug, Clone)]
struct ImageBatches {
    batches: Vec<ImageBatch>,
    skipped: Vec<String>,
}

impl ImageBatches {
    fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }

    fn iter(&self) -> std::slice::Iter<'_, ImageBatch> {
        self.batches.iter()
    }
}

fn build_image_batches(
    paths: Vec<PathBuf>,
    batch_size: usize,
) -> std::result::Result<ImageBatches, String> {
    let mut batches = Vec::new();
    let mut current = ImageBatch { images: Vec::new() };
    let mut current_total_bytes = 0usize;
    let mut skipped = Vec::new();

    for path in paths {
        let name = image_display_name(&path);
        let bytes = fs::read(&path).map_err(|err| format!("读取图片 {name} 失败: {err}"))?;
        if bytes.len() > PHASE_ONE_MAX_IMAGE_BYTES {
            skipped.push(format!(
                "- {name}: VISION_IMAGE_TOO_LARGE ({} bytes > {} bytes)",
                bytes.len(),
                PHASE_ONE_MAX_IMAGE_BYTES
            ));
            continue;
        }
        if !current.images.is_empty()
            && (current.images.len() >= batch_size
                || current_total_bytes.saturating_add(bytes.len())
                    > PHASE_ONE_MAX_TOTAL_IMAGE_BYTES)
        {
            batches.push(current);
            current = ImageBatch { images: Vec::new() };
            current_total_bytes = 0;
        }
        if current_total_bytes.saturating_add(bytes.len()) > PHASE_ONE_MAX_TOTAL_IMAGE_BYTES {
            skipped.push(format!(
                "- {name}: VISION_IMAGE_TOO_LARGE (batch total would exceed {} bytes)",
                PHASE_ONE_MAX_TOTAL_IMAGE_BYTES
            ));
            continue;
        }
        let mime_type = image_mime_type_for_path(&path).unwrap_or("image/png");
        let bytes_len = bytes.len();
        current.images.push(ImagePayload {
            name,
            data_url: format!("data:{mime_type};base64,{}", BASE64.encode(bytes)),
        });
        current_total_bytes = current_total_bytes.saturating_add(bytes_len);
    }

    if !current.images.is_empty() {
        batches.push(current);
    }

    Ok(ImageBatches { batches, skipped })
}

fn collect_workspace_images(ctx: &ToolContext) -> Result<Vec<PathBuf>> {
    let root = ctx
        .work_dir
        .as_ref()
        .ok_or_else(|| anyhow!("WORKDIR_REQUIRED: current workspace is not available"))?;
    let entries = fs::read_dir(root)?;
    let mut paths = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_supported_image_path(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn collect_local_paths(target: &Value, ctx: &ToolContext) -> Result<Vec<PathBuf>> {
    let values = target
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("target.paths is required for local_paths"))?;
    let mut paths = Vec::new();
    for value in values {
        let Some(path_text) = value.as_str() else {
            continue;
        };
        let path = ctx.check_path(path_text)?;
        if path.is_file() && is_supported_image_path(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn is_supported_image_path(path: &Path) -> bool {
    image_mime_type_for_path(path).is_some()
}

fn image_mime_type_for_path(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

fn image_display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::VisionAnalyzeTool;
    use crate::agent::{Tool, ToolContext};
    use serde_json::json;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn setup_vision_route_pool() -> (sqlx::SqlitePool, tempfile::TempDir) {
        let db_dir = tempfile::tempdir().expect("db temp dir");
        let db_path = db_dir.path().join("vision-route.sqlite");
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("test sqlite");
        sqlx::query(
            "CREATE TABLE routing_policies (
                capability TEXT PRIMARY KEY,
                primary_provider_id TEXT NOT NULL,
                primary_model TEXT NOT NULL,
                fallback_chain_json TEXT NOT NULL DEFAULT '[]',
                timeout_ms INTEGER NOT NULL DEFAULT 60000,
                retry_count INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER NOT NULL DEFAULT 1
            )",
        )
        .execute(&pool)
        .await
        .expect("create routing_policies");
        sqlx::query(
            "CREATE TABLE provider_configs (
                id TEXT PRIMARY KEY,
                provider_key TEXT NOT NULL,
                protocol_type TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_key_encrypted TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1
            )",
        )
        .execute(&pool)
        .await
        .expect("create provider_configs");
        sqlx::query(
            "INSERT INTO routing_policies
             (capability, primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, enabled)
             VALUES ('vision', 'provider-vision', 'mock-vision', '[]', 60000, 0, 1)",
        )
        .execute(&pool)
        .await
        .expect("insert vision route");
        sqlx::query(
            "INSERT INTO provider_configs
             (id, provider_key, protocol_type, base_url, api_key_encrypted, enabled)
             VALUES ('provider-vision', 'openai', 'openai', 'http://mock-vision-summary-success', 'sk-test', 1)",
        )
        .execute(&pool)
        .await
        .expect("insert vision provider");
        (pool, db_dir)
    }

    #[test]
    fn vision_analyze_selects_workspace_images() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("a.png"), b"png").expect("write png");
        std::fs::write(temp.path().join("b.jpg"), b"jpg").expect("write jpg");
        std::fs::write(temp.path().join("notes.txt"), b"text").expect("write text");
        let (pool, _db_dir) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(setup_vision_route_pool());
        let tool = VisionAnalyzeTool::new(pool);
        let ctx = ToolContext {
            work_dir: Some(temp.path().to_path_buf()),
            ..ToolContext::default()
        };

        let output = tool
            .execute(
                json!({
                    "target": { "type": "workspace_image_set", "selection": "all" },
                    "prompt": "Describe each image.",
                    "batch_size": 4
                }),
                &ctx,
            )
            .expect("vision analyze output");

        assert!(output.contains("MOCK_VISION_SUMMARY"));
        assert!(output.contains("a.png"));
        assert!(output.contains("b.jpg"));
    }
}
