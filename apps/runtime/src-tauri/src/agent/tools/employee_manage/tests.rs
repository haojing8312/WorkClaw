use super::*;
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use uuid::Uuid;

fn setup_pool() -> SqlitePool {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create runtime");

    rt.block_on(async {
        let db_path = std::env::temp_dir().join(format!(
            "employee-manage-tool-test-{}.db",
            Uuid::new_v4()
        ));
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("create sqlite memory pool");

        sqlx::query(
            "CREATE TABLE installed_skills (
                id TEXT PRIMARY KEY,
                manifest TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                last_used_at TEXT,
                username TEXT NOT NULL,
                pack_path TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'encrypted'
            )",
        )
        .execute(&pool)
        .await
        .expect("create installed_skills");

        sqlx::query(
            "CREATE TABLE agent_employees (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                role_id TEXT NOT NULL,
                persona TEXT NOT NULL DEFAULT '',
                feishu_open_id TEXT NOT NULL DEFAULT '',
                feishu_app_id TEXT NOT NULL DEFAULT '',
                feishu_app_secret TEXT NOT NULL DEFAULT '',
                primary_skill_id TEXT NOT NULL DEFAULT '',
                default_work_dir TEXT NOT NULL DEFAULT '',
                openclaw_agent_id TEXT NOT NULL DEFAULT '',
                routing_priority INTEGER NOT NULL DEFAULT 100,
                enabled_scopes_json TEXT NOT NULL DEFAULT '[]',
                enabled INTEGER NOT NULL DEFAULT 1,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employees");

        sqlx::query(
            "CREATE TABLE agent_employee_skills (
                employee_id TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (employee_id, skill_id)
            )",
        )
        .execute(&pool)
        .await
        .expect("create agent_employee_skills");

        let manifest = json!({
            "id": "builtin-general",
            "name": "通用助手",
            "description": "通用处理能力",
            "version": "1.0.0",
            "author": "WorkClaw",
            "recommended_model": "",
            "tags": [],
            "created_at": "2026-01-01T00:00:00Z",
            "username_hint": null,
            "encrypted_verify": ""
        })
        .to_string();

        sqlx::query(
            "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type)
             VALUES ('builtin-general', ?, '2026-01-01T00:00:00Z', '', '', 'builtin')",
        )
        .bind(manifest)
        .execute(&pool)
        .await
        .expect("seed builtin-general");

        pool
    })
}

#[test]
fn employee_manage_lists_skills() {
    let pool = setup_pool();
    let tool = EmployeeManageTool::new(pool);
    let output = tool
        .execute(json!({ "action": "list_skills" }), &ToolContext::default())
        .expect("list skills");
    let payload: Value = serde_json::from_str(&output).expect("parse json");
    assert_eq!(payload["action"], "list_skills");
    assert_eq!(payload["items"][0]["id"], "builtin-general");
}

#[test]
fn employee_manage_creates_employee_and_can_list() {
    let pool = setup_pool();
    let tool = EmployeeManageTool::new(pool);
    let profile_root =
        std::env::temp_dir().join(format!("employee-manage-profile-{}", Uuid::new_v4()));
    let profile_root_text = profile_root.to_string_lossy().to_string();
    let create_output = tool
        .execute(
            json!({
                "action": "create_employee",
                "name": "项目经理",
                "persona": "推进需求交付并协调多技能执行",
                "primary_skill_id": "builtin-general",
                "skill_ids": ["builtin-general"],
                "enabled_scopes": ["app"],
                "default_work_dir": profile_root_text,
                "profile_answers": [
                    { "key": "mission", "question": "核心使命", "answer": "推进需求上线交付" },
                    { "key": "tone", "question": "沟通风格", "answer": "结论先行、简洁明确" }
                ]
            }),
            &ToolContext::default(),
        )
        .expect("create employee");
    let created: Value = serde_json::from_str(&create_output).expect("parse create output");
    assert_eq!(created["action"], "create_employee");
    assert_eq!(created["ok"], true);
    assert_eq!(created["employee"]["name"], "项目经理");
    assert!(created["employee"]["employee_id"]
        .as_str()
        .is_some_and(|v| !v.is_empty()));
    assert_eq!(created["profile"]["applied"], true);
    assert_eq!(
        created["profile"]["files"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or_default(),
        3
    );
    let has_agents = created["profile"]["files"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("AGENTS.md") && Path::new(path).exists())
        })
    });
    assert!(has_agents);

    let list_output = tool
        .execute(
            json!({ "action": "list_employees" }),
            &ToolContext::default(),
        )
        .expect("list employees");
    let listed: Value = serde_json::from_str(&list_output).expect("parse list output");
    assert_eq!(listed["action"], "list_employees");
    assert_eq!(listed["items"][0]["name"], "项目经理");
    let _ = std::fs::remove_dir_all(&profile_root);
}

