use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use skillpack_rs::pack::parse_front_matter;
use skillpack_rs::{pack, FrontMatter, PackConfig};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;

#[derive(Debug, Serialize, Deserialize)]
pub struct SkillDirInfo {
    pub files: Vec<String>,
    pub front_matter: FrontMatter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkClawDirSummary {
    pub dir_path: String,
    pub slug: String,
    pub front_matter: FrontMatter,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndustryPackSkillEntry {
    pub slug: String,
    pub name: String,
    pub version: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndustryPackManifest {
    pub pack_id: String,
    pub name: String,
    pub version: String,
    pub industry_tag: String,
    pub created_at: String,
    pub skills: Vec<IndustryPackSkillEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpackedIndustryBundle {
    pub manifest: IndustryPackManifest,
    pub skill_dirs: Vec<String>,
}

fn is_hidden_or_excluded(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|name| name.starts_with('.') || name == "node_modules" || name == "__pycache__")
        .unwrap_or(false)
}

fn sanitize_slug(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
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

fn split_front_matter(content: &str) -> Option<(String, String)> {
    if !content.starts_with("---") {
        return None;
    }
    let rest = &content[3..];
    let end_pos = rest.find("\n---")?;
    let yaml = rest[..end_pos].trim_start_matches('\n').to_string();
    let body_start = 3 + end_pos + 4;
    let body = if body_start < content.len() {
        content[body_start..].trim_start_matches('\n').to_string()
    } else {
        String::new()
    };
    Some((yaml, body))
}

fn parse_tags_from_yaml(yaml: &str) -> Vec<String> {
    let value: Value = match serde_yaml::from_str(yaml) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let mapping = match value.as_mapping() {
        Some(m) => m,
        None => return vec![],
    };
    let tags_value = mapping.get(Value::String("tags".to_string()));
    match tags_value {
        Some(Value::Sequence(list)) => normalize_tags(
            &list
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>(),
        ),
        Some(Value::String(raw)) => normalize_tags(
            &raw.split(',')
                .map(|part| part.trim().to_string())
                .collect::<Vec<_>>(),
        ),
        _ => vec![],
    }
}

pub fn parse_skill_tags(markdown: &str) -> Vec<String> {
    split_front_matter(markdown)
        .map(|(yaml, _)| parse_tags_from_yaml(&yaml))
        .unwrap_or_default()
}

fn write_skill_tags(markdown: &str, tags: &[String]) -> Result<String, String> {
    let normalized = normalize_tags(tags);
    let tags_yaml = Value::Sequence(
        normalized
            .iter()
            .map(|tag| Value::String(tag.clone()))
            .collect::<Vec<_>>(),
    );

    let mut mapping = if let Some((yaml, _)) = split_front_matter(markdown) {
        serde_yaml::from_str::<Mapping>(&yaml).unwrap_or_default()
    } else {
        Mapping::new()
    };
    mapping.insert(Value::String("tags".to_string()), tags_yaml);

    let mut yaml_text =
        serde_yaml::to_string(&mapping).map_err(|e| format!("序列化 tags 失败: {}", e))?;
    if yaml_text.starts_with("---\n") {
        yaml_text = yaml_text.trim_start_matches("---\n").to_string();
    }
    if !yaml_text.ends_with('\n') {
        yaml_text.push('\n');
    }

    let body = split_front_matter(markdown)
        .map(|(_, body)| body)
        .unwrap_or_else(|| markdown.to_string());
    let rebuilt = if body.trim().is_empty() {
        format!("---\n{}---\n", yaml_text)
    } else {
        format!("---\n{}---\n{}\n", yaml_text, body.trim_end())
    };
    Ok(rebuilt)
}

fn collect_skill_dirs(root_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for entry in WalkDir::new(root_dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let Some(file_name) = entry.file_name().to_str() else {
            continue;
        };
        if !file_name.eq_ignore_ascii_case("SKILL.md") {
            continue;
        }
        if let Some(parent) = entry.path().parent() {
            dirs.push(parent.to_path_buf());
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

fn skill_has_markdown(dir: &Path) -> bool {
    dir.join("SKILL.md").exists() || dir.join("skill.md").exists()
}

fn read_skill_markdown(dir: &Path) -> Result<String, String> {
    let path_upper = dir.join("SKILL.md");
    let path_lower = dir.join("skill.md");
    if path_upper.exists() {
        return fs::read_to_string(path_upper).map_err(|e| format!("读取 SKILL.md 失败: {}", e));
    }
    if path_lower.exists() {
        return fs::read_to_string(path_lower).map_err(|e| format!("读取 skill.md 失败: {}", e));
    }
    Err("未找到 SKILL.md".to_string())
}

fn default_industry_pack_root() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".workclaw").join("industry-packs")
}

fn read_industry_manifest_from_zip(path: &Path) -> Result<IndustryPackManifest, String> {
    let file = fs::File::open(path).map_err(|e| format!("打开行业包失败: {}", e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("读取 zip 失败: {}", e))?;
    let mut entry = zip
        .by_name("industry-manifest.json")
        .map_err(|_| "行业包缺少 industry-manifest.json".to_string())?;
    let mut json = String::new();
    entry
        .read_to_string(&mut json)
        .map_err(|e| format!("读取行业包清单失败: {}", e))?;
    serde_json::from_str::<IndustryPackManifest>(&json)
        .map_err(|e| format!("解析行业包清单失败: {}", e))
}

fn semver_valid(version: &str) -> bool {
    let parts: Vec<&str> = version.trim().split('.').collect();
    if parts.len() < 3 {
        return false;
    }
    parts[0].parse::<u64>().is_ok()
        && parts[1].parse::<u64>().is_ok()
        && parts[2].parse::<u64>().is_ok()
}

#[tauri::command]
pub async fn read_skill_dir(dir_path: String) -> Result<SkillDirInfo, String> {
    let skill_dir = Path::new(&dir_path);
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_md_path.exists() {
        return Err("所选目录中未找到 SKILL.md 文件".to_string());
    }

    let skill_md_content =
        fs::read_to_string(&skill_md_path).map_err(|e| format!("读取 SKILL.md 失败: {}", e))?;
    let front_matter = parse_front_matter(&skill_md_content);

    let files: Vec<String> = WalkDir::new(skill_dir)
        .into_iter()
        .filter_entry(|e| {
            if e.path() == skill_dir {
                return true;
            }
            !is_hidden_or_excluded(e)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(skill_dir)
                .unwrap_or(e.path())
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();

    Ok(SkillDirInfo {
        files,
        front_matter,
    })
}

#[tauri::command]
pub async fn scan_workclaw_dirs(root_dir: String) -> Result<Vec<WorkClawDirSummary>, String> {
    let root = Path::new(&root_dir);
    if !root.exists() {
        return Err("目录不存在".to_string());
    }
    if !root.is_dir() {
        return Err("请选择目录".to_string());
    }

    let mut out = Vec::new();
    for dir in collect_skill_dirs(root) {
        if !skill_has_markdown(&dir) {
            continue;
        }
        let content = match read_skill_markdown(&dir) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let fm = parse_front_matter(&content);
        let tags = parse_skill_tags(&content);
        let slug_base = dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(sanitize_slug)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "skill".to_string());
        out.push(WorkClawDirSummary {
            dir_path: dir.to_string_lossy().to_string(),
            slug: slug_base,
            front_matter: fm,
            tags,
        });
    }

    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

#[tauri::command]
pub async fn update_skill_dir_tags(dir_path: String, tags: Vec<String>) -> Result<(), String> {
    let dir = Path::new(&dir_path);
    let skill_md_path = if dir.join("SKILL.md").exists() {
        dir.join("SKILL.md")
    } else {
        dir.join("skill.md")
    };
    if !skill_md_path.exists() {
        return Err("所选目录中未找到 SKILL.md 文件".to_string());
    }

    let content =
        fs::read_to_string(&skill_md_path).map_err(|e| format!("读取 SKILL.md 失败: {}", e))?;
    let updated = write_skill_tags(&content, &tags)?;
    fs::write(&skill_md_path, updated).map_err(|e| format!("写入 SKILL.md 失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn pack_skill(
    dir_path: String,
    name: String,
    description: String,
    version: String,
    author: String,
    username: String,
    recommended_model: String,
    output_path: String,
) -> Result<(), String> {
    let config = PackConfig {
        dir_path,
        name,
        description,
        version,
        author,
        username,
        recommended_model,
        output_path,
    };
    pack(&config).map_err(|e| format!("打包失败: {}", e))
}

#[tauri::command]
pub async fn pack_industry_bundle(
    skill_dirs: Vec<String>,
    pack_name: String,
    pack_id: String,
    version: String,
    industry_tag: String,
    output_path: String,
) -> Result<(), String> {
    if skill_dirs.is_empty() {
        return Err("请至少选择一个技能目录".to_string());
    }
    if pack_name.trim().is_empty() {
        return Err("行业包名称不能为空".to_string());
    }
    if !semver_valid(&version) {
        return Err("版本号格式不正确，请使用 1.0.0".to_string());
    }

    let normalized_pack_id = {
        let id = sanitize_slug(pack_id.trim());
        if id.is_empty() {
            sanitize_slug(pack_name.trim())
        } else {
            id
        }
    };
    if normalized_pack_id.is_empty() {
        return Err("pack_id 不能为空".to_string());
    }

    let mut used_slugs = HashSet::new();
    let mut entries: Vec<(IndustryPackSkillEntry, String, Vec<(String, Vec<u8>)>)> = Vec::new();
    for dir_str in skill_dirs {
        let dir = Path::new(&dir_str);
        if !dir.exists() || !dir.is_dir() {
            return Err(format!("技能目录不存在: {}", dir_str));
        }
        let markdown = read_skill_markdown(dir)?;
        let front_matter = parse_front_matter(&markdown);
        let mut base_slug = dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(sanitize_slug)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "skill".to_string());
        if base_slug.is_empty() {
            base_slug = "skill".to_string();
        }
        let mut unique_slug = base_slug.clone();
        let mut n = 2_u32;
        while !used_slugs.insert(unique_slug.clone()) {
            unique_slug = format!("{}-{}", base_slug, n);
            n += 1;
        }

        let mut files = Vec::new();
        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_entry(|e| {
                if e.path() == dir {
                    return true;
                }
                !is_hidden_or_excluded(e)
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let rel = entry
                .path()
                .strip_prefix(dir)
                .map_err(|e| format!("读取目录失败: {}", e))?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let bytes = fs::read(entry.path()).map_err(|e| format!("读取文件失败: {}", e))?;
            files.push((rel_str, bytes));
        }

        entries.push((
            IndustryPackSkillEntry {
                slug: unique_slug.clone(),
                name: front_matter
                    .name
                    .clone()
                    .unwrap_or_else(|| unique_slug.clone()),
                version: front_matter
                    .version
                    .clone()
                    .unwrap_or_else(|| "1.0.0".to_string()),
                tags: parse_skill_tags(&markdown),
            },
            unique_slug,
            files,
        ));
    }

    let manifest = IndustryPackManifest {
        pack_id: normalized_pack_id,
        name: pack_name.trim().to_string(),
        version: version.trim().to_string(),
        industry_tag: industry_tag.trim().to_string(),
        created_at: Utc::now().to_rfc3339(),
        skills: entries.iter().map(|(meta, _, _)| meta.clone()).collect(),
    };

    let output_file =
        fs::File::create(&output_path).map_err(|e| format!("创建行业包失败: {}", e))?;
    let mut zip = zip::ZipWriter::new(output_file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("industry-manifest.json", options)
        .map_err(|e| format!("写入清单失败: {}", e))?;
    zip.write_all(
        serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("序列化行业包清单失败: {}", e))?
            .as_bytes(),
    )
    .map_err(|e| format!("写入清单失败: {}", e))?;

    for (_, slug, files) in entries {
        for (rel, bytes) in files {
            let path = format!("skills/{}/{}", slug, rel);
            zip.start_file(path, options)
                .map_err(|e| format!("写入技能文件失败: {}", e))?;
            zip.write_all(&bytes)
                .map_err(|e| format!("写入技能文件失败: {}", e))?;
        }
    }

    zip.finish()
        .map_err(|e| format!("完成行业包写入失败: {}", e))?;
    Ok(())
}

pub fn read_industry_bundle_manifest_from_path(
    bundle_path: &str,
) -> Result<IndustryPackManifest, String> {
    read_industry_manifest_from_zip(Path::new(bundle_path))
}

#[tauri::command]
pub async fn read_industry_bundle_manifest(
    bundle_path: String,
) -> Result<IndustryPackManifest, String> {
    read_industry_bundle_manifest_from_path(&bundle_path)
}

fn safe_relative_path(path: &Path) -> bool {
    !path.components().any(|comp| {
        matches!(
            comp,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    })
}

pub fn unpack_industry_bundle_to_root(
    bundle_path: &str,
    output_root: Option<String>,
) -> Result<UnpackedIndustryBundle, String> {
    let bundle = Path::new(bundle_path);
    if !bundle.exists() {
        return Err("行业包文件不存在".to_string());
    }

    let manifest = read_industry_manifest_from_zip(bundle)?;
    let base_root = output_root
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_industry_pack_root);
    let version_root = base_root.join(&manifest.pack_id).join(&manifest.version);
    if version_root.exists() {
        fs::remove_dir_all(&version_root).map_err(|e| format!("清理旧版本目录失败: {}", e))?;
    }
    fs::create_dir_all(&version_root).map_err(|e| format!("创建导入目录失败: {}", e))?;

    let file = fs::File::open(bundle).map_err(|e| format!("打开行业包失败: {}", e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("读取行业包失败: {}", e))?;

    let mut skill_dir_map: HashMap<String, PathBuf> = HashMap::new();
    let pack_slug = sanitize_slug(&manifest.pack_id);
    for skill in &manifest.skills {
        let local_dir = version_root.join(format!("{}--{}", pack_slug, skill.slug));
        skill_dir_map.insert(skill.slug.clone(), local_dir);
    }

    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("读取行业包条目失败: {}", e))?;
        let name = entry.name().to_string();
        if !name.starts_with("skills/") {
            continue;
        }
        let relative = name.trim_start_matches("skills/");
        let rel_path = Path::new(relative);
        if !safe_relative_path(rel_path) {
            return Err(format!("行业包包含不安全路径: {}", name));
        }
        let mut components = rel_path.components();
        let slug = match components.next() {
            Some(std::path::Component::Normal(v)) => v.to_string_lossy().to_string(),
            _ => continue,
        };
        let target_dir = skill_dir_map
            .get(&slug)
            .cloned()
            .unwrap_or_else(|| version_root.join(format!("{}--{}", pack_slug, slug)));
        let file_rel: PathBuf = components.map(|c| c.as_os_str()).collect::<PathBuf>();
        if file_rel.as_os_str().is_empty() {
            continue;
        }
        let target_path = target_dir.join(file_rel);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建导入目录失败: {}", e))?;
        }
        let mut out =
            fs::File::create(&target_path).map_err(|e| format!("写入导入文件失败: {}", e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("写入导入文件失败: {}", e))?;
    }

    let mut skill_dirs = manifest
        .skills
        .iter()
        .map(|skill| {
            let p = skill_dir_map
                .get(&skill.slug)
                .cloned()
                .unwrap_or_else(|| version_root.join(format!("{}--{}", pack_slug, skill.slug)));
            p.to_string_lossy().to_string()
        })
        .collect::<Vec<_>>();
    skill_dirs.sort();
    skill_dirs.dedup();

    Ok(UnpackedIndustryBundle {
        manifest,
        skill_dirs,
    })
}

#[tauri::command]
pub async fn unpack_industry_bundle(
    bundle_path: String,
    output_root: Option<String>,
) -> Result<UnpackedIndustryBundle, String> {
    unpack_industry_bundle_to_root(&bundle_path, output_root)
}
