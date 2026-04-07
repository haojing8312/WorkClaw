use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub(crate) fn sanitize_slug(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        format!("expert-skill-{:x}", hasher.finish())
    } else {
        trimmed
    }
}

fn sanitize_slug_stable(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "skill".to_string()
    } else {
        trimmed
    }
}

pub(crate) fn build_local_skill_id(seed: &str, dir_path: &str) -> String {
    let slug = sanitize_slug_stable(seed);
    if slug != "skill" {
        return format!("local-{}", slug);
    }

    let mut hasher = DefaultHasher::new();
    dir_path.hash(&mut hasher);
    format!("local-skill-{:x}", hasher.finish())
}

pub(crate) fn default_skill_base_dir() -> PathBuf {
    crate::runtime_paths::resolve_runtime_root().join("skills")
}

pub(crate) fn normalize_skill_description(description: &str, when_to_use: &str) -> String {
    let trimmed = description.trim();
    let fallback = format!("Use when {}", when_to_use.trim());

    if trimmed.is_empty() {
        return fallback;
    }

    if trimmed.to_ascii_lowercase().starts_with("use when") {
        trimmed.to_string()
    } else {
        format!("Use when {}", trimmed)
    }
}

pub(crate) fn render_local_skill_markdown(
    name: &str,
    description: &str,
    when_to_use: &str,
) -> String {
    let normalized_description = normalize_skill_description(description, when_to_use);
    let overview = if description.trim().is_empty() {
        "该技能用于稳定处理特定任务流程。"
    } else {
        description.trim()
    };

    crate::builtin_skills::local_skill_template_markdown()
        .replace("{{SKILL_NAME}}", name)
        .replace("{{SKILL_DESCRIPTION}}", &normalized_description)
        .replace("{{SKILL_TITLE}}", name)
        .replace("{{SKILL_OVERVIEW}}", overview)
        .replace("{{SKILL_WHEN_TO_USE}}", when_to_use.trim())
}

pub(crate) fn read_skill_markdown_with_fallback(dir_path: &str) -> Result<String, String> {
    let dir = Path::new(dir_path);
    let upper = dir.join("SKILL.md");
    if upper.exists() {
        return std::fs::read_to_string(upper).map_err(|e| format!("无法读取 SKILL.md: {}", e));
    }
    let lower = dir.join("skill.md");
    if lower.exists() {
        return std::fs::read_to_string(lower).map_err(|e| format!("无法读取 skill.md: {}", e));
    }
    Err("无法读取 SKILL.md: 未找到 SKILL.md".to_string())
}

pub(crate) fn merge_tags(base: Vec<String>, extra: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in base.into_iter().chain(extra.iter().cloned()) {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_lowercase();
        if seen.insert(key) {
            out.push(trimmed.to_string());
        }
    }
    out
}

pub(crate) fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let parts: Vec<&str> = version.trim().split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch = parts[2]
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .ok()?;
    Some((major, minor, patch))
}

pub(crate) fn compare_semver(left: &str, right: &str) -> Ordering {
    match (parse_semver(left), parse_semver(right)) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => left.cmp(right),
    }
}

pub(crate) fn extract_tag_value(tags: &[String], prefix: &str) -> Option<String> {
    let p = prefix.to_ascii_lowercase();
    tags.iter().find_map(|tag| {
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with(&p) {
            Some(tag[prefix.len()..].to_string())
        } else {
            None
        }
    })
}

pub(crate) fn normalize_display_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}
