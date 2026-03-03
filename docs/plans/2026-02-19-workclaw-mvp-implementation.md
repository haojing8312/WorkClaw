# WorkClaw MVP Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build WorkClaw MVP — Studio（Skill 打包工具）and Runtime（Skill 运行客户端）as two Tauri 2.0 desktop apps sharing a common Rust encryption crate.

**Architecture:** Monorepo (pnpm + Turborepo) with two Tauri apps (`apps/studio`, `apps/runtime`) sharing a Rust crate `packages/skillpack-rs` for AES-256-GCM encryption/decryption. Studio packs `.skillpack` files encrypted with a username-derived key; Runtime installs and runs them after the user enters their username.

**Tech Stack:** Tauri 2.0, Rust (aes-gcm, pbkdf2, zip, sqlx, reqwest, keyring), React 18, TypeScript, Tailwind CSS, shadcn/ui, pnpm workspaces, Turborepo

---

## Prerequisites (read before starting)

- Install Rust: https://rustup.rs/
- Install Node.js 20+ and pnpm: `npm i -g pnpm`
- Install Tauri CLI v2: `cargo install tauri-cli --version "^2"`
- Windows: install WebView2 (usually pre-installed on Win10/11)
- Verify: `cargo tauri --version` → `tauri-cli 2.x.x`

---

## Task 1: Monorepo Scaffold

**Files:**
- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `turbo.json`
- Create: `.gitignore`

**Step 1: Create root package.json**

```json
{
  "name": "workclaw",
  "private": true,
  "scripts": {
    "studio": "pnpm --filter studio tauri dev",
    "runtime": "pnpm --filter runtime tauri dev",
    "build:studio": "pnpm --filter studio tauri build",
    "build:runtime": "pnpm --filter runtime tauri build"
  },
  "devDependencies": {
    "turbo": "^2.0.0"
  }
}
```

**Step 2: Create pnpm-workspace.yaml**

```yaml
packages:
  - 'apps/*'
  - 'packages/*'
```

**Step 3: Create turbo.json**

```json
{
  "$schema": "https://turbo.build/schema.json",
  "tasks": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": ["dist/**", ".next/**"]
    },
    "dev": {
      "cache": false,
      "persistent": true
    }
  }
}
```

**Step 4: Create .gitignore**

```
node_modules/
dist/
target/
.turbo/
*.skillpack
*.key
.env
```

**Step 5: Install root deps and verify**

```bash
pnpm install
```

Expected: `node_modules/` created, no errors.

**Step 6: Commit**

```bash
git add package.json pnpm-workspace.yaml turbo.json .gitignore
git commit -m "chore: monorepo scaffold with pnpm + turborepo"
```

---

## Task 2: skillpack-rs Crate (Crypto Core)

**Files:**
- Create: `packages/skillpack-rs/Cargo.toml`
- Create: `packages/skillpack-rs/src/lib.rs`
- Create: `packages/skillpack-rs/src/crypto.rs`
- Create: `packages/skillpack-rs/src/pack.rs`
- Create: `packages/skillpack-rs/src/unpack.rs`
- Create: `packages/skillpack-rs/src/types.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "skillpack-rs"
version = "0.1.0"
edition = "2021"

[dependencies]
aes-gcm = "0.10"
pbkdf2 = { version = "0.12", features = ["hmac"] }
hmac = "0.12"
sha2 = "0.10"
rand = "0.8"
zip = "2.1"
uuid = { version = "1", features = ["v4"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
base64 = "0.22"
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
walkdir = "2"

[dev-dependencies]
tempfile = "3"
```

**Step 2: Create src/types.rs**

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub recommended_model: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub username_hint: Option<String>,
    pub encrypted_verify: String,
}

#[derive(Debug, Clone)]
pub struct PackConfig {
    pub dir_path: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub username: String,
    pub recommended_model: String,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontMatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub model: Option<String>,
}
```

**Step 3: Create src/crypto.rs**

```rust
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use anyhow::{Result, anyhow};

const PBKDF2_ITERATIONS: u32 = 100_000;
const VERIFY_PLAINTEXT: &[u8] = b"SKILLMINT_OK";

pub fn derive_key(username: &str, skill_id: &str, skill_name: &str) -> [u8; 32] {
    // salt = SHA256(skill_id + skill_name) — deterministic, no random
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(skill_id.as_bytes());
    hasher.update(skill_name.as_bytes());
    let salt = hasher.finalize();

    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        username.as_bytes(),
        &salt,
        PBKDF2_ITERATIONS,
        &mut key,
    );
    key
}

pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow!("encrypt error: {e}"))?;
    // prepend 12-byte nonce so decrypt can extract it
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if data.len() < 12 {
        return Err(anyhow!("data too short"));
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow!("decrypt failed — wrong username?"))
}

pub fn make_verify_token(key: &[u8; 32]) -> Result<String> {
    let encrypted = encrypt(VERIFY_PLAINTEXT, key)?;
    Ok(B64.encode(&encrypted))
}

pub fn check_verify_token(token: &str, key: &[u8; 32]) -> bool {
    let Ok(data) = B64.decode(token) else { return false };
    let Ok(plain) = decrypt(&data, key) else { return false };
    plain == VERIFY_PLAINTEXT
}
```

**Step 4: Write crypto tests**

Add to end of `src/crypto.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() {
        let k1 = derive_key("alice", "skill-id-123", "合同审查");
        let k2 = derive_key("alice", "skill-id-123", "合同审查");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_derive_key_different_users() {
        let k1 = derive_key("alice", "skill-id-123", "合同审查");
        let k2 = derive_key("bob", "skill-id-123", "合同审查");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_key("alice", "skill-id-123", "test");
        let plaintext = b"Hello, WorkClaw!";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = derive_key("alice", "skill-id-123", "test");
        let key2 = derive_key("bob", "skill-id-123", "test");
        let encrypted = encrypt(b"secret", &key1).unwrap();
        assert!(decrypt(&encrypted, &key2).is_err());
    }

    #[test]
    fn test_verify_token_roundtrip() {
        let key = derive_key("alice", "id", "name");
        let token = make_verify_token(&key).unwrap();
        assert!(check_verify_token(&token, &key));
    }

    #[test]
    fn test_verify_token_wrong_key() {
        let key1 = derive_key("alice", "id", "name");
        let key2 = derive_key("bob", "id", "name");
        let token = make_verify_token(&key1).unwrap();
        assert!(!check_verify_token(&token, &key2));
    }
}
```

**Step 5: Run crypto tests**

```bash
cd packages/skillpack-rs
cargo test crypto
```

Expected: 6 tests pass.

**Step 6: Create src/pack.rs**

```rust
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use uuid::Uuid;
use chrono::Utc;
use anyhow::{Result, anyhow};

use crate::crypto::{derive_key, encrypt, make_verify_token};
use crate::types::{PackConfig, SkillManifest};