#[test]
fn employee_manage_can_apply_profile_for_existing_employee() {
    let pool = setup_pool();
    let tool = EmployeeManageTool::new(pool);
    let profile_root =
        std::env::temp_dir().join(format!("employee-manage-apply-profile-{}", Uuid::new_v4()));
    let profile_root_text = profile_root.to_string_lossy().to_string();

    let create_output = tool
        .execute(
            json!({
                "action": "create_employee",
                "name": "客服专员",
                "employee_id": "service_agent",
                "primary_skill_id": "builtin-general",
                "default_work_dir": profile_root_text,
                "auto_apply_profile": false
            }),
            &ToolContext::default(),
        )
        .expect("create employee");
    let created: Value = serde_json::from_str(&create_output).expect("parse create output");
    assert_eq!(created["profile"]["applied"], false);

    let apply_output = tool
        .execute(
            json!({
                "action": "apply_profile",
                "employee_id": "service_agent",
                "profile_answers": [
                    { "key": "mission", "question": "核心使命", "answer": "保障客户问题闭环" },
                    { "key": "boundaries", "question": "边界规则", "answer": "高风险操作需二次确认" }
                ]
            }),
            &ToolContext::default(),
        )
        .expect("apply profile");
    let applied: Value = serde_json::from_str(&apply_output).expect("parse apply output");
    assert_eq!(applied["action"], "apply_profile");
    assert_eq!(applied["ok"], true);
    assert_eq!(
        applied["files"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or_default(),
        3
    );
    let has_user = applied["files"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("USER.md") && Path::new(path).exists())
        })
    });
    assert!(has_user);
    let _ = std::fs::remove_dir_all(&profile_root);
}

#[test]
fn employee_manage_auto_derives_primary_skill_when_missing() {
    let pool = setup_pool();
    let tool = EmployeeManageTool::new(pool);
    let profile_root =
        std::env::temp_dir().join(format!("employee-manage-auto-primary-{}", Uuid::new_v4()));
    let profile_root_text = profile_root.to_string_lossy().to_string();

    let create_output = tool
        .execute(
            json!({
                "action": "create_employee",
                "name": "自动主技能员工",
                "default_work_dir": profile_root_text,
                "auto_apply_profile": false
            }),
            &ToolContext::default(),
        )
        .expect("create employee");
    let created: Value = serde_json::from_str(&create_output).expect("parse create output");
    assert_eq!(created["action"], "create_employee");
    assert_eq!(created["employee"]["primary_skill_id"], "builtin-general");
    assert_eq!(
        created["employee"]["skill_ids"]
            .as_array()
            .map(|items| items.len()),
        Some(1)
    );
    assert_eq!(created["employee"]["skill_ids"][0], "builtin-general");
    let _ = std::fs::remove_dir_all(&profile_root);
}

#[test]
fn employee_manage_defaults_enabled_scopes_to_app() {
    let pool = setup_pool();
    let tool = EmployeeManageTool::new(pool);
    let profile_root =
        std::env::temp_dir().join(format!("employee-manage-default-scope-{}", Uuid::new_v4()));
    let profile_root_text = profile_root.to_string_lossy().to_string();

    let create_output = tool
        .execute(
            json!({
                "action": "create_employee",
                "name": "默认范围员工",
                "employee_id": "default_scope_employee",
                "default_work_dir": profile_root_text,
                "auto_apply_profile": false
            }),
            &ToolContext::default(),
        )
        .expect("create employee");
    let created: Value = serde_json::from_str(&create_output).expect("parse create output");
    assert_eq!(created["employee"]["enabled_scopes"], json!(["app"]));
    let _ = std::fs::remove_dir_all(&profile_root);
}

#[test]
fn employee_manage_updates_employee_with_skill_deltas() {
    let pool = setup_pool();
    let tool = EmployeeManageTool::new(pool);
    let profile_root =
        std::env::temp_dir().join(format!("employee-manage-update-{}", Uuid::new_v4()));
    let profile_root_text = profile_root.to_string_lossy().to_string();

    let create_output = tool
        .execute(
            json!({
                "action": "create_employee",
                "name": "内容运营",
                "employee_id": "content_creator",
                "persona": "负责内容生产与发布",
                "skill_ids": ["builtin-general", "docx-helper"],
                "default_work_dir": profile_root_text,
                "auto_apply_profile": false
            }),
            &ToolContext::default(),
        )
        .expect("create employee");
    let created: Value = serde_json::from_str(&create_output).expect("parse create output");
    assert_eq!(created["employee"]["employee_id"], "content_creator");

    let update_output = tool
        .execute(
            json!({
                "action": "update_employee",
                "employee_id": "content_creator",
                "name": "内容专家",
                "persona": "负责内容策略、素材管理与产出审核",
                "primary_skill_id": "docx-helper",
                "add_skill_ids": ["find-skills"],
                "remove_skill_ids": ["builtin-general"],
                "enabled": false
            }),
            &ToolContext::default(),
        )
        .expect("update employee");
    let updated: Value = serde_json::from_str(&update_output).expect("parse update output");
    assert_eq!(updated["action"], "update_employee");
    assert_eq!(updated["ok"], true);
    assert_eq!(updated["employee"]["name"], "内容专家");
    assert_eq!(
        updated["employee"]["persona"],
        "负责内容策略、素材管理与产出审核"
    );
    assert_eq!(updated["employee"]["primary_skill_id"], "docx-helper");
    assert_eq!(updated["employee"]["enabled"], false);
    assert_eq!(
        updated["employee"]["skill_ids"],
        json!(["docx-helper", "find-skills"])
    );

    let _ = std::fs::remove_dir_all(&profile_root);
}

#[test]
fn employee_manage_schema_exposes_update_employee_action() {
    let pool = setup_pool();
    let tool = EmployeeManageTool::new(pool);
    let schema = tool.input_schema();
    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum should be array");
    assert!(
        actions
            .iter()
            .any(|item| item.as_str().is_some_and(|v| v == "update_employee")),
        "update_employee should be exposed in employee_manage schema"
    );
}

#[test]
fn employee_manage_schema_hides_routing_priority() {
    let pool = setup_pool();
    let tool = EmployeeManageTool::new(pool);
    let schema = tool.input_schema();
    assert!(
        schema["properties"].get("routing_priority").is_none(),
        "routing priority should not be exposed in employee_manage schema"
    );
}
