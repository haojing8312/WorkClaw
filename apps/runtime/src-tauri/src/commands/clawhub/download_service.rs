use chrono::Utc;
use reqwest::Client;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use tauri::AppHandle;
use walkdir::WalkDir;
use zip::ZipArchive;

use super::types::{DiscoveredSkillDir, GithubRepoDownloadResult};

fn sanitize_zip_entry_path(name: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for comp in Path::new(name).components() {
        match comp {
            Component::Normal(seg) => out.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

pub(crate) fn extract_skill_md_from_zip_bytes(bytes: &[u8]) -> Result<String, String> {
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("解压失败: {}", e))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let path = Path::new(entry.name());
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if file_name.eq_ignore_ascii_case("SKILL.md") {
            let mut content = String::new();
            std::io::Read::read_to_string(&mut entry, &mut content)
                .map_err(|e| format!("读取远端 SKILL.md 失败: {}", e))?;
            return Ok(content);
        }
    }
    Err("下载包中未找到 SKILL.md".to_string())
}

async fn download_skill_zip_bytes(client: &Client, repo_url: &str) -> Result<Vec<u8>, String> {
    if super::prefer_proxy_download_for_github_archives() {
        let base = super::clawhub_base_url();
        let download_url = format!(
            "{}/api/v1/download?url={}",
            base,
            urlencoding::encode(repo_url.trim())
        );
        let resp = client
            .get(&download_url)
            .send()
            .await
            .map_err(|e| format!("下载失败: {}", e))?;
        if resp.status().is_success() {
            let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
            if !bytes.is_empty() {
                return Ok(bytes.to_vec());
            }
        }
    }

    if let Some(urls) = build_github_archive_urls(repo_url) {
        for url in urls {
            let resp = client
                .get(&url)
                .header("User-Agent", "WorkClaw/1.0")
                .send()
                .await
                .map_err(|e| format!("下载失败: {}", e))?;
            if !resp.status().is_success() {
                continue;
            }
            let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
            if !bytes.is_empty() {
                return Ok(bytes.to_vec());
            }
        }
    }

    let base = super::clawhub_base_url();
    let download_url = format!(
        "{}/api/v1/download?url={}",
        base,
        urlencoding::encode(repo_url.trim())
    );
    let resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("下载内容为空".to_string());
    }
    Ok(bytes.to_vec())
}

