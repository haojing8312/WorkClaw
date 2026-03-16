use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserStageHints {
    pub cover_filled: bool,
    pub title_filled: bool,
    pub body_segment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserProgressSnapshot {
    pub url: String,
    pub title: String,
    pub page_signature: String,
    pub facts_signature: String,
    pub stage_hints: BrowserStageHints,
}

impl BrowserProgressSnapshot {
    pub fn new(
        url: impl Into<String>,
        title: impl Into<String>,
        page_signature: impl Into<String>,
        facts_signature: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            title: title.into(),
            page_signature: page_signature.into(),
            facts_signature: facts_signature.into(),
            stage_hints: BrowserStageHints::default(),
        }
    }

    pub fn is_same_state_as(&self, other: &Self) -> bool {
        self.url == other.url
            && self.title == other.title
            && self.page_signature == other.page_signature
            && self.facts_signature == other.facts_signature
            && self.stage_hints == other.stage_hints
    }

    pub fn progress_signature(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.url, self.title, self.page_signature, self.facts_signature
        )
    }

    pub fn last_completed_step(&self) -> Option<String> {
        if self.stage_hints.body_segment_count > 0 {
            return Some("已填写正文".to_string());
        }
        if self.stage_hints.title_filled {
            return Some("已填写标题".to_string());
        }
        if self.stage_hints.cover_filled {
            return Some("已填写封面标题".to_string());
        }
        None
    }

    pub fn from_tool_output(tool_name: &str, output: &str) -> Option<Self> {
        if !tool_name
            .trim()
            .to_ascii_lowercase()
            .starts_with("browser_")
        {
            return None;
        }
        let parsed: Value = serde_json::from_str(output).ok()?;
        let url = parsed.get("url")?.as_str()?.trim().to_string();
        if url.is_empty() {
            return None;
        }
        let title = parsed
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let page_signature = parsed
            .get("page_signature")
            .or_else(|| parsed.get("interactive_hash"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| hash_json_value(parsed.get("interactive_elements")));
        let facts_value = parsed.get("facts").cloned().unwrap_or(Value::Null);
        let facts_signature = parsed
            .get("facts_signature")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| hash_json_value(Some(&facts_value)));
        let stage_hints = BrowserStageHints::from_value(Some(&facts_value));

        Some(Self {
            url,
            title,
            page_signature,
            facts_signature,
            stage_hints,
        })
    }
}

impl BrowserStageHints {
    fn from_value(value: Option<&Value>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };
        Self {
            cover_filled: value
                .get("cover_filled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            title_filled: value
                .get("title_filled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            body_segment_count: value
                .get("body_segment_count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        }
    }
}

fn hash_json_value(value: Option<&Value>) -> String {
    let raw = value
        .and_then(|item| serde_json::to_string(item).ok())
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_progress_marks_unchanged_page_signature_as_no_progress() {
        let snapshot_a = BrowserProgressSnapshot::new("https://x.com", "发布", "hash-1", "facts-1");
        let snapshot_b = BrowserProgressSnapshot::new("https://x.com", "发布", "hash-1", "facts-1");

        assert!(snapshot_b.is_same_state_as(&snapshot_a));
    }

    #[test]
    fn browser_progress_extracts_last_completed_step_from_stage_hints() {
        let snapshot = BrowserProgressSnapshot {
            url: "https://x.com".to_string(),
            title: "发布".to_string(),
            page_signature: "hash-1".to_string(),
            facts_signature: "facts-1".to_string(),
            stage_hints: BrowserStageHints {
                cover_filled: true,
                title_filled: false,
                body_segment_count: 0,
            },
        };

        assert_eq!(
            snapshot.last_completed_step().as_deref(),
            Some("已填写封面标题")
        );
    }

    #[test]
    fn browser_progress_parses_snapshot_from_browser_tool_output() {
        let snapshot = BrowserProgressSnapshot::from_tool_output(
            "browser_snapshot",
            r#"{"url":"https://x.com","title":"发布","page_signature":"page-hash","facts":{"cover_filled":true,"title_filled":true,"body_segment_count":1}}"#,
        )
        .expect("browser progress snapshot");

        assert_eq!(snapshot.page_signature, "page-hash");
        assert_eq!(
            snapshot.last_completed_step().as_deref(),
            Some("已填写正文")
        );
    }
}