/// Parse SKILL.md front matter (---\n...\n---\n)
pub fn parse_front_matter(content: &str) -> crate::types::FrontMatter {
    let mut fm = crate::types::FrontMatter {
        name: None,
        description: None,
        version: None,
        model: None,
    };
    let lines: Vec<&str> = content.lines().collect();
    let mut in_fm = false;
    for line in &lines {
        if *line == "---" {
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
    let skill_md = skill_dir.join("SKILL.md");
    if !skill_md.exists() {
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
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        let plaintext = fs::read(abs_path)?;
        let ciphertext = encrypt(&plaintext, &key)?;

        let enc_path = format!("encrypted/{}.enc", rel_str);
        zip.start_file(&enc_path, options)?;
        zip.write_all(&ciphertext)?;
    }

    zip.finish()?;
    Ok(())
}
```

**Step 7: Write pack tests**

Add to end of `src/pack.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

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
}
```

**Step 8: Create src/unpack.rs**

```rust
use std::io::Read;
use anyhow::{Result, anyhow};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

use crate::crypto::{derive_key, decrypt, check_verify_token};
use crate::types::SkillManifest;

pub struct UnpackedSkill {
    pub manifest: SkillManifest,
    /// Map of relative path → decrypted content bytes
    /// e.g. "SKILL.md" → b"..."
    pub files: std::collections::HashMap<String, Vec<u8>>,
}

pub fn verify_and_unpack(pack_path: &str, username: &str) -> Result<UnpackedSkill> {
    let file = std::fs::File::open(pack_path)?;
    let mut zip = zip::ZipArchive::new(file)?;

    // Read manifest
    let manifest: SkillManifest = {
        let mut entry = zip.by_name("manifest.json")
            .map_err(|_| anyhow!("manifest.json not found in skillpack"))?;
        let mut buf = String::new();
        entry.read_to_string(&mut buf)?;
        serde_json::from_str(&buf)?
    };

    // Derive key and verify username
    let key = derive_key(username, &manifest.id, &manifest.name);
    if !check_verify_token(&manifest.encrypted_verify, &key) {
        return Err(anyhow!("用户名错误，无法解密此 Skill"));
    }

    // Decrypt all files in encrypted/
    let mut files = std::collections::HashMap::new();
    let names: Vec<String> = (0..zip.len())
        .filter_map(|i| {
            let entry = zip.by_index(i).ok()?;
            let name = entry.name().to_string();
            if name.starts_with("encrypted/") && name.ends_with(".enc") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    for enc_name in names {
        let mut entry = zip.by_name(&enc_name)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let plain = decrypt(&buf, &key)?;

        // Strip "encrypted/" prefix and ".enc" suffix to get original path
        let rel = enc_name
            .strip_prefix("encrypted/").unwrap()
            .strip_suffix(".enc").unwrap()
            .to_string();
        files.insert(rel, plain);
    }

    Ok(UnpackedSkill { manifest, files })
}
```

**Step 9: Write unpack tests**

Add to end of `src/unpack.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::{pack, PackConfig};
    use tempfile::tempdir;
    use std::fs;
    use std::path::Path;

    fn setup_and_pack(dir: &Path, username: &str) -> String {
        let skill_dir = dir.join("skill");
        fs::create_dir(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: Test\n---\nYou are a test.").unwrap();
        let output = dir.join("test.skillpack");
        pack(&PackConfig {
            dir_path: skill_dir.to_string_lossy().to_string(),
            name: "Test".to_string(),
            description: "desc".to_string(),
            version: "1.0.0".to_string(),
            author: "author".to_string(),
            username: username.to_string(),
            recommended_model: "claude-3-5-sonnet-20241022".to_string(),
            output_path: output.to_string_lossy().to_string(),
        }).unwrap();
        output.to_string_lossy().to_string()
    }

    #[test]
    fn test_correct_username_unpacks() {
        let dir = tempdir().unwrap();
        let pack_path = setup_and_pack(dir.path(), "alice");
        let result = verify_and_unpack(&pack_path, "alice").unwrap();
        assert_eq!(result.manifest.name, "Test");
        assert!(result.files.contains_key("SKILL.md"));
    }

    #[test]
    fn test_wrong_username_fails() {
        let dir = tempdir().unwrap();
        let pack_path = setup_and_pack(dir.path(), "alice");
        let result = verify_and_unpack(&pack_path, "bob");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("用户名错误"));
    }
}
```

**Step 10: Create src/lib.rs**

```rust
pub mod crypto;
pub mod pack;
pub mod unpack;
pub mod types;

pub use pack::pack;
pub use unpack::verify_and_unpack;
pub use types::{PackConfig, SkillManifest, FrontMatter};
```

**Step 11: Run all crate tests**

```bash
cd packages/skillpack-rs
cargo test
```

Expected: All tests pass (crypto: 6, pack: 2, unpack: 2 = 10 total).

**Step 12: Commit**

```bash
git add packages/skillpack-rs/
git commit -m "feat: skillpack-rs crate with AES-256-GCM crypto, pack, and unpack"
```

---

## Task 3: Studio Tauri App Scaffold

**Files:**
- Create: `apps/studio/` (via tauri CLI)
- Modify: `apps/studio/src-tauri/Cargo.toml`
- Create: `apps/studio/package.json`

**Step 1: Scaffold Studio Tauri app**

```bash
cd apps
cargo tauri init --app-name "WorkClaw Studio" --window-title "WorkClaw Studio" --frontend-dist ../dist --dev-url http://localhost:5173 --before-dev-command "pnpm dev" --before-build-command "pnpm build"
```

When prompted for the app identifier: `dev.workclaw.studio`

This creates `apps/studio/src-tauri/`. Move if needed so structure is `apps/studio/src-tauri/`.

**Step 2: Create apps/studio/package.json**

```json
{
  "name": "studio",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "@tauri-apps/plugin-fs": "^2",
    "react": "^18",
    "react-dom": "^18"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^18",
    "@types/react-dom": "^18",
    "@vitejs/plugin-react": "^4",
    "tailwindcss": "^3",
    "typescript": "^5",
    "vite": "^5"
  }
}
```

**Step 3: Add skillpack-rs as dependency in apps/studio/src-tauri/Cargo.toml**

Add to `[dependencies]`:

```toml
skillpack-rs = { path = "../../../packages/skillpack-rs" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
```

**Step 4: Install frontend deps**

```bash
cd apps/studio
pnpm install
```

**Step 5: Verify Tauri builds (debug)**

```bash
cd apps/studio
cargo tauri build --debug
```

Expected: Builds without errors (initial scaffold).

**Step 6: Commit**

```bash
git add apps/studio/
git commit -m "chore: scaffold studio tauri app"
```

---

## Task 4: Studio Tauri Commands (Rust Backend)

**Files:**
- Modify: `apps/studio/src-tauri/src/main.rs`
- Create: `apps/studio/src-tauri/src/commands.rs`

**Step 1: Create commands.rs**

```rust
use skillpack_rs::{pack, PackConfig, FrontMatter};
use skillpack_rs::pack::parse_front_matter;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
pub struct SkillDirInfo {
    pub files: Vec<String>,
    pub front_matter: FrontMatter,
}

#[tauri::command]
pub async fn read_skill_dir(dir_path: String) -> Result<SkillDirInfo, String> {
    let skill_dir = Path::new(&dir_path);
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_md_path.exists() {
        return Err("SKILL.md not found in selected directory".to_string());
    }

    let skill_md_content = fs::read_to_string(&skill_md_path)
        .map_err(|e| e.to_string())?;
    let front_matter = parse_front_matter(&skill_md_content);

    let files: Vec<String> = WalkDir::new(skill_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(skill_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();

    Ok(SkillDirInfo { files, front_matter })
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
    pack(&config).map_err(|e| e.to_string())
}
```

**Step 2: Update main.rs**

```rust
// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::read_skill_dir,
            commands::pack_skill,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: Verify it compiles**

```bash
cd apps/studio/src-tauri
cargo build
```

Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add apps/studio/src-tauri/
git commit -m "feat: studio tauri commands for read_skill_dir and pack_skill"
```

---

## Task 5: Studio Frontend (React UI)

**Files:**
- Create: `apps/studio/src/main.tsx`
- Create: `apps/studio/src/App.tsx`
- Create: `apps/studio/src/components/FileTree.tsx`
- Create: `apps/studio/src/components/PackForm.tsx`
- Create: `apps/studio/index.html`
- Create: `apps/studio/vite.config.ts`
- Create: `apps/studio/tailwind.config.js`
- Create: `apps/studio/tsconfig.json`

**Step 1: Create index.html**

```html
<!doctype html>
<html lang="zh">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>WorkClaw Studio</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

**Step 2: Create vite.config.ts**

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  envPrefix: ["VITE_", "TAURI_"],
  build: { target: "chrome105", minify: !process.env.TAURI_DEBUG },
});
```

**Step 3: Create tailwind.config.js**

```js
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: { extend: {} },
  plugins: [],
};
```

**Step 4: Create src/main.tsx**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

**Step 5: Create src/index.css**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

body {
  font-family: system-ui, sans-serif;
  background: #0f172a;
  color: #f8fafc;
  margin: 0;
  height: 100vh;
  overflow: hidden;
}
```

**Step 6: Create src/components/FileTree.tsx**

```tsx
interface FileTreeProps {
  files: string[];
}

export function FileTree({ files }: FileTreeProps) {
  if (files.length === 0) {
    return (
      <div className="text-slate-400 text-sm p-4">
        选择 Skill 目录后显示文件树
      </div>
    );
  }

  return (
    <div className="text-sm font-mono space-y-1 p-2">
      {files.map((f) => (
        <div key={f} className="flex items-center gap-2 text-slate-300 py-0.5">
          <span className="text-slate-500">📄</span>
          <span>{f}</span>
        </div>
      ))}
    </div>
  );
}
```

**Step 7: Create src/components/PackForm.tsx**

```tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

interface FrontMatter {
  name?: string;
  description?: string;
  version?: string;
  model?: string;
}

interface PackFormProps {
  dirPath: string;
  frontMatter: FrontMatter;
}

export function PackForm({ dirPath, frontMatter }: PackFormProps) {
  const [name, setName] = useState(frontMatter.name ?? "");
  const [description, setDescription] = useState(frontMatter.description ?? "");
  const [version, setVersion] = useState(frontMatter.version ?? "1.0.0");
  const [author, setAuthor] = useState("");
  const [username, setUsername] = useState("");
  const [recommendedModel, setRecommendedModel] = useState(
    frontMatter.model ?? "claude-3-5-sonnet-20241022"
  );
  const [status, setStatus] = useState<"idle" | "packing" | "done" | "error">("idle");
  const [errorMsg, setErrorMsg] = useState("");

  async function handlePack() {
    if (!username.trim()) {
      setErrorMsg("请填写客户用户名");
      return;
    }
    const outputPath = await save({
      defaultPath: `${name || "skill"}.skillpack`,
      filters: [{ name: "SkillPack", extensions: ["skillpack"] }],
    });
    if (!outputPath) return;

    setStatus("packing");
    setErrorMsg("");
    try {
      await invoke("pack_skill", {
        dirPath,
        name,
        description,
        version,
        author,
        username,
        recommendedModel,
        outputPath,
      });
      setStatus("done");
    } catch (e: unknown) {
      setStatus("error");
      setErrorMsg(String(e));
    }
  }

  const inputCls =
    "w-full bg-slate-800 border border-slate-600 rounded px-3 py-2 text-sm text-slate-100 focus:outline-none focus:border-blue-500";
  const labelCls = "block text-xs text-slate-400 mb-1";

  return (
    <div className="space-y-4">
      <div>
        <label className={labelCls}>Skill 名称</label>
        <input className={inputCls} value={name} onChange={(e) => setName(e.target.value)} />
      </div>
      <div>
        <label className={labelCls}>描述</label>
        <input className={inputCls} value={description} onChange={(e) => setDescription(e.target.value)} />
      </div>
      <div>
        <label className={labelCls}>版本号</label>
        <input className={inputCls} value={version} onChange={(e) => setVersion(e.target.value)} />
      </div>
      <div>
        <label className={labelCls}>作者</label>
        <input className={inputCls} value={author} onChange={(e) => setAuthor(e.target.value)} />
      </div>
      <div>
        <label className={labelCls}>推荐模型</label>
        <input className={inputCls} value={recommendedModel} onChange={(e) => setRecommendedModel(e.target.value)} />
      </div>
      <div>
        <label className={labelCls}>客户用户名（解密密钥）</label>
        <input
          className={inputCls}
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          placeholder="例如：alice"
        />
        <p className="text-xs text-slate-500 mt-1">
          ℹ️ 客户需在 Runtime 中输入此用户名才能解锁 Skill
        </p>
      </div>

      {errorMsg && (
        <div className="text-red-400 text-sm bg-red-900/30 rounded p-2">{errorMsg}</div>
      )}
      {status === "done" && (
        <div className="text-green-400 text-sm bg-green-900/30 rounded p-2">
          ✅ 打包成功！
        </div>
      )}

      <button
        onClick={handlePack}
        disabled={status === "packing"}
        className="w-full bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 text-white font-medium py-2 rounded transition-colors"
      >
        {status === "packing" ? "打包中..." : "一键打包"}
      </button>
    </div>
  );
}
```

**Step 8: Create src/App.tsx**

```tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FileTree } from "./components/FileTree";
import { PackForm } from "./components/PackForm";

interface FrontMatter {
  name?: string;
  description?: string;
  version?: string;
  model?: string;
}

interface SkillDirInfo {
  files: string[];
  front_matter: FrontMatter;
}

export default function App() {
  const [dirPath, setDirPath] = useState<string | null>(null);
  const [skillInfo, setSkillInfo] = useState<SkillDirInfo | null>(null);
  const [error, setError] = useState("");

  async function handleSelectDir() {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || typeof selected !== "string") return;
    setError("");
    try {
      const info = await invoke<SkillDirInfo>("read_skill_dir", { dirPath: selected });
      setDirPath(selected);
      setSkillInfo(info);
    } catch (e: unknown) {
      setError(String(e));
      setSkillInfo(null);
    }
  }

  return (
    <div className="flex flex-col h-screen bg-slate-900 text-slate-100">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-3 border-b border-slate-700 bg-slate-800">
        <h1 className="font-semibold text-lg">WorkClaw Studio</h1>
        <button
          onClick={handleSelectDir}
          className="bg-slate-700 hover:bg-slate-600 text-sm px-4 py-1.5 rounded transition-colors"
        >
          选择 Skill 目录
        </button>
      </div>

      {/* Path bar */}
      {dirPath && (
        <div className="px-6 py-2 text-xs text-slate-400 border-b border-slate-700 bg-slate-800/50">
          {dirPath}
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="mx-6 mt-3 text-red-400 text-sm bg-red-900/30 rounded p-2">{error}</div>
      )}

      {/* Main content */}
      <div className="flex flex-1 overflow-hidden">
        {/* File tree */}
        <div className="w-1/2 border-r border-slate-700 overflow-y-auto">
          <div className="px-4 py-2 text-xs font-medium text-slate-400 border-b border-slate-700">
            文件树（只读）
          </div>
          <FileTree files={skillInfo?.files ?? []} />
        </div>

        {/* Pack form */}
        <div className="w-1/2 overflow-y-auto p-6">
          <div className="text-xs font-medium text-slate-400 mb-4">打包配置</div>
          {skillInfo ? (
            <PackForm dirPath={dirPath!} frontMatter={skillInfo.front_matter} />
          ) : (
            <div className="text-slate-500 text-sm">请先选择 Skill 目录</div>
          )}
        </div>
      </div>
    </div>
  );
}
```

**Step 9: Start Studio in dev mode and verify UI**

```bash
cd apps/studio
cargo tauri dev
```

Expected: Window opens, "选择 Skill 目录" button works, selecting a directory with SKILL.md shows file tree and form.

**Step 10: Test pack with a real skill directory**

Use any `.claude/skills/xxx/` directory, fill in the form, click "一键打包", verify `.skillpack` file is created on disk.

**Step 11: Commit**

```bash
git add apps/studio/src/
git commit -m "feat: studio frontend — file tree + pack form"
```

---

## Task 6: Runtime Tauri App Scaffold

**Files:**
- Create: `apps/runtime/` (same scaffold pattern as Studio)
- Modify: `apps/runtime/src-tauri/Cargo.toml`

**Step 1: Scaffold Runtime app**

```bash
cd apps
cargo tauri init --app-name "WorkClaw Runtime" --window-title "WorkClaw Runtime" --frontend-dist ../dist --dev-url http://localhost:5174
```

Identifier: `dev.workclaw.runtime`

**Step 2: Create apps/runtime/package.json**

Same structure as studio's, but change port to 5174 in vite config and name to `runtime`.

```json
{
  "name": "runtime",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite --port 5174",
    "build": "tsc && vite build",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "@tauri-apps/plugin-shell": "^2",
    "react": "^18",
    "react-dom": "^18",
    "react-markdown": "^9",
    "react-syntax-highlighter": "^15"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^18",
    "@types/react-dom": "^18",
    "@types/react-syntax-highlighter": "^15",
    "@vitejs/plugin-react": "^4",
    "tailwindcss": "^3",
    "typescript": "^5",
    "vite": "^5"
  }
}
```

**Step 3: Add Rust dependencies to apps/runtime/src-tauri/Cargo.toml**

```toml
skillpack-rs = { path = "../../../packages/skillpack-rs" }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio-rustls", "macros"] }
keyring = "3"
reqwest = { version = "0.12", features = ["json", "stream"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
futures-util = "0.3"
tauri-plugin-dialog = "2"
```

**Step 4: Commit scaffold**

```bash
git add apps/runtime/
git commit -m "chore: scaffold runtime tauri app"
```

---

## Task 7: Runtime Database & Skill Installation (Rust)

**Files:**
- Create: `apps/runtime/src-tauri/src/db.rs`
- Create: `apps/runtime/src-tauri/src/commands/skills.rs`
- Modify: `apps/runtime/src-tauri/src/main.rs`

**Step 1: Create db.rs**

```rust
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tauri::AppHandle;
use anyhow::Result;

pub async fn init_db(app: &AppHandle) -> Result<SqlitePool> {
    let app_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;
    let db_path = app_dir.join("workclaw.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS installed_skills (
            id TEXT PRIMARY KEY,
            manifest TEXT NOT NULL,
            installed_at TEXT NOT NULL,
            last_used_at TEXT,
            username TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            skill_id TEXT NOT NULL,
            title TEXT,
            created_at TEXT NOT NULL,
            model_id TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS model_configs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            api_format TEXT NOT NULL,
            base_url TEXT NOT NULL,
            model_name TEXT NOT NULL,
            is_default INTEGER DEFAULT 0
        );
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}
```

**Step 2: Create commands/skills.rs**

```rust
use sqlx::SqlitePool;
use tauri::State;
use skillpack_rs::{verify_and_unpack, SkillManifest};
use chrono::Utc;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use anyhow::Result;

pub struct DbState(pub SqlitePool);

#[tauri::command]
pub async fn install_skill(
    pack_path: String,
    username: String,
    db: State<'_, DbState>,
) -> Result<SkillManifest, String> {
    let unpacked = verify_and_unpack(&pack_path, &username)
        .map_err(|e| e.to_string())?;

    let manifest_json = serde_json::to_string(&unpacked.manifest)
        .map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR REPLACE INTO installed_skills (id, manifest, installed_at, username) VALUES (?, ?, ?, ?)"
    )
    .bind(&unpacked.manifest.id)
    .bind(&manifest_json)
    .bind(&now)
    .bind(&username)
    .execute(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(unpacked.manifest)
}

#[tauri::command]
pub async fn list_skills(db: State<'_, DbState>) -> Result<Vec<SkillManifest>, String> {
    let rows = sqlx::query_as::<_, (String,)>("SELECT manifest FROM installed_skills ORDER BY installed_at DESC")
        .fetch_all(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    rows.iter()
        .map(|(json,)| serde_json::from_str::<SkillManifest>(json).map_err(|e| e.to_string()))
        .collect()
}

#[tauri::command]
pub async fn delete_skill(skill_id: String, db: State<'_, DbState>) -> Result<(), String> {
    sqlx::query("DELETE FROM installed_skills WHERE id = ?")
        .bind(&skill_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

**Step 3: Update main.rs**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod commands;

use commands::skills::DbState;

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let pool = tauri::async_runtime::block_on(db::init_db(app.handle()))
                .expect("failed to init db");
            app.manage(DbState(pool));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::skills::install_skill,
            commands::skills::list_skills,
            commands::skills::delete_skill,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 4: Verify it compiles**

```bash
cd apps/runtime/src-tauri
cargo build
```

Expected: Compiles without errors.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/
git commit -m "feat: runtime db init, install/list/delete skill commands"
```

---

## Task 8: Runtime Model Config & Chat Commands (Rust)

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/models.rs`
- Create: `apps/runtime/src-tauri/src/commands/chat.rs`
- Create: `apps/runtime/src-tauri/src/adapters/mod.rs`
- Create: `apps/runtime/src-tauri/src/adapters/anthropic.rs`
- Create: `apps/runtime/src-tauri/src/adapters/openai.rs`

**Step 1: Create commands/models.rs**

```rust
use keyring::Entry;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;
use super::skills::DbState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub api_format: String,  // "anthropic" | "openai"
    pub base_url: String,
    pub model_name: String,
    pub is_default: bool,
}

