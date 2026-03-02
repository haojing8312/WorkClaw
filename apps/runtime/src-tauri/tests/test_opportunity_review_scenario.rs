use runtime_lib::im::orchestrator::advance_opportunity_stage;
use runtime_lib::im::scenarios::opportunity_review::{
    final_recommendation, OpportunityReviewInput, OpportunityStage,
};

#[test]
fn opportunity_review_reaches_final_recommendation_stage() {
    let input = OpportunityReviewInput {
        clarified: true,
        feasible: true,
        estimated_cost_range: "80-120万".to_string(),
        key_risks: vec!["需求边界可能变化".to_string()],
    };

    let stage1 = advance_opportunity_stage(OpportunityStage::Clarify, &input);
    assert_eq!(stage1, OpportunityStage::Feasibility);

    let stage2 = advance_opportunity_stage(stage1, &input);
    assert_eq!(stage2, OpportunityStage::CostRisk);

    let stage3 = advance_opportunity_stage(stage2, &input);
    assert_eq!(stage3, OpportunityStage::Recommendation);

    let summary = final_recommendation(&input);
    assert!(summary.contains("承接建议"));
    assert!(summary.contains("成本区间"));
    assert!(summary.contains("风险"));
}

