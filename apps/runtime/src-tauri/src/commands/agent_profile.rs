use crate::commands::employee_agents::{list_agent_employees_with_pool, AgentEmployee};
use crate::commands::runtime_preferences::resolve_default_work_dir_with_pool;
use crate::commands::skills::DbState;
use sqlx::SqlitePool;
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AgentProfileAnswerInput {
    pub key: String,
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AgentProfilePayload {
    pub employee_db_id: String,
    pub answers: Vec<AgentProfileAnswerInput>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AgentProfileDraft {
    pub employee_id: String,
    pub employee_name: String,
    pub agents_md: String,
    pub soul_md: String,
    pub user_md: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AgentProfileFileResult {
    pub path: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ApplyAgentProfileResult {
    pub files: Vec<AgentProfileFileResult>,
}

fn normalized_answer(answers: &[AgentProfileAnswerInput], key: &str) -> String {
    answers
        .iter()
        .find(|item| item.key.trim().eq_ignore_ascii_case(key))
        .map(|item| item.answer.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_default()
}

fn render_markdown(
    employee: &AgentEmployee,
    answers: &[AgentProfileAnswerInput],
) -> AgentProfileDraft {
    let mission = normalized_answer(answers, "mission");
    let responsibilities = normalized_answer(answers, "responsibilities");
    let collaboration = normalized_answer(answers, "collaboration");
    let tone = normalized_answer(answers, "tone");
    let boundaries = normalized_answer(answers, "boundaries");
    let user_profile = normalized_answer(answers, "user_profile");

    let agents_md = format!(
        "# AGENTS\n\n## Agent\n- 名称: {name}\n- 员工编号: {employee_id}\n- 飞书范围: feishu\n\n## Mission\n{mission}\n\n## Responsibilities\n{responsibilities}\n\n## Collaboration\n{collaboration}\n",
        name = employee.name,
        employee_id = employee.employee_id,
        mission = if mission.is_empty() { "请补充该员工的核心使命。" } else { mission.as_str() },
        responsibilities = if responsibilities.is_empty() { "请补充该员工的关键职责。" } else { responsibilities.as_str() },
        collaboration = if collaboration.is_empty() { "请补充该员工的协作方式与升级路径。" } else { collaboration.as_str() },
    );

    let soul_md = format!(
        "# SOUL\n\n## Tone\n{tone}\n\n## Boundaries\n{boundaries}\n\n## Operating Principles\n1. 先澄清上下文，再执行。\n2. 输出可执行步骤与验收标准。\n3. 遇到风险先预警，再给替代方案。\n",
        tone = if tone.is_empty() { "专业、简洁、可执行。" } else { tone.as_str() },
        boundaries = if boundaries.is_empty() { "不编造事实；权限不明时先确认；高风险操作必须二次确认。" } else { boundaries.as_str() },
    );

    let user_md = format!(
        "# USER\n\n## User Profile\n{user_profile}\n\n## Communication Preferences\n- 先结论，后细节\n- 默认给出下一步执行建议\n- 对关键决策提供利弊权衡\n",
        user_profile = if user_profile.is_empty() { "面向业务与产品协作场景，关注交付结果与效率。" } else { user_profile.as_str() },
    );

    AgentProfileDraft {
        employee_id: employee.employee_id.clone(),
        employee_name: employee.name.clone(),
        agents_md,
        soul_md,
        user_md,
    }
}

async fn find_employee_with_pool(
    pool: &SqlitePool,
    employee_db_id: &str,
) -> Result<AgentEmployee, String> {
    let rows = list_agent_employees_with_pool(pool).await?;
    rows.into_iter()
        .find(|item| item.id == employee_db_id)
        .ok_or_else(|| "employee not found".to_string())
}

fn resolve_profile_dir(employee: &AgentEmployee, fallback_base: &str) -> PathBuf {
    let base = if employee.default_work_dir.trim().is_empty() {
        fallback_base.to_string()
    } else {
        employee.default_work_dir.trim().to_string()
    };
    PathBuf::from(base)
        .join("openclaw")
        .join(employee.employee_id.trim())
}

pub async fn generate_agent_profile_draft_with_pool(
    pool: &SqlitePool,
    payload: AgentProfilePayload,
) -> Result<AgentProfileDraft, String> {
    let employee = find_employee_with_pool(pool, payload.employee_db_id.trim()).await?;
    Ok(render_markdown(&employee, &payload.answers))
}

pub async fn apply_agent_profile_with_pool(
    pool: &SqlitePool,
    payload: AgentProfilePayload,
) -> Result<ApplyAgentProfileResult, String> {
    let employee = find_employee_with_pool(pool, payload.employee_db_id.trim()).await?;
    let draft = render_markdown(&employee, &payload.answers);
    let fallback_base = resolve_default_work_dir_with_pool(pool).await?;
    let profile_dir = resolve_profile_dir(&employee, &fallback_base);
    std::fs::create_dir_all(&profile_dir)
        .map_err(|e| format!("failed to create profile dir: {e}"))?;

    let mut files = Vec::with_capacity(3);
    let write_targets = [
        ("AGENTS.md", draft.agents_md),
        ("SOUL.md", draft.soul_md),
        ("USER.md", draft.user_md),
    ];

    for (name, content) in write_targets {
        let file_path = profile_dir.join(name);
        let path_text = file_path.to_string_lossy().to_string();
        match std::fs::write(&file_path, content.as_bytes()) {
            Ok(_) => files.push(AgentProfileFileResult {
                path: path_text,
                ok: true,
                error: None,
            }),
            Err(e) => files.push(AgentProfileFileResult {
                path: path_text,
                ok: false,
                error: Some(e.to_string()),
            }),
        }
    }

    Ok(ApplyAgentProfileResult { files })
}

#[tauri::command]
pub async fn generate_agent_profile_draft(
    payload: AgentProfilePayload,
    db: State<'_, DbState>,
) -> Result<AgentProfileDraft, String> {
    generate_agent_profile_draft_with_pool(&db.0, payload).await
}

#[tauri::command]
pub async fn apply_agent_profile(
    payload: AgentProfilePayload,
    db: State<'_, DbState>,
) -> Result<ApplyAgentProfileResult, String> {
    apply_agent_profile_with_pool(&db.0, payload).await
}
