#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SessionSurfaceKind {
    #[default]
    LocalChat,
    HiddenChildSession,
    EmployeeStepSession,
}

impl SessionSurfaceKind {
    #[allow(dead_code)]
    pub(crate) fn journal_key(self) -> &'static str {
        match self {
            SessionSurfaceKind::LocalChat => "local_chat",
            SessionSurfaceKind::HiddenChildSession => "hidden_child_session",
            SessionSurfaceKind::EmployeeStepSession => "employee_step_session",
        }
    }

    pub(crate) fn from_journal_key(key: Option<&str>) -> Self {
        match key.map(str::trim).filter(|value| !value.is_empty()) {
            Some("hidden_child_session") => SessionSurfaceKind::HiddenChildSession,
            Some("employee_step_session") => SessionSurfaceKind::EmployeeStepSession,
            _ => SessionSurfaceKind::LocalChat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SessionContinuationProfile {
    #[default]
    LocalChat,
    HiddenChildSession,
    EmployeeStepSession,
}

impl SessionContinuationProfile {
    pub(crate) fn for_surface(surface: SessionSurfaceKind) -> Self {
        match surface {
            SessionSurfaceKind::LocalChat => SessionContinuationProfile::LocalChat,
            SessionSurfaceKind::HiddenChildSession => {
                SessionContinuationProfile::HiddenChildSession
            }
            SessionSurfaceKind::EmployeeStepSession => {
                SessionContinuationProfile::EmployeeStepSession
            }
        }
    }

    pub(crate) fn allows_compaction_runtime_notes(self) -> bool {
        matches!(self, SessionContinuationProfile::LocalChat)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SessionExecutionProfile {
    pub surface: SessionSurfaceKind,
    pub continuation_mode: SessionContinuationProfile,
}

impl SessionExecutionProfile {
    pub(crate) fn for_surface(surface: SessionSurfaceKind) -> Self {
        Self {
            surface,
            continuation_mode: SessionContinuationProfile::for_surface(surface),
        }
    }

    pub(crate) fn local_chat() -> Self {
        Self::for_surface(SessionSurfaceKind::LocalChat)
    }

    pub(crate) fn hidden_child_session() -> Self {
        Self::for_surface(SessionSurfaceKind::HiddenChildSession)
    }

    pub(crate) fn employee_step_session() -> Self {
        Self::for_surface(SessionSurfaceKind::EmployeeStepSession)
    }
}
