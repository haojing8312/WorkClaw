use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubSkillSummary {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub github_url: Option<String>,
    pub source_url: Option<String>,
    #[serde(default)]
    pub stars: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubLibraryItem {
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub github_url: Option<String>,
    pub source_url: Option<String>,
    pub tags: Vec<String>,
    pub stars: i64,
    pub downloads: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubLibraryResponse {
    pub items: Vec<ClawhubLibraryItem>,
    pub next_cursor: Option<String>,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillhubCatalogSyncStatus {
    pub total_skills: usize,
    pub last_synced_at: Option<String>,
    pub refreshed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubSkillDetail {
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub author: Option<String>,
    pub github_url: Option<String>,
    pub source_url: Option<String>,
    pub updated_at: Option<String>,
    pub stars: i64,
    pub downloads: i64,
    pub tags: Vec<String>,
    pub readme: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubSkillRecommendation {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub stars: i64,
    pub score: f64,
    pub reason: String,
    pub github_url: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClawhubUpdateStatus {
    pub skill_id: String,
    pub has_update: bool,
    pub local_hash: String,
    pub remote_hash: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSkillDir {
    pub name: String,
    pub dir_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRepoSkippedImport {
    pub dir_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRepoInstallResult {
    pub repo_dir: String,
    pub detected_skills: Vec<DiscoveredSkillDir>,
    pub imported_manifests: Vec<skillpack_rs::SkillManifest>,
    pub skipped: Vec<GithubRepoSkippedImport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRepoDownloadResult {
    pub repo_dir: String,
    pub detected_skills: Vec<DiscoveredSkillDir>,
}

#[derive(Debug, Clone)]
pub(crate) struct SkillhubCatalogIndexRow {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) summary: String,
    pub(crate) description: String,
    pub(crate) github_url: Option<String>,
    pub(crate) source_url: Option<String>,
    pub(crate) tags_json: String,
    pub(crate) stars: i64,
    pub(crate) downloads: i64,
    pub(crate) updated_at: Option<String>,
    pub(crate) synced_at: String,
}
