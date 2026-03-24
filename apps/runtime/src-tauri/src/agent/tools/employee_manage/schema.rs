use serde_json::{json, Value};

pub(crate) fn input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["list_skills", "list_employees", "create_employee", "update_employee", "apply_profile"],
                "description": "执行动作"
            },
            "id": { "type": "string" },
            "employee_db_id": { "type": "string" },
            "employee_id": { "type": "string" },
            "name": { "type": "string" },
            "persona": { "type": "string" },
            "primary_skill_id": { "type": "string" },
            "skill_ids": {
                "type": "array",
                "items": { "type": "string" }
            },
            "add_skill_ids": {
                "type": "array",
                "items": { "type": "string" }
            },
            "remove_skill_ids": {
                "type": "array",
                "items": { "type": "string" }
            },
            "enabled_scopes": {
                "type": "array",
                "items": { "type": "string" }
            },
            "enabled": { "type": "boolean" },
            "is_default": { "type": "boolean" },
            "auto_apply_profile": { "type": "boolean" },
            "default_work_dir": { "type": "string" },
            "feishu_open_id": { "type": "string" },
            "feishu_app_id": { "type": "string" },
            "feishu_app_secret": { "type": "string" },
            "profile_answers": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "key": { "type": "string" },
                        "question": { "type": "string" },
                        "answer": { "type": "string" }
                    },
                    "required": ["key", "answer"]
                }
            }
        },
        "required": ["action"]
    })
}
