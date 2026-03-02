#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpportunityStage {
    Clarify,
    Feasibility,
    CostRisk,
    Recommendation,
}

#[derive(Debug, Clone)]
pub struct OpportunityReviewInput {
    pub clarified: bool,
    pub feasible: bool,
    pub estimated_cost_range: String,
    pub key_risks: Vec<String>,
}

pub fn next_stage(current: OpportunityStage, input: &OpportunityReviewInput) -> OpportunityStage {
    match current {
        OpportunityStage::Clarify if input.clarified => OpportunityStage::Feasibility,
        OpportunityStage::Feasibility if input.feasible => OpportunityStage::CostRisk,
        OpportunityStage::CostRisk => OpportunityStage::Recommendation,
        _ => current,
    }
}

pub fn final_recommendation(input: &OpportunityReviewInput) -> String {
    let risk_text = if input.key_risks.is_empty() {
        "暂无关键风险".to_string()
    } else {
        input.key_risks.join("；")
    };
    format!(
        "承接建议: {}\n成本区间: {}\n风险: {}",
        if input.feasible { "建议承接" } else { "暂不承接" },
        input.estimated_cost_range,
        risk_text
    )
}

