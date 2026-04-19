use crate::agent::tools::search_providers::cache::SearchCache;
use crate::agent::tools::AskUserResponder;
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    Initialized,
}

#[derive(Serialize, Clone)]
pub struct StreamToken {
    pub session_id: String,
    pub token: String,
    pub done: bool,
    #[serde(default)]
    pub sub_agent: bool,
}

pub type ToolConfirmResponder = Arc<Mutex<Option<std::sync::mpsc::Sender<bool>>>>;

pub struct AskUserState(pub AskUserResponder);

pub struct AskUserPendingSessionState(pub Arc<Mutex<Option<String>>>);

pub struct SearchCacheState(pub Arc<SearchCache>);

pub struct CancelFlagState(pub Arc<AtomicBool>);

#[derive(Serialize, Clone, Debug)]
pub struct SkillRouteEvent {
    pub session_id: String,
    pub route_run_id: String,
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub skill_name: String,
    pub depth: usize,
    pub status: String,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}
