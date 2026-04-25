use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WorkspaceImageResourceSummary {
    pub id: String,
    pub source: String,
    pub count: usize,
    pub sample_names: Vec<String>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TurnResourceContext {
    pub work_dir: Option<String>,
    pub workspace_images: Option<WorkspaceImageResourceSummary>,
}

pub(crate) fn resolve_turn_resource_context(work_dir: Option<&str>) -> TurnResourceContext {
    let Some(work_dir) = work_dir.map(str::trim).filter(|value| !value.is_empty()) else {
        return TurnResourceContext::default();
    };
    let root = Path::new(work_dir);
    TurnResourceContext {
        work_dir: Some(work_dir.to_string()),
        workspace_images: summarize_top_level_images(root),
    }
}

pub(crate) fn is_workspace_image_analysis_request(
    context: Option<&TurnResourceContext>,
    user_message: &str,
) -> bool {
    let Some(context) = context else {
        return false;
    };
    if context
        .workspace_images
        .as_ref()
        .map(|images| images.count == 0)
        .unwrap_or(true)
    {
        return false;
    }

    let message = user_message.trim().to_ascii_lowercase();
    if message.is_empty() {
        return false;
    }

    let mentions_images = [
        "图片",
        "图像",
        "截图",
        "照片",
        "这些图",
        "这些图片",
        "image",
        "images",
        "screenshot",
        "screenshots",
        "photo",
        "photos",
    ]
    .iter()
    .any(|needle| message.contains(needle));
    let asks_for_analysis = [
        "读取",
        "读一下",
        "查看",
        "看看",
        "分析",
        "描述",
        "识别",
        "内容",
        "比较",
        "read",
        "inspect",
        "analyze",
        "analyse",
        "describe",
        "recognize",
        "compare",
        "content",
    ]
    .iter()
    .any(|needle| message.contains(needle));

    mentions_images && asks_for_analysis
}

fn summarize_top_level_images(root: &Path) -> Option<WorkspaceImageResourceSummary> {
    let entries = fs::read_dir(root).ok()?;
    let mut files = Vec::<(String, u64)>::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !is_supported_image_path(&path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        files.push((name, size));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    if files.is_empty() {
        return None;
    }
    let total_bytes = files.iter().map(|(_, size)| *size).sum();
    let sample_names = files.iter().take(5).map(|(name, _)| name.clone()).collect();
    Some(WorkspaceImageResourceSummary {
        id: "workspace.images".to_string(),
        source: "workspace_top_level".to_string(),
        count: files.len(),
        sample_names,
        total_bytes,
    })
}

fn is_supported_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{is_workspace_image_analysis_request, resolve_turn_resource_context};
    use std::fs;

    #[test]
    fn resolve_turn_resource_context_summarizes_top_level_images() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("b.jpg"), b"bb").expect("write jpg");
        fs::write(temp.path().join("a.png"), b"a").expect("write png");
        fs::write(temp.path().join("notes.md"), b"text").expect("write text");

        let context = resolve_turn_resource_context(temp.path().to_str());
        let images = context.workspace_images.expect("workspace images");

        assert_eq!(images.id, "workspace.images");
        assert_eq!(images.count, 2);
        assert_eq!(images.sample_names, vec!["a.png", "b.jpg"]);
        assert_eq!(images.total_bytes, 3);
    }

    #[test]
    fn workspace_image_analysis_request_requires_images_and_intent() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("a.png"), b"a").expect("write png");
        let context = resolve_turn_resource_context(temp.path().to_str());

        assert!(is_workspace_image_analysis_request(
            Some(&context),
            "读取这些图片，并告诉我每个图片的内容"
        ));
        assert!(!is_workspace_image_analysis_request(
            Some(&context),
            "当前工作空间里有什么"
        ));
        assert!(!is_workspace_image_analysis_request(None, "分析这些图片"));
    }
}
