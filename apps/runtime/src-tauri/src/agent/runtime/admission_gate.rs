use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct SessionAdmissionGate {
    active_sessions: Mutex<HashSet<String>>,
}

impl SessionAdmissionGate {
    pub fn is_reserved(&self, session_id: &str) -> bool {
        let session_id = normalize_session_id(session_id);
        if session_id.is_empty() {
            return false;
        }

        self.active_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(session_id)
    }

    pub fn try_acquire(
        self: &Arc<Self>,
        session_id: &str,
    ) -> Result<SessionAdmissionLease, SessionAdmissionConflict> {
        let session_id = normalize_session_id(session_id);
        if session_id.is_empty() {
            return Err(SessionAdmissionConflict::new(""));
        }

        let mut active_sessions = self
            .active_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !active_sessions.insert(session_id.to_string()) {
            return Err(SessionAdmissionConflict::new(session_id));
        }

        Ok(SessionAdmissionLease {
            gate: Arc::clone(self),
            session_id: session_id.to_string(),
            released: false,
        })
    }

    fn release(&self, session_id: &str) {
        let session_id = normalize_session_id(session_id);
        if session_id.is_empty() {
            return;
        }

        self.active_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(session_id);
    }
}

#[derive(Debug)]
pub struct SessionAdmissionLease {
    gate: Arc<SessionAdmissionGate>,
    session_id: String,
    released: bool,
}

impl SessionAdmissionLease {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn release(mut self) {
        self.release_inner();
    }

    fn release_inner(&mut self) {
        if self.released {
            return;
        }
        self.gate.release(&self.session_id);
        self.released = true;
    }
}

impl Drop for SessionAdmissionLease {
    fn drop(&mut self) {
        self.release_inner();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAdmissionConflict {
    session_id: String,
}

impl SessionAdmissionConflict {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        "SESSION_RUN_CONFLICT"
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn message(&self) -> &'static str {
        "当前会话仍在执行中，请等待当前任务完成后再发送新消息"
    }
}

impl fmt::Display for SessionAdmissionConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

#[derive(Debug, Clone)]
pub struct SessionAdmissionGateState(pub Arc<SessionAdmissionGate>);

fn normalize_session_id(session_id: &str) -> &str {
    session_id.trim()
}

#[cfg(test)]
mod tests {
    use super::SessionAdmissionGate;
    use std::sync::Arc;

    #[test]
    fn admission_gate_rejects_same_session_while_leased() {
        let gate = Arc::new(SessionAdmissionGate::default());
        let _lease = gate.try_acquire("session-1").expect("first lease");

        let conflict = gate
            .try_acquire("session-1")
            .expect_err("same session should conflict");

        assert_eq!(conflict.code(), "SESSION_RUN_CONFLICT");
        assert_eq!(conflict.session_id(), "session-1");
    }

    #[test]
    fn admission_gate_allows_different_sessions_in_parallel() {
        let gate = Arc::new(SessionAdmissionGate::default());
        let lease_a = gate.try_acquire("session-a").expect("lease a");
        let lease_b = gate.try_acquire("session-b").expect("lease b");

        assert_eq!(lease_a.session_id(), "session-a");
        assert_eq!(lease_b.session_id(), "session-b");
        assert!(gate.is_reserved("session-a"));
        assert!(gate.is_reserved("session-b"));
    }

    #[test]
    fn dropping_lease_releases_the_session() {
        let gate = Arc::new(SessionAdmissionGate::default());
        {
            let _lease = gate.try_acquire("session-1").expect("lease");
            assert!(gate.is_reserved("session-1"));
        }

        assert!(!gate.is_reserved("session-1"));
        gate.try_acquire("session-1")
            .expect("session should be available after lease drop");
    }
}
