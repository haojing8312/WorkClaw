use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct RouteConfidence(f32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteConfidenceError;

impl fmt::Display for RouteConfidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("route confidence must be finite and between 0.0 and 1.0")
    }
}

impl std::error::Error for RouteConfidenceError {}

impl RouteConfidence {
    pub fn new(score: f32) -> Result<Self, RouteConfidenceError> {
        Self::validate(score).map(Self)
    }

    pub fn score(self) -> f32 {
        self.0
    }

    fn validate(score: f32) -> Result<f32, RouteConfidenceError> {
        if score.is_finite() && (0.0..=1.0).contains(&score) {
            Ok(score)
        } else {
            Err(RouteConfidenceError)
        }
    }
}

impl TryFrom<f32> for RouteConfidence {
    type Error = RouteConfidenceError;

    fn try_from(score: f32) -> Result<Self, Self::Error> {
        Self::new(score)
    }
}

impl From<RouteConfidence> for f32 {
    fn from(confidence: RouteConfidence) -> Self {
        confidence.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RouteFallbackReason {
    ExplicitOpenTask,
    NoCandidates,
    AmbiguousCandidates,
    InvalidSkillContract,
    DispatchArgumentResolutionFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvocationIntent {
    OpenTask {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback_reason: Option<RouteFallbackReason>,
    },
    PromptSkillInline {
        skill_id: String,
    },
    PromptSkillFork {
        skill_id: String,
    },
    DirectDispatchSkill {
        skill_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteDecision {
    OpenTask {
        confidence: RouteConfidence,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback_reason: Option<RouteFallbackReason>,
    },
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
            Self::OpenTask { confidence, .. }
            | Self::PromptSkillInline { confidence, .. }
            | Self::PromptSkillFork { confidence, .. }
            | Self::DirectDispatchSkill { confidence, .. } => *confidence,
        }
    }

    pub fn fallback_reason(&self) -> Option<RouteFallbackReason> {
        match self {
            Self::OpenTask {
                fallback_reason, ..
            } => *fallback_reason,
            Self::PromptSkillInline { .. }
            | Self::PromptSkillFork { .. }
            | Self::DirectDispatchSkill { .. } => None,
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
            Self::OpenTask {
                fallback_reason, ..
            } => InvocationIntent::OpenTask {
                fallback_reason: *fallback_reason,
            },
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
    use super::{
        InvocationIntent, RouteConfidence, RouteConfidenceError, RouteDecision, RouteFallbackReason,
    };

    #[test]
    fn route_confidence_rejects_invalid_values() {
        assert_eq!(RouteConfidence::new(0.93).unwrap().score(), 0.93);
        assert_eq!(
            RouteConfidence::new(-0.1).unwrap_err(),
            RouteConfidenceError
        );
        assert_eq!(RouteConfidence::new(1.1).unwrap_err(), RouteConfidenceError);
        assert_eq!(
            RouteConfidence::new(f32::NAN).unwrap_err(),
            RouteConfidenceError
        );
        assert_eq!(
            RouteConfidence::new(f32::INFINITY).unwrap_err(),
            RouteConfidenceError
        );
    }

    #[test]
    fn route_confidence_deserialization_is_validated() {
        let confidence: RouteConfidence = serde_json::from_str("0.93").unwrap();
        assert_eq!(confidence.score(), 0.93);
        assert!(serde_json::from_str::<RouteConfidence>("1.1").is_err());
    }

    #[test]
    fn route_decision_accessors_cover_all_execution_lanes() {
        let confidence = RouteConfidence::new(0.93).unwrap();

        let open_task = RouteDecision::OpenTask {
            confidence,
            fallback_reason: Some(RouteFallbackReason::NoCandidates),
        };
        let inline = RouteDecision::PromptSkillInline {
            skill_id: "feishu-pm-weekly-work-summary".to_string(),
            confidence,
        };
        let fork = RouteDecision::PromptSkillFork {
            skill_id: "feishu-bitable-analyst".to_string(),
            confidence,
        };
        let dispatch = RouteDecision::DirectDispatchSkill {
            skill_id: "feishu-pm-task-dispatch".to_string(),
            confidence,
        };

        assert_eq!(open_task.confidence().score(), 0.93);
        assert_eq!(
            open_task.fallback_reason(),
            Some(RouteFallbackReason::NoCandidates)
        );
        assert_eq!(open_task.skill_id(), None);
        assert_eq!(
            open_task.intent(),
            InvocationIntent::OpenTask {
                fallback_reason: Some(RouteFallbackReason::NoCandidates),
            }
        );

        assert_eq!(inline.confidence().score(), 0.93);
        assert_eq!(inline.skill_id(), Some("feishu-pm-weekly-work-summary"));
        assert_eq!(
            inline.intent(),
            InvocationIntent::PromptSkillInline {
                skill_id: "feishu-pm-weekly-work-summary".to_string(),
            }
        );

        assert_eq!(fork.confidence().score(), 0.93);
        assert_eq!(fork.skill_id(), Some("feishu-bitable-analyst"));
        assert_eq!(
            fork.intent(),
            InvocationIntent::PromptSkillFork {
                skill_id: "feishu-bitable-analyst".to_string(),
            }
        );

        assert_eq!(dispatch.confidence().score(), 0.93);
        assert_eq!(dispatch.skill_id(), Some("feishu-pm-task-dispatch"));
        assert_eq!(
            dispatch.intent(),
            InvocationIntent::DirectDispatchSkill {
                skill_id: "feishu-pm-task-dispatch".to_string(),
            }
        );
    }
}
