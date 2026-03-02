use std::fs;
use std::io::Write;
use std::path::Path;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use uuid::Uuid;
use chrono::Utc;
use anyhow::{Result, anyhow};

use crate::crypto::{derive_key, encrypt, make_verify_token};
use crate::types::{PackConfig, SkillManifest};

fn has_root_skill_markdown(skill_dir: &Path) -> bool {
    if skill_dir.join("SKILL.md").exists() || skill_dir.join("skill.md").exists() {
        return true;
    }

    let entries = match fs::read_dir(skill_dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("skill.md")
        })
}

fn canonical_rel_path(rel: &Path) -> String {
    let mut rel_str = rel.to_string_lossy().replace('\\', "/");
    if rel
        .parent()
        .map(|p| p.as_os_str().is_empty())
        .unwrap_or(true)
        && rel
            .file_name()
            .map(|name| name.to_string_lossy().eq_ignore_ascii_case("skill.md"))
            .unwrap_or(false)
    {
        rel_str = "SKILL.md".to_string();
    }
    rel_str
}

/// Parse SKILL.md front matter (---\n...\n---\n)
pub fn parse_front_matter(content: &str) -> crate::types::FrontMatter {
    let mut fm = crate::types::FrontMatter {
        name: None,
        description: None,
        version: None,
        model: None,
    };
    let mut in_fm = false;
    for line in content.lines() {
        if line == "---" {
            if !in_fm { in_fm = true; continue; }
            else { break; }
        }
        if !in_fm { continue; }
        if let Some(rest) = line.strip_prefix("name:") {
            fm.name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("description:") {
            fm.description = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("version:") {
            fm.version = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("model:") {
            fm.model = Some(rest.trim().to_string());
        }
    }
    fm
}

pub fn pack(config: &PackConfig) -> Result<()> {
    let skill_dir = Path::new(&config.dir_path);
    if !has_root_skill_markdown(skill_dir) {
        return Err(anyhow!("SKILL.md not found in {:?}", skill_dir));
    }

    let skill_id = Uuid::new_v4().to_string();
    let key = derive_key(&config.username, &skill_id, &config.name);

    let manifest = SkillManifest {
        id: skill_id,
        name: config.name.clone(),
        description: config.description.clone(),
        version: config.version.clone(),
        author: config.author.clone(),
        recommended_model: config.recommended_model.clone(),
        tags: vec![],
        created_at: Utc::now(),
        username_hint: Some(config.username.clone()),
        encrypted_verify: make_verify_token(&key)?,
    };

    let output_file = fs::File::create(&config.output_path)?;
    let mut zip = zip::ZipWriter::new(output_file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Write manifest.json (plaintext)
    zip.start_file("manifest.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

    // Encrypt and write all files under encrypted/
    for entry in WalkDir::new(skill_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_dir() { continue; }
        let abs_path = entry.path();
        let rel = abs_path.strip_prefix(skill_dir)?;
        let rel_str = canonical_rel_path(rel);

        let plaintext = fs::read(abs_path)?;
        let ciphertext = encrypt(&plaintext, &key)?;

        let enc_path = format!("encrypted/{}.enc", rel_str);
        zip.start_file(&enc_path, options)?;
        zip.write_all(&ciphertext)?;
    }

    zip.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use std::io::Read;

    fn make_test_skill_dir(dir: &Path) {
        fs::write(dir.join("SKILL.md"), "---\nname: Test Skill\nversion: 1.0.0\n---\n\n# Test\nYou are a test assistant.").unwrap();
        fs::create_dir(dir.join("templates")).unwrap();
        fs::write(dir.join("templates/outline.md"), "# Outline template").unwrap();
    }

    #[test]
    fn test_pack_creates_skillpack() {
        let dir = tempdir().unwrap();
        let skill_dir = dir.path().join("test-skill");
        fs::create_dir(&skill_dir).unwrap();
        make_test_skill_dir(&skill_dir);

        let output = dir.path().join("test.skillpack");
        let config = PackConfig {
            dir_path: skill_dir.to_string_lossy().to_string(),
            name: "Test Skill".to_string(),
            description: "A test skill".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            username: "alice".to_string(),
            recommended_model: "claude-3-5-sonnet-20241022".to_string(),
            output_path: output.to_string_lossy().to_string(),
        };
        pack(&config).unwrap();
        assert!(output.exists());
        assert!(output.metadata().unwrap().len() > 0);
    }

    #[test]
    fn test_pack_fails_without_skill_md() {
        let dir = tempdir().unwrap();
        let config = PackConfig {
            dir_path: dir.path().to_string_lossy().to_string(),
            name: "Bad".to_string(),
            description: "".to_string(),
            version: "1.0.0".to_string(),
            author: "".to_string(),
            username: "alice".to_string(),
            recommended_model: "".to_string(),
            output_path: dir.path().join("out.skillpack").to_string_lossy().to_string(),
        };
        assert!(pack(&config).is_err());
    }

    #[test]
    fn test_pack_normalizes_lowercase_skill_md_path() {
        let dir = tempdir().unwrap();
        let skill_dir = dir.path().join("skill");
        fs::create_dir(&skill_dir).unwrap();
        fs::write(skill_dir.join("skill.md"), "---\nname: Lower\n---\n").unwrap();

        let output = dir.path().join("lower.skillpack");
        let config = PackConfig {
            dir_path: skill_dir.to_string_lossy().to_string(),
            name: "Lower".to_string(),
            description: "".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            username: "alice".to_string(),
            recommended_model: "claude-3-5-sonnet-20241022".to_string(),
            output_path: output.to_string_lossy().to_string(),
        };
        pack(&config).unwrap();

        let file = fs::File::open(output).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        let mut has_upper = false;
        for i in 0..zip.len() {
            let entry = zip.by_index(i).unwrap();
            if entry.name() == "encrypted/SKILL.md.enc" {
                has_upper = true;
                break;
            }
        }
        assert!(has_upper, "expected encrypted/SKILL.md.enc in archive");

        let mut skill_entry = zip.by_name("encrypted/SKILL.md.enc").unwrap();
        let mut encrypted_bytes = Vec::new();
        skill_entry.read_to_end(&mut encrypted_bytes).unwrap();
        assert!(!encrypted_bytes.is_empty());
    }
}
