#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRunRequest {
    pub group_id: String,
    pub coordinator_employee_id: String,
    pub member_employee_ids: Vec<String>,
    pub user_goal: String,
    pub execution_window: usize,
    pub timeout_employee_ids: Vec<String>,
    pub max_retry_per_step: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupRunState {
    Planning,
    Executing,
    Synthesizing,
    Done,
    Failed,
}

impl GroupRunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            GroupRunState::Planning => "planning",
            GroupRunState::Executing => "executing",
            GroupRunState::Synthesizing => "synthesizing",
            GroupRunState::Done => "done",
            GroupRunState::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupPlanItem {
    pub id: String,
    pub assignee_employee_id: String,
    pub objective: String,
    pub acceptance: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupExecutionItem {
    pub id: String,
    pub round_no: i64,
    pub assignee_employee_id: String,
    pub status: String,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRunOutcome {
    pub states: Vec<GroupRunState>,
    pub plan: Vec<GroupPlanItem>,
    pub execution: Vec<GroupExecutionItem>,
    pub final_report: String,
}

pub fn simulate_group_run(request: GroupRunRequest) -> GroupRunOutcome {
    let members = normalize_members(
        request.coordinator_employee_id.as_str(),
        &request.member_employee_ids,
    );

    if members.is_empty() {
        return GroupRunOutcome {
            states: vec![
                GroupRunState::Planning,
                GroupRunState::Failed,
            ],
            plan: Vec::new(),
            execution: Vec::new(),
            final_report: "计划：无可用成员\n执行：未开始\n汇报：执行失败，原因=无可用成员".to_string(),
        };
    }

    let states = vec![
        GroupRunState::Planning,
        GroupRunState::Executing,
        GroupRunState::Synthesizing,
        GroupRunState::Done,
    ];

    let mut plan = Vec::with_capacity(members.len());
    let mut execution = Vec::with_capacity(members.len());
    let timeout_targets = normalize_timeouts(&request.timeout_employee_ids);
    let window = request.execution_window.clamp(1, 10);
    let retry_limit = request.max_retry_per_step.min(3);
    let mut failed_members: Vec<String> = Vec::new();
    for (idx, assignee) in members.iter().enumerate() {
        let step_id = format!("step-{}", idx + 1);
        let round_no = ((idx / window) as i64) + 1;
        plan.push(GroupPlanItem {
            id: step_id.clone(),
            assignee_employee_id: assignee.clone(),
            objective: format!("围绕目标执行子任务：{}", request.user_goal),
            acceptance: "输出可验证结果与下一步建议".to_string(),
        });
        if timeout_targets.contains(assignee) {
            failed_members.push(assignee.clone());
            execution.push(GroupExecutionItem {
                id: step_id,
                round_no,
                assignee_employee_id: assignee.clone(),
                status: "failed".to_string(),
                output: format!(
                    "{} 在第 {} 轮执行超时，重试{}次后仍失败",
                    assignee, round_no, retry_limit
                ),
            });
        } else {
            execution.push(GroupExecutionItem {
                id: step_id,
                round_no,
                assignee_employee_id: assignee.clone(),
                status: "completed".to_string(),
                output: format!("{} 已完成第 {} 轮执行", assignee, round_no),
            });
        }
    }

    let mut final_report = format!(
        "计划：共 {} 步，协调员={}。\n执行：已完成 {} 步。\n汇报：目标“{}”已产出阶段结果，建议进入交付复核。",
        plan.len(),
        request.coordinator_employee_id,
        execution.iter().filter(|item| item.status == "completed").count(),
        request.user_goal
    );
    if !failed_members.is_empty() {
        final_report.push_str(&format!(
            "\n未完成项：{}。\n补救建议：由协调员改派可用成员或缩减范围后重试。",
            failed_members.join(", ")
        ));
    }

    GroupRunOutcome {
        states,
        plan,
        execution,
        final_report,
    }
}

fn normalize_members(coordinator: &str, members: &[String]) -> Vec<String> {
    use std::collections::HashSet;
    let coordinator = coordinator.trim().to_lowercase();
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    if !coordinator.is_empty() {
        seen.insert(coordinator.clone());
        out.push(coordinator);
    }

    for member in members {
        let normalized = member.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
        if out.len() >= 10 {
            break;
        }
    }

    out
}

fn normalize_timeouts(raw: &[String]) -> std::collections::HashSet<String> {
    raw.iter()
        .map(|item| item.trim().to_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<std::collections::HashSet<_>>()
}

#[cfg(test)]
mod tests {
    use super::{simulate_group_run, GroupRunRequest, GroupRunState};

    #[test]
    fn simulate_group_run_has_required_phases_and_report_sections() {
        let outcome = simulate_group_run(GroupRunRequest {
            group_id: "g1".to_string(),
            coordinator_employee_id: "project_manager".to_string(),
            member_employee_ids: vec![
                "project_manager".to_string(),
                "dev_team".to_string(),
                "qa_team".to_string(),
            ],
            user_goal: "发布协作功能".to_string(),
            execution_window: 3,
            timeout_employee_ids: Vec::new(),
            max_retry_per_step: 1,
        });

        assert_eq!(
            outcome.states,
            vec![
                GroupRunState::Planning,
                GroupRunState::Executing,
                GroupRunState::Synthesizing,
                GroupRunState::Done,
            ]
        );
        assert!(outcome.final_report.contains("计划"));
        assert!(outcome.final_report.contains("执行"));
        assert!(outcome.final_report.contains("汇报"));
    }
}
