use crate::im::types::{ImEvent, ImEventType};
use crate::im::scenarios::opportunity_review::{next_stage, OpportunityReviewInput, OpportunityStage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestratorAction {
    ApplyOverride,
    PauseFlow,
    ResumeFlow,
    PrioritizeMentionedRole,
    ContinueAutoTurn,
    Ignore,
}

pub fn select_next_action(event: &ImEvent) -> OrchestratorAction {
    match event.event_type {
        ImEventType::HumanOverride => OrchestratorAction::ApplyOverride,
        ImEventType::CommandPause => OrchestratorAction::PauseFlow,
        ImEventType::CommandResume => OrchestratorAction::ResumeFlow,
        ImEventType::MentionRole => OrchestratorAction::PrioritizeMentionedRole,
        ImEventType::MessageCreated => OrchestratorAction::ContinueAutoTurn,
    }
}

pub fn resolve_next_action(events: &[ImEvent]) -> OrchestratorAction {
    let priority = [
        ImEventType::HumanOverride,
        ImEventType::CommandPause,
        ImEventType::CommandResume,
        ImEventType::MentionRole,
        ImEventType::MessageCreated,
    ];

    for event_type in priority {
        if let Some(event) = events.iter().find(|e| e.event_type == event_type) {
            return select_next_action(event);
        }
    }

    OrchestratorAction::Ignore
}

pub fn advance_opportunity_stage(
    current: OpportunityStage,
    input: &OpportunityReviewInput,
) -> OpportunityStage {
    next_stage(current, input)
}