async fn download_skillhub_slug_zip_bytes(client: &Client, slug: &str) -> Result<Vec<u8>, String> {
    let clean_slug = slug.trim();
    if clean_slug.is_empty() {
        return Err("slug 不能为空".to_string());
    }
    let download_url = super::build_skillhub_download_url(clean_slug);
    let resp = client
        .get(&download_url)
        .header("User-Agent", "WorkClaw/1.0")
        .send()
        .await
        .map_err(|e| format!("SkillHub 下载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("SkillHub 下载失败: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("SkillHub 下载内容为空".to_string());
    }
    Ok(bytes.to_vec())
}

pub(crate) async fn download_skill_bytes_with_fallback(
    client: &Client,
    slug: &str,
    github_url: Option<String>,
) -> Result<Vec<u8>, String> {
    match download_skillhub_slug_zip_bytes(client, slug).await {
        Ok(bytes) => Ok(bytes),
        Err(skillhub_error) => {
            let repo_url = super::resolve_repo_url(client, slug, github_url).await?;
            download_skill_zip_bytes(client, &repo_url)
                .await
                .map_err(|fallback_error| {
                    format!(
                        "{}；ClawHub/GitHub 回退失败: {}",
                        skillhub_error, fallback_error
                    )
                })
        }
    }
}

pub(crate) fn build_github_archive_urls(repo_url: &str) -> Option<Vec<String>> {
    let trimmed = repo_url.trim().trim_end_matches('/');
    let without_git = trimmed.trim_end_matches(".git");
    let prefix = "https://github.com/";
    if !without_git.starts_with(prefix) {
        return None;
    }
    let path = &without_git[prefix.len()..];
    let mut parts = path.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    let base = format!("https://github.com/{}/{}", owner, repo);
    Some(vec![
        format!("{}/archive/refs/heads/main.zip", base),
        format!("{}/archive/refs/heads/master.zip", base),
    ])
}

pub(crate) fn find_skill_roots(extract_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for entry in WalkDir::new(extract_dir).min_depth(1).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry
            .path()
            .file_name()
            .and_then(|f| f.to_str())
            .map(|n| n.eq_ignore_ascii_case("SKILL.md"))
            .unwrap_or(false)
        {
            if let Some(parent) = entry.path().parent() {
                roots.push(parent.to_path_buf());
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

pub(crate) fn find_skill_root(extract_dir: &Path) -> Option<PathBuf> {
    find_skill_roots(extract_dir).into_iter().next()
}

fn describe_skill_dir(dir_path: &Path) -> DiscoveredSkillDir {
    let name = std::fs::read_to_string(dir_path.join("SKILL.md"))
        .ok()
        .and_then(|content| crate::agent::skill_config::SkillConfig::parse(&content).name)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            dir_path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "unnamed-skill".to_string());
    DiscoveredSkillDir {
        name,
        dir_path: dir_path.to_string_lossy().to_string(),
    }
}

fn build_github_repo_key(repo_url: &str, repo_slug: &str) -> String {
    super::sanitize_slug_stable(if repo_slug.trim().is_empty() {
        repo_url
            .rsplit('/')
            .next()
            .unwrap_or("github-skill")
            .trim_end_matches(".git")
    } else {
        repo_slug.trim()
    })
}

pub(crate) fn extract_github_repo_archive(
    bytes: &[u8],
    base_dir: &Path,
    repo_key: &str,
) -> Result<GithubRepoDownloadResult, String> {
    std::fs::create_dir_all(base_dir).map_err(|e| e.to_string())?;
    let extract_dir = base_dir.join(format!("{}-{}", repo_key, Utc::now().timestamp_millis()));
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    extract_zip_to_dir(bytes, &extract_dir)?;

    let roots = find_skill_roots(&extract_dir);
    if roots.is_empty() {
        return Err("未在 GitHub 仓库中发现可导入的 SKILL.md".to_string());
    }

    Ok(GithubRepoDownloadResult {
        repo_dir: extract_dir.to_string_lossy().to_string(),
        detected_skills: roots.iter().map(|dir| describe_skill_dir(dir)).collect(),
    })
}

pub async fn download_github_skill_repo_to_workspace(
    app: &AppHandle,
    repo_url: &str,
    repo_slug: &str,
    workspace: Option<&str>,
) -> Result<GithubRepoDownloadResult, String> {
    let clean_repo_url = repo_url.trim().to_string();
    if clean_repo_url.is_empty() {
        return Err("repo_url 不能为空".to_string());
    }

    let repo_key = build_github_repo_key(&clean_repo_url, repo_slug);
    let client = Client::new();
    let bytes = download_skill_zip_bytes(&client, &clean_repo_url).await?;
    let base_dir = super::default_workspace_import_base_dir(app, workspace);
    extract_github_repo_archive(&bytes, &base_dir, &repo_key)
}

pub async fn download_github_skill_repo_to_dir(
    repo_url: &str,
    repo_slug: &str,
    base_dir: &Path,
) -> Result<GithubRepoDownloadResult, String> {
    let clean_repo_url = repo_url.trim().to_string();
    if clean_repo_url.is_empty() {
        return Err("repo_url 不能为空".to_string());
    }

    let repo_key = build_github_repo_key(&clean_repo_url, repo_slug);
    let client = Client::new();
    let bytes = download_skill_zip_bytes(&client, &clean_repo_url).await?;
    extract_github_repo_archive(&bytes, base_dir, &repo_key)
}

pub(crate) fn extract_zip_to_dir(bytes: &[u8], extract_dir: &Path) -> Result<(), String> {
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("解压失败: {}", e))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel_path) = sanitize_zip_entry_path(entry.name()) else {
            continue;
        };
        let out_path = extract_dir.join(rel_path);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut outfile = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use zip::write::FileOptions;

    fn build_skill_repo_zip() -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            let options = FileOptions::default();
            writer
                .add_directory("repo-main/skills/brainstorming/", options)
                .expect("add brainstorming dir");
            writer
                .start_file("repo-main/skills/brainstorming/SKILL.md", options)
                .expect("start brainstorming skill");
            use std::io::Write as _;
            writer
                .write_all(b"---\nname: brainstorming\n---\n")
                .expect("write brainstorming skill");
            writer
                .add_directory("repo-main/skills/debugging/", options)
                .expect("add debugging dir");
            writer
                .start_file("repo-main/skills/debugging/SKILL.md", options)
                .expect("start debugging skill");
            writer
                .write_all(b"---\nname: debugging\n---\n")
                .expect("write debugging skill");
            writer.finish().expect("finish zip");
        }
        cursor.into_inner()
    }

    #[test]
    fn find_skill_roots_returns_all_matching_skill_directories() {
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("repo-a/skills/brainstorming")).expect("mkdir first");
        std::fs::create_dir_all(root.join("repo-a/skills/debugging")).expect("mkdir second");
        std::fs::write(
            root.join("repo-a/skills/brainstorming/SKILL.md"),
            "---\nname: brainstorming\n---\n",
        )
        .expect("write first skill");
        std::fs::write(
            root.join("repo-a/skills/debugging/SKILL.md"),
            "---\nname: debugging\n---\n",
        )
        .expect("write second skill");

        let roots = find_skill_roots(root);

        assert_eq!(roots.len(), 2);
        assert!(roots.iter().any(|path| path.ends_with("brainstorming")));
        assert!(roots.iter().any(|path| path.ends_with("debugging")));
    }

    #[test]
    fn extract_github_repo_archive_returns_repo_dir_and_detected_skills() {
        let tmp = tempdir().expect("tempdir");
        let zip_bytes = build_skill_repo_zip();

        let result =
            extract_github_repo_archive(&zip_bytes, tmp.path(), "superpowers").expect("extract");

        assert!(result.repo_dir.contains("superpowers-"));
        assert_eq!(result.detected_skills.len(), 2);
        assert!(result
            .detected_skills
            .iter()
            .any(|skill| skill.name.eq_ignore_ascii_case("brainstorming")));
        assert!(result
            .detected_skills
            .iter()
            .any(|skill| skill.name.eq_ignore_ascii_case("debugging")));
    }

    #[test]
    fn build_github_repo_key_prefers_slug_and_strips_git_suffix_from_url() {
        assert_eq!(
            build_github_repo_key(
                "https://github.com/obra/superpowers.git",
                "obra/superpowers"
            ),
            "obra-superpowers"
        );
        assert_eq!(
            build_github_repo_key("https://github.com/obra/superpowers.git", ""),
            "superpowers"
        );
    }

    #[test]
    fn build_github_archive_urls_supports_standard_repo_urls() {
        assert_eq!(
            build_github_archive_urls("https://github.com/pskoett/self-improving-agent"),
            Some(vec![
                "https://github.com/pskoett/self-improving-agent/archive/refs/heads/main.zip"
                    .to_string(),
                "https://github.com/pskoett/self-improving-agent/archive/refs/heads/master.zip"
                    .to_string(),
            ])
        );
        assert_eq!(
            build_github_archive_urls("https://github.com/obra/superpowers.git"),
            Some(vec![
                "https://github.com/obra/superpowers/archive/refs/heads/main.zip".to_string(),
                "https://github.com/obra/superpowers/archive/refs/heads/master.zip".to_string(),
            ])
        );
    }

    #[test]
    fn build_github_archive_urls_rejects_clawhub_skill_pages() {
        assert_eq!(
            build_github_archive_urls("https://www.clawhub.ai/skills/self-improving-agent"),
            None
        );
    }

    #[test]
    fn build_skillhub_download_url_encodes_slug() {
        assert_eq!(
            super::super::build_skillhub_download_url("self-improving-agent"),
            "https://lightmake.site/api/v1/download?slug=self-improving-agent"
        );
        assert_eq!(
            super::super::build_skillhub_download_url("a skill"),
            "https://lightmake.site/api/v1/download?slug=a%20skill"
        );
    }

    #[test]
    fn custom_clawhub_base_prefers_proxy_download_for_github_archives() {
        std::env::set_var("CLAWHUB_API_BASE", "http://127.0.0.1:8787");
        let preference = super::super::prefer_proxy_download_for_github_archives();
        std::env::remove_var("CLAWHUB_API_BASE");

        assert!(preference);
    }
}