fn keyring_entry(model_id: &str) -> keyring::Result<Entry> {
    Entry::new("workclaw-runtime", &format!("model-{}", model_id))
}

#[tauri::command]
pub async fn save_model_config(
    config: ModelConfig,
    api_key: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    let id = if config.id.is_empty() { Uuid::new_v4().to_string() } else { config.id.clone() };

    sqlx::query(
        "INSERT OR REPLACE INTO model_configs (id, name, api_format, base_url, model_name, is_default) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&config.name)
    .bind(&config.api_format)
    .bind(&config.base_url)
    .bind(&config.model_name)
    .bind(config.is_default)
    .execute(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    if !api_key.is_empty() {
        keyring_entry(&id)
            .and_then(|e| e.set_password(&api_key))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_model_configs(db: State<'_, DbState>) -> Result<Vec<ModelConfig>, String> {
    sqlx::query_as!(
        ModelConfig,
        r#"SELECT id, name, api_format, base_url, model_name, is_default as "is_default: bool" FROM model_configs"#
    )
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_model_config(model_id: String, db: State<'_, DbState>) -> Result<(), String> {
    sqlx::query("DELETE FROM model_configs WHERE id = ?")
        .bind(&model_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    let _ = keyring_entry(&model_id).and_then(|e| e.delete_credential());
    Ok(())
}

pub fn get_api_key(model_id: &str) -> Option<String> {
    keyring_entry(model_id).ok()?.get_password().ok()
}
```

**Step 2: Create adapters/anthropic.rs**

```rust
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde_json::{json, Value};
use futures_util::StreamExt;

pub async fn chat_stream(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    messages: Vec<Value>,
    on_token: impl Fn(String) + Send,
) -> Result<()> {
    let client = Client::new();
    let body = json!({
        "model": model,
        "max_tokens": 4096,
        "system": system_prompt,
        "messages": messages,
        "stream": true
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        return Err(anyhow!("Anthropic API error: {text}"));
    }

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" { break; }
                if let Ok(v) = serde_json::from_str::<Value>(data) {
                    if let Some(token) = v["delta"]["text"].as_str() {
                        on_token(token.to_string());
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn test_connection(api_key: &str, model: &str) -> Result<bool> {
    let client = Client::new();
    let body = json!({
        "model": model,
        "max_tokens": 10,
        "messages": [{"role": "user", "content": "hi"}]
    });
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;
    Ok(resp.status().is_success())
}
```

**Step 3: Create adapters/openai.rs**

```rust
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde_json::{json, Value};
use futures_util::StreamExt;

pub async fn chat_stream(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    messages: Vec<Value>,
    on_token: impl Fn(String) + Send,
) -> Result<()> {
    let client = Client::new();
    let mut all_messages = vec![json!({"role": "system", "content": system_prompt})];
    all_messages.extend(messages);

    let body = json!({
        "model": model,
        "messages": all_messages,
        "stream": true
    });

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        return Err(anyhow!("API error: {text}"));
    }

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" { break; }
                if let Ok(v) = serde_json::from_str::<Value>(data) {
                    if let Some(token) = v["choices"][0]["delta"]["content"].as_str() {
                        on_token(token.to_string());
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn test_connection(base_url: &str, api_key: &str, model: &str) -> Result<bool> {
    let client = Client::new();
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 10
    });
    let resp = client.post(&url).bearer_auth(api_key).json(&body).send().await?;
    Ok(resp.status().is_success())
}
```

**Step 4: Create adapters/mod.rs**

```rust
pub mod anthropic;
pub mod openai;
```

**Step 5: Create commands/chat.rs**

```rust
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};
use serde_json::{json, Value};
use uuid::Uuid;
use chrono::Utc;
use skillpack_rs::verify_and_unpack;
use super::skills::DbState;
use super::models::get_api_key;
use crate::adapters;

#[derive(serde::Serialize, Clone)]
struct StreamToken {
    session_id: String,
    token: String,
    done: bool,
}

#[tauri::command]
pub async fn create_session(
    skill_id: String,
    model_id: String,
    db: State<'_, DbState>,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO sessions (id, skill_id, title, created_at, model_id) VALUES (?, ?, ?, ?, ?)")
        .bind(&session_id)
        .bind(&skill_id)
        .bind("New Chat")
        .bind(&now)
        .bind(&model_id)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;
    Ok(session_id)
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    session_id: String,
    user_message: String,
    db: State<'_, DbState>,
) -> Result<(), String> {
    // Save user message
    let msg_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&msg_id)
        .bind(&session_id)
        .bind("user")
        .bind(&user_message)
        .bind(&now)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    // Load session + skill + model
    let (skill_id, model_id) = sqlx::query_as::<_, (String, String)>(
        "SELECT skill_id, model_id FROM sessions WHERE id = ?"
    )
    .bind(&session_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let (manifest_json, username) = sqlx::query_as::<_, (String, String)>(
        "SELECT manifest, username FROM installed_skills WHERE id = ?"
    )
    .bind(&skill_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let manifest: skillpack_rs::SkillManifest = serde_json::from_str(&manifest_json)
        .map_err(|e| e.to_string())?;

    // We need the skillpack file path — store it at install time
    // For MVP: re-derive system prompt from stored manifest + username
    // (We store encrypted content; re-unpack from stored bytes)
    // Simplified: use manifest description as system prompt for now
    // Full impl: store skillpack path in DB and re-unpack on demand
    let system_prompt = manifest.description.clone();

    // Load message history
    let history = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM messages WHERE session_id = ? ORDER BY created_at ASC"
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let messages: Vec<Value> = history.iter()
        .map(|(role, content)| json!({"role": role, "content": content}))
        .collect();

    let (api_format, base_url, model_name) = sqlx::query_as::<_, (String, String, String)>(
        "SELECT api_format, base_url, model_name FROM model_configs WHERE id = ?"
    )
    .bind(&model_id)
    .fetch_one(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    let api_key = get_api_key(&model_id).ok_or("No API key found for this model")?;

    // Stream tokens to frontend via Tauri event
    let app_clone = app.clone();
    let session_id_clone = session_id.clone();
    let mut full_response = String::new();

    let on_token = |token: String| {
        full_response.push_str(&token);
        let _ = app_clone.emit("stream-token", StreamToken {
            session_id: session_id_clone.clone(),
            token,
            done: false,
        });
    };

    let result = if api_format == "anthropic" {
        adapters::anthropic::chat_stream(&api_key, &model_name, &system_prompt, messages, on_token).await
    } else {
        adapters::openai::chat_stream(&base_url, &api_key, &model_name, &system_prompt, messages, on_token).await
    };

    // Emit done event
    let _ = app.emit("stream-token", StreamToken {
        session_id: session_id.clone(),
        token: String::new(),
        done: true,
    });

    if let Err(e) = result {
        return Err(e.to_string());
    }

    // Save assistant message
    let asst_id = Uuid::new_v4().to_string();
    let now2 = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&asst_id)
        .bind(&session_id)
        .bind("assistant")
        .bind(&full_response)
        .bind(&now2)
        .execute(&db.0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_messages(
    session_id: String,
    db: State<'_, DbState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT role, content, created_at FROM messages WHERE session_id = ? ORDER BY created_at ASC"
    )
    .bind(&session_id)
    .fetch_all(&db.0)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.iter().map(|(role, content, created_at)| {
        json!({"role": role, "content": content, "created_at": created_at})
    }).collect())
}
```

**Step 6: Update main.rs with all commands**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod commands;
mod adapters;

use commands::skills::DbState;

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let pool = tauri::async_runtime::block_on(db::init_db(app.handle()))
                .expect("failed to init db");
            app.manage(DbState(pool));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::skills::install_skill,
            commands::skills::list_skills,
            commands::skills::delete_skill,
            commands::models::save_model_config,
            commands::models::list_model_configs,
            commands::models::delete_model_config,
            commands::chat::create_session,
            commands::chat::send_message,
            commands::chat::get_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 7: Compile check**

```bash
cd apps/runtime/src-tauri
cargo build
```

**Step 8: Commit**

```bash
git add apps/runtime/src-tauri/
git commit -m "feat: runtime chat commands, model config, anthropic/openai adapters"
```

---

## Task 9: Runtime Frontend (React UI)

**Files:**
- Create: `apps/runtime/src/main.tsx`
- Create: `apps/runtime/src/App.tsx`
- Create: `apps/runtime/src/components/Sidebar.tsx`
- Create: `apps/runtime/src/components/ChatView.tsx`
- Create: `apps/runtime/src/components/InstallDialog.tsx`
- Create: `apps/runtime/src/components/SettingsView.tsx`
- Create: `apps/runtime/index.html`
- Create: `apps/runtime/src/index.css`
- Create: same vite.config.ts / tailwind.config.js / tsconfig.json as Studio (port 5174)

**Step 1: Create src/components/InstallDialog.tsx**

```tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

interface Props {
  onInstalled: () => void;
  onClose: () => void;
}

export function InstallDialog({ onInstalled, onClose }: Props) {
  const [packPath, setPackPath] = useState("");
  const [username, setUsername] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function pickFile() {
    const f = await open({ filters: [{ name: "SkillPack", extensions: ["skillpack"] }] });
    if (f && typeof f === "string") setPackPath(f);
  }

  async function handleInstall() {
    if (!packPath || !username.trim()) {
      setError("请选择文件并填写用户名");
      return;
    }
    setLoading(true);
    setError("");
    try {
      await invoke("install_skill", { packPath, username });
      onInstalled();
      onClose();
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-slate-800 rounded-lg p-6 w-96 space-y-4 border border-slate-600">
        <h2 className="font-semibold text-lg">安装 Skill</h2>
        <div>
          <button
            onClick={pickFile}
            className="w-full border border-dashed border-slate-500 rounded p-3 text-sm text-slate-400 hover:border-blue-500 hover:text-blue-400 transition-colors"
          >
            {packPath ? packPath.split(/[\\/]/).pop() : "选择 .skillpack 文件"}
          </button>
        </div>
        <div>
          <label className="block text-xs text-slate-400 mb-1">用户名（创作者提供）</label>
          <input
            className="w-full bg-slate-700 border border-slate-600 rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder="例如：alice"
          />
        </div>
        {error && <div className="text-red-400 text-sm">{error}</div>}
        <div className="flex gap-2">
          <button
            onClick={onClose}
            className="flex-1 bg-slate-700 hover:bg-slate-600 py-2 rounded text-sm transition-colors"
          >
            取消
          </button>
          <button
            onClick={handleInstall}
            disabled={loading}
            className="flex-1 bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 py-2 rounded text-sm transition-colors"
          >
            {loading ? "安装中..." : "安装"}
          </button>
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Create src/components/Sidebar.tsx**

```tsx
import { SkillManifest } from "../types";

interface Props {
  skills: SkillManifest[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onInstall: () => void;
  onSettings: () => void;
}

export function Sidebar({ skills, selectedId, onSelect, onInstall, onSettings }: Props) {
  return (
    <div className="w-56 bg-slate-800 flex flex-col h-full border-r border-slate-700">
      <div className="px-4 py-3 text-xs font-medium text-slate-400 border-b border-slate-700">
        已安装 Skill
      </div>
      <div className="flex-1 overflow-y-auto py-2">
        {skills.length === 0 && (
          <div className="px-4 py-3 text-xs text-slate-500">暂无已安装 Skill</div>
        )}
        {skills.map((s) => (
          <button
            key={s.id}
            onClick={() => onSelect(s.id)}
            className={`w-full text-left px-4 py-2.5 text-sm transition-colors ${
              selectedId === s.id
                ? "bg-blue-600/30 text-blue-300"
                : "text-slate-300 hover:bg-slate-700"
            }`}
          >
            <div className="font-medium truncate">🤖 {s.name}</div>
            <div className="text-xs text-slate-500 truncate">{s.version}</div>
          </button>
        ))}
      </div>
      <div className="p-3 space-y-2 border-t border-slate-700">
        <button
          onClick={onInstall}
          className="w-full bg-blue-600 hover:bg-blue-700 text-sm py-1.5 rounded transition-colors"
        >
          + 安装 Skill
        </button>
        <button
          onClick={onSettings}
          className="w-full bg-slate-700 hover:bg-slate-600 text-sm py-1.5 rounded transition-colors"
        >
          ⚙ 设置
        </button>
      </div>
    </div>
  );
}
```

**Step 3: Create src/types.ts**

```ts
export interface SkillManifest {
  id: string;
  name: string;
  description: string;
  version: string;
  author: string;
  recommended_model: string;
  tags: string[];
  created_at: string;
  username_hint?: string;
}

export interface ModelConfig {
  id: string;
  name: string;
  api_format: string;
  base_url: string;
  model_name: string;
  is_default: boolean;
}

export interface Message {
  role: "user" | "assistant";
  content: string;
  created_at: string;
}
```

**Step 4: Create src/components/ChatView.tsx**

```tsx
import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ReactMarkdown from "react-markdown";
import { SkillManifest, ModelConfig, Message } from "../types";

interface Props {
  skill: SkillManifest;
  models: ModelConfig[];
}

export function ChatView({ skill, models }: Props) {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [streamBuffer, setStreamBuffer] = useState("");
  const [selectedModelId, setSelectedModelId] = useState(models[0]?.id ?? "");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    startNewSession();
  }, [skill.id]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamBuffer]);

  useEffect(() => {
    const unlisten = listen<{ session_id: string; token: string; done: boolean }>(
      "stream-token",
      ({ payload }) => {
        if (payload.session_id !== sessionId) return;
        if (payload.done) {
          setMessages((prev) => [
            ...prev,
            { role: "assistant", content: streamBuffer + payload.token, created_at: new Date().toISOString() },
          ]);
          setStreamBuffer("");
          setStreaming(false);
        } else {
          setStreamBuffer((b) => b + payload.token);
        }
      }
    );
    return () => { unlisten.then((fn) => fn()); };
  }, [sessionId, streamBuffer]);

  async function startNewSession() {
    const id = await invoke<string>("create_session", {
      skillId: skill.id,
      modelId: selectedModelId,
    });
    setSessionId(id);
    setMessages([]);
    setStreamBuffer("");
  }

  async function handleSend() {
    if (!input.trim() || streaming || !sessionId) return;
    const msg = input.trim();
    setInput("");
    setMessages((prev) => [...prev, { role: "user", content: msg, created_at: new Date().toISOString() }]);
    setStreaming(true);
    setStreamBuffer("");
    try {
      await invoke("send_message", { sessionId, userMessage: msg });
    } catch (e) {
      setStreaming(false);
      setMessages((prev) => [...prev, { role: "assistant", content: `错误: ${e}`, created_at: new Date().toISOString() }]);
    }
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-3 border-b border-slate-700 bg-slate-800">
        <div>
          <span className="font-medium">{skill.name}</span>
          <span className="text-xs text-slate-400 ml-2">v{skill.version}</span>
        </div>
        <div className="flex items-center gap-2">
          <select
            value={selectedModelId}
            onChange={(e) => setSelectedModelId(e.target.value)}
            className="bg-slate-700 text-sm rounded px-2 py-1 border border-slate-600 focus:outline-none"
          >
            {models.map((m) => (
              <option key={m.id} value={m.id}>{m.name}</option>
            ))}
          </select>
          <button
            onClick={startNewSession}
            className="text-sm bg-slate-700 hover:bg-slate-600 px-3 py-1 rounded transition-colors"
          >
            新建会话
          </button>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-6 space-y-4">
        {messages.map((m, i) => (
          <div key={i} className={`flex ${m.role === "user" ? "justify-end" : "justify-start"}`}>
            <div
              className={`max-w-2xl rounded-lg px-4 py-2 text-sm ${
                m.role === "user"
                  ? "bg-blue-600 text-white"
                  : "bg-slate-700 text-slate-100"
              }`}
            >
              {m.role === "assistant" ? (
                <ReactMarkdown>{m.content}</ReactMarkdown>
              ) : (
                m.content
              )}
            </div>
          </div>
        ))}
        {streamBuffer && (
          <div className="flex justify-start">
            <div className="max-w-2xl bg-slate-700 rounded-lg px-4 py-2 text-sm text-slate-100">
              <ReactMarkdown>{streamBuffer}</ReactMarkdown>
              <span className="animate-pulse">▌</span>
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <div className="px-6 py-4 border-t border-slate-700 bg-slate-800">
        <div className="flex gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); } }}
            placeholder="输入消息... (Enter 发送，Shift+Enter 换行)"
            rows={1}
            className="flex-1 bg-slate-700 border border-slate-600 rounded px-3 py-2 text-sm resize-none focus:outline-none focus:border-blue-500"
          />
          <button
            onClick={handleSend}
            disabled={streaming || !input.trim()}
            className="bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 px-4 rounded text-sm font-medium transition-colors"
          >
            发送
          </button>
        </div>
      </div>
    </div>
  );
}
```

**Step 5: Create src/components/SettingsView.tsx**

```tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ModelConfig } from "../types";

interface Props {
  onClose: () => void;
}

export function SettingsView({ onClose }: Props) {
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [form, setForm] = useState({
    name: "", api_format: "openai", base_url: "https://api.openai.com/v1",
    model_name: "gpt-4o-mini", api_key: "",
  });
  const [error, setError] = useState("");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<boolean | null>(null);

  useEffect(() => { loadModels(); }, []);

  async function loadModels() {
    const list = await invoke<ModelConfig[]>("list_model_configs");
    setModels(list);
  }

  async function handleSave() {
    try {
      await invoke("save_model_config", {
        config: {
          id: "", name: form.name, api_format: form.api_format,
          base_url: form.base_url, model_name: form.model_name, is_default: models.length === 0,
        },
        apiKey: form.api_key,
      });
      setForm({ name: "", api_format: "openai", base_url: "https://api.openai.com/v1", model_name: "gpt-4o-mini", api_key: "" });
      loadModels();
    } catch (e: unknown) {
      setError(String(e));
    }
  }

  async function handleDelete(id: string) {
    await invoke("delete_model_config", { modelId: id });
    loadModels();
  }

  const inputCls = "w-full bg-slate-700 border border-slate-600 rounded px-3 py-1.5 text-sm focus:outline-none focus:border-blue-500";
  const labelCls = "block text-xs text-slate-400 mb-1";

  return (
    <div className="flex flex-col h-full p-6 overflow-y-auto">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-lg font-semibold">模型配置</h2>
        <button onClick={onClose} className="text-slate-400 hover:text-white text-sm">← 返回</button>
      </div>

      {/* Existing models */}
      {models.length > 0 && (
        <div className="mb-6 space-y-2">
          <div className="text-xs text-slate-400 mb-2">已配置模型</div>
          {models.map((m) => (
            <div key={m.id} className="flex items-center justify-between bg-slate-800 rounded px-3 py-2 text-sm">
              <div>
                <span className="font-medium">{m.name}</span>
                <span className="text-slate-400 ml-2">{m.model_name}</span>
              </div>
              <button onClick={() => handleDelete(m.id)} className="text-red-400 hover:text-red-300 text-xs">删除</button>
            </div>
          ))}
        </div>
      )}

      {/* Add model form */}
      <div className="bg-slate-800 rounded-lg p-4 space-y-3">
        <div className="text-xs font-medium text-slate-400 mb-2">添加模型</div>
        <div>
          <label className={labelCls}>名称</label>
          <input className={inputCls} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="例如：GPT-4o Mini" />
        </div>
        <div>
          <label className={labelCls}>API 格式</label>
          <select className={inputCls} value={form.api_format} onChange={(e) => setForm({ ...form, api_format: e.target.value })}>
            <option value="openai">OpenAI 兼容</option>
            <option value="anthropic">Anthropic (Claude)</option>
          </select>
        </div>
        <div>
          <label className={labelCls}>Base URL</label>
          <input className={inputCls} value={form.base_url} onChange={(e) => setForm({ ...form, base_url: e.target.value })} />
        </div>
        <div>
          <label className={labelCls}>模型名称</label>
          <input className={inputCls} value={form.model_name} onChange={(e) => setForm({ ...form, model_name: e.target.value })} />
        </div>
        <div>
          <label className={labelCls}>API Key</label>
          <input className={inputCls} type="password" value={form.api_key} onChange={(e) => setForm({ ...form, api_key: e.target.value })} />
        </div>
        {error && <div className="text-red-400 text-xs">{error}</div>}
        {testResult !== null && (
          <div className={`text-xs ${testResult ? "text-green-400" : "text-red-400"}`}>
            {testResult ? "✅ 连接成功" : "❌ 连接失败，请检查配置"}
          </div>
        )}
        <div className="flex gap-2 pt-1">
          <button
            onClick={handleSave}
            className="flex-1 bg-blue-600 hover:bg-blue-700 text-sm py-1.5 rounded transition-colors"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
```

**Step 6: Create src/App.tsx**

```tsx
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sidebar } from "./components/Sidebar";
import { ChatView } from "./components/ChatView";
import { InstallDialog } from "./components/InstallDialog";
import { SettingsView } from "./components/SettingsView";
import { SkillManifest, ModelConfig } from "./types";

export default function App() {
  const [skills, setSkills] = useState<SkillManifest[]>([]);
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const [showInstall, setShowInstall] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

  useEffect(() => {
    loadSkills();
    loadModels();
  }, []);

  async function loadSkills() {
    const list = await invoke<SkillManifest[]>("list_skills");
    setSkills(list);
  }

  async function loadModels() {
    const list = await invoke<ModelConfig[]>("list_model_configs");
    setModels(list);
  }

  const selectedSkill = skills.find((s) => s.id === selectedSkillId) ?? null;

  return (
    <div className="flex h-screen bg-slate-900 text-slate-100 overflow-hidden">
      <Sidebar
        skills={skills}
        selectedId={selectedSkillId}
        onSelect={setSelectedSkillId}
        onInstall={() => setShowInstall(true)}
        onSettings={() => setShowSettings(true)}
      />
      <div className="flex-1 overflow-hidden">
        {showSettings ? (
          <SettingsView onClose={() => { setShowSettings(false); loadModels(); }} />
        ) : selectedSkill && models.length > 0 ? (
          <ChatView skill={selectedSkill} models={models} />
        ) : selectedSkill && models.length === 0 ? (
          <div className="flex items-center justify-center h-full text-slate-400 text-sm">
            请先在设置中配置模型和 API Key
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-slate-400 text-sm">
            从左侧选择一个 Skill 开始对话
          </div>
        )}
      </div>
      {showInstall && (
        <InstallDialog
          onInstalled={loadSkills}
          onClose={() => setShowInstall(false)}
        />
      )}
    </div>
  );
}
```

**Step 7: Start Runtime in dev mode**

```bash
cd apps/runtime
cargo tauri dev
```

Expected: Window opens. Click "安装 Skill" → select `.skillpack` → enter username → Skill appears in sidebar → select it → configure model in settings → chat works with streaming.

**Step 8: End-to-end smoke test**

1. In Studio: select a skill dir, fill form, pack → `test.skillpack`
2. In Runtime: install `test.skillpack` with correct username → success
3. In Runtime: try wrong username → "用户名错误" error
4. In Runtime: configure a model (OpenAI/Anthropic) → send message → see streaming response

**Step 9: Commit**

```bash
git add apps/runtime/src/
git commit -m "feat: runtime frontend — sidebar, chat view, install dialog, settings"
```

---

## Task 10: System Prompt from Skill Content

**Problem:** Task 8 used `manifest.description` as system prompt. We need to use the actual SKILL.md content.

**Fix:** Store the skillpack file path in the DB and re-unpack on each chat (memory-safe: decrypt on demand, not persisted).

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/skills.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`

**Step 1: Update installed_skills table to store pack_path**

Add migration in `db.rs` — add `pack_path` column:

```rust
sqlx::query(
    "ALTER TABLE installed_skills ADD COLUMN pack_path TEXT NOT NULL DEFAULT ''"
)
.execute(&pool)
.await
.ok(); // ok() because column may already exist
```

**Step 2: Update install_skill command to store pack_path**

In `commands/skills.rs`, add `pack_path: pack_path.clone()` to the INSERT and include it in the query.

**Step 3: Update send_message to use actual SKILL.md**

In `commands/chat.rs`, replace system prompt derivation:

```rust
let (manifest_json, username, pack_path) = sqlx::query_as::<_, (String, String, String)>(
    "SELECT manifest, username, pack_path FROM installed_skills WHERE id = ?"
)
.bind(&skill_id)
.fetch_one(&db.0)
.await
.map_err(|e| e.to_string())?;

let unpacked = skillpack_rs::verify_and_unpack(&pack_path, &username)
    .map_err(|e| e.to_string())?;

let system_prompt = String::from_utf8_lossy(
    unpacked.files.get("SKILL.md").map(|v| v.as_slice()).unwrap_or_default()
).to_string();
```

**Step 4: Test again**

Start a new chat. Verify the AI response reflects the actual Skill content, not just the description.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/
git commit -m "fix: use actual SKILL.md content as system prompt via re-unpack"
```

---

## Task 11: Final Polish & Verification

**Step 1: Test connection in Settings**

Add `test_connection` command to Rust and wire up the "测试连接" button in SettingsView.

In `commands/models.rs`:

```rust
#[tauri::command]
pub async fn test_connection_cmd(config: ModelConfig, api_key: String) -> Result<bool, String> {
    if config.api_format == "anthropic" {
        crate::adapters::anthropic::test_connection(&api_key, &config.model_name)
            .await.map_err(|e| e.to_string())
    } else {
        crate::adapters::openai::test_connection(&config.base_url, &api_key, &config.model_name)
            .await.map_err(|e| e.to_string())
    }
}
```

Register it in `main.rs` and call it from SettingsView's test button.

**Step 2: Delete Skill from Runtime**

Add a "卸载" button in Sidebar (right-click or hover icon) that calls `delete_skill`.

**Step 3: Debug build verification**

```bash
# Studio
cd apps/studio && cargo tauri build --debug
# Runtime
cd apps/runtime && cargo tauri build --debug
```

Expected: Both build without errors. Installers appear in `target/debug/bundle/`.

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: MVP complete — Studio packing + Runtime install/chat with AES-256-GCM"
```

---

## Quick Reference

### Key file locations
- Crypto core: `packages/skillpack-rs/src/crypto.rs`
- Pack logic: `packages/skillpack-rs/src/pack.rs`
- Unpack logic: `packages/skillpack-rs/src/unpack.rs`
- Studio commands: `apps/studio/src-tauri/src/commands.rs`
- Runtime commands: `apps/runtime/src-tauri/src/commands/`
- Model adapters: `apps/runtime/src-tauri/src/adapters/`

### Run tests
```bash
cd packages/skillpack-rs && cargo test
```

### Start dev servers
```bash
# Terminal 1
cd apps/studio && cargo tauri dev

# Terminal 2
cd apps/runtime && cargo tauri dev
```
