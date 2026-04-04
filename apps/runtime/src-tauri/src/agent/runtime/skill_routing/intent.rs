use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RouteConfidence {
    pub score: f32,
}

impl RouteConfidence {
    pub fn new(score: f32) -> Self {
        Self { score }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvocationIntent {
    OpenTask,
    PromptSkillInline { skill_id: String },
    PromptSkillFork { skill_id: String },
    DirectDispatchSkill { skill_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteDecision {
    OpenTask { confidence: RouteConfidence },
    PromptSkillInline {
        skill_id: String,
        confidence: RouteConfidence,
    },
    PromptSkillFork {
        skill_id: String,
        confidence: RouteConfidence,
    },
    DirectDispatchSkill {
        skill_id: String,
        confidence: RouteConfidence,
    },
}

impl RouteDecision {
    pub fn confidence(&self) -> RouteConfidence {
        match self {
            Self::OpenTask { confidence }
            | Self::PromptSkillInline { confidence, .. }
            | Self::PromptSkillFork { confidence, .. }
            | Self::DirectDispatchSkill { confidence, .. } => *confidence,
        }
    }

    pub fn skill_id(&self) -> Option<&str> {
        match self {
            Self::OpenTask { .. } => None,
            Self::PromptSkillInline { skill_id, .. }
            | Self::PromptSkillFork { skill_id, .. }
            | Self::DirectDispatchSkill { skill_id, .. } => Some(skill_id.as_str()),
        }
    }

    pub fn intent(&self) -> InvocationIntent {
        match self {
            Self::OpenTask { .. } => InvocationIntent::OpenTask,
            Self::PromptSkillInline { skill_id, .. } => InvocationIntent::PromptSkillInline {
                skill_id: skill_id.clone(),
            },
            Self::PromptSkillFork { skill_id, .. } => InvocationIntent::PromptSkillFork {
                skill_id: skill_id.clone(),
            },
            Self::DirectDispatchSkill { skill_id, .. } => InvocationIntent::DirectDispatchSkill {
                skill_id: skill_id.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InvocationIntent, RouteConfidence, RouteDecision};

    #[test]
    fn route_decision_variants_cover_all_execution_lanes() {
        let confidence = RouteConfidence::new(0.93);

        let open_task = RouteDecision::OpenTask {
            confidence: confidence.clone(),
        };
        let inline = RouteDecision::PromptSkillInline {
            skill_id: "feishu-pm-weekly-work-summary".to_string(),
            confidence: confidence.clone(),
        };
        let fork = RouteDecision::PromptSkillFork {
            skill_id: "feishu-bitable-analyst".to_string(),
            confidence: confidence.clone(),
        };
        let dispatch = RouteDecision::DirectDispatchSkill {
            skill_id: "feishu-pm-task-dispatch".to_string(),
            confidence,
        };

        assert!(matches!(open_task, RouteDecision::OpenTask { .. }));
        assert!(matches!(inline, RouteDecision::PromptSkillInline { .. }));
        assert!(matches!(fork, RouteDecision::PromptSkillFork { .. }));
        assert!(matches!(dispatch, RouteDecision::DirectDispatchSkill { .. }));

        let intent = InvocationIntent::PromptSkillInline {
            skill_id: "feishu-pm-weekly-work-summary".to_string(),
        };
        assert!(matches!(intent, InvocationIntent::PromptSkillInline { .. }));
    }
}
