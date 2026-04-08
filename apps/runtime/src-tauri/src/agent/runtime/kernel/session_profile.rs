#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SessionSurfaceKind {
    #[default]
    LocalChat,
    HiddenChildSession,
    EmployeeStepSession,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SessionExecutionProfile {
    pub surface: SessionSurfaceKind,
}

impl SessionExecutionProfile {
    pub(crate) fn for_surface(surface: SessionSurfaceKind) -> Self {
        Self { surface }
    }
}
