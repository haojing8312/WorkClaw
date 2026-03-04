mod helpers;

use chrono::Utc;
use helpers::*;
use runtime_lib::agent::skill_config::SkillConfig;
use uuid::Uuid;

// ============================================================
// 测试 1: 导入本地 Skill 并验证可读回
// ============================================================

#[tokio::test]
async fn test_import_local_skill_and_read() {
    let (pool, _tmp_db) = setup_test_db().await;
    let (_tmp_skill, skill_dir) = create_test_skill_dir();

    // 读取 SKILL.md 并解析
    let skill_md_path = skill_dir.join("SKILL.md");
    let content = std::fs::read_to_string(&skill_md_path).unwrap();
    let config = SkillConfig::parse(&content);

    // 验证 frontmatter 解析正确
    assert_eq!(config.name.as_deref(), Some("test-skill"));
    assert_eq!(
        config.description.as_deref(),
        Some("A test skill for E2E testing")
    );
    assert_eq!(
        config.allowed_tools,
        Some(vec!["ReadFile".to_string(), "Glob".to_string()])
    );
    assert!(config.user_invocable);
    assert!(config
        .system_prompt
        .contains("You are a helpful test assistant."));

    // 构造 manifest（模拟 import_local_skill 逻辑）
    let name = config.name.clone().unwrap_or_default();
    let skill_id = format!("local-{}", name);
    let manifest_json = serde_json::json!({
        "id": skill_id,
        "name": name,
        "description": config.description.unwrap_or_default(),
        "version": "local",
        "author": "",
        "recommended_model": config.model.unwrap_or_default(),
        "tags": [],
        "created_at": Utc::now().to_rfc3339(),
        "username_hint": null,
        "encrypted_verify": "",
    });

    let now = Utc::now().to_rfc3339();
    let dir_str = skill_dir.to_string_lossy().to_string();

    // 插入到数据库
    sqlx::query(
        "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type) VALUES (?, ?, ?, ?, ?, 'local')"
    )
    .bind(&skill_id)
    .bind(manifest_json.to_string())
    .bind(&now)
    .bind("")
    .bind(&dir_str)
    .execute(&pool)
    .await
    .unwrap();

    // 验证可读回
    let (read_manifest, read_path, read_source): (String, String, String) = sqlx::query_as(
        "SELECT manifest, pack_path, source_type FROM installed_skills WHERE id = ?",
    )
    .bind(&skill_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(read_source, "local");
    assert_eq!(read_path, dir_str);

    let parsed: serde_json::Value = serde_json::from_str(&read_manifest).unwrap();
    assert_eq!(parsed["name"].as_str().unwrap(), "test-skill");
    assert_eq!(
        parsed["description"].as_str().unwrap(),
        "A test skill for E2E testing"
    );
    assert_eq!(parsed["version"].as_str().unwrap(), "local");

    // 验证 Skill 在列表中
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT manifest FROM installed_skills ORDER BY installed_at DESC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
}

// ============================================================
// 测试 2: 会话全生命周期
// ============================================================

#[tokio::test]
async fn test_session_lifecycle() {
    let (pool, _tmp_db) = setup_test_db().await;

    // 先插入一个 Skill
    let skill_id = "test-skill-001";
    let manifest_json = serde_json::json!({
        "id": skill_id,
        "name": "lifecycle-test",
        "description": "For lifecycle testing",
        "version": "1.0.0",
        "author": "tester",
        "recommended_model": "",
        "tags": [],
        "created_at": Utc::now().to_rfc3339(),
        "username_hint": null,
        "encrypted_verify": "",
    });

    sqlx::query(
        "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(skill_id)
    .bind(manifest_json.to_string())
    .bind(Utc::now().to_rfc3339())
    .bind("test-user")
    .bind("")
    .execute(&pool)
    .await
    .unwrap();

    // 创建会话
    let session_id = Uuid::new_v4().to_string();
    let model_id = "model-test-001";
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(skill_id)
    .bind("测试会话")
    .bind(&now)
    .bind(model_id)
    .execute(&pool)
    .await
    .unwrap();

    // 插入 3 条消息
    for i in 0..3 {
        let msg_id = Uuid::new_v4().to_string();
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        let content = format!("消息 #{}", i);
        let msg_time = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&msg_id)
        .bind(&session_id)
        .bind(role)
        .bind(&content)
        .bind(&msg_time)
        .execute(&pool)
        .await
        .unwrap();
    }

    // 验证消息计数
    let (msg_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM messages WHERE session_id = ?")
        .bind(&session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(msg_count, 3);

    // 验证会话存在
    let (title,): (String,) = sqlx::query_as("SELECT title FROM sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title, "测试会话");

    // 删除会话（先删消息，再删会话 — 模拟 delete_session 逻辑）
    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(&session_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&session_id)
        .execute(&pool)
        .await
        .unwrap();

    // 验证清理完成
    let (remaining_msgs,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM messages WHERE session_id = ?")
            .bind(&session_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining_msgs, 0);

    let remaining_sessions = sqlx::query_as::<_, (String,)>("SELECT id FROM sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert!(remaining_sessions.is_none());
}

// ============================================================
// 测试 3: 搜索会话
// ============================================================

#[tokio::test]
async fn test_search_sessions() {
    let (pool, _tmp_db) = setup_test_db().await;

    let skill_id = "search-test-skill";

    // 创建两个会话，不同标题
    let session_a = Uuid::new_v4().to_string();
    let session_b = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&session_a)
    .bind(skill_id)
    .bind("Rust 编程讨论")
    .bind(&now)
    .bind("model-1")
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&session_b)
    .bind(skill_id)
    .bind("Python 数据分析")
    .bind(&now)
    .bind("model-1")
    .execute(&pool)
    .await
    .unwrap();

    // 搜索 "Rust" — 应只匹配 session_a
    let pattern = "%Rust%";
    let results = sqlx::query_as::<_, (String, String)>(
        "SELECT DISTINCT s.id, s.title
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
         WHERE s.skill_id = ? AND (s.title LIKE ? OR m.content LIKE ?)
         ORDER BY s.created_at DESC",
    )
    .bind(skill_id)
    .bind(pattern)
    .bind(pattern)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, session_a);
    assert_eq!(results[0].1, "Rust 编程讨论");

    // 搜索 "Python" — 应只匹配 session_b
    let pattern_py = "%Python%";
    let results_py = sqlx::query_as::<_, (String, String)>(
        "SELECT DISTINCT s.id, s.title
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
         WHERE s.skill_id = ? AND (s.title LIKE ? OR m.content LIKE ?)
         ORDER BY s.created_at DESC",
    )
    .bind(skill_id)
    .bind(pattern_py)
    .bind(pattern_py)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(results_py.len(), 1);
    assert_eq!(results_py[0].0, session_b);

    // 搜索消息内容 — 先给 session_a 添加消息
    let msg_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&msg_id)
    .bind(&session_a)
    .bind("user")
    .bind("请教一下 Tokio 异步运行时的用法")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 搜索 "Tokio" — 应通过消息内容匹配到 session_a
    let pattern_tokio = "%Tokio%";
    let results_tokio = sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT s.id
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
         WHERE s.skill_id = ? AND (s.title LIKE ? OR m.content LIKE ?)",
    )
    .bind(skill_id)
    .bind(pattern_tokio)
    .bind(pattern_tokio)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(results_tokio.len(), 1);
    assert_eq!(results_tokio[0].0, session_a);
}

// ============================================================
// 测试 4: MCP 服务器 CRUD
// ============================================================

#[tokio::test]
async fn test_mcp_server_crud() {
    let (pool, _tmp_db) = setup_test_db().await;

    let server_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let server_name = "test-mcp-server";

    // 添加 MCP 服务器
    sqlx::query(
        "INSERT INTO mcp_servers (id, name, command, args, env, enabled, created_at) VALUES (?, ?, ?, ?, ?, 1, ?)"
    )
    .bind(&server_id)
    .bind(server_name)
    .bind("npx")
    .bind(r#"["-y","@test/mcp-server"]"#)
    .bind(r#"{"API_KEY":"test-key"}"#)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 列表查询 — 验证能找到
    let rows = sqlx::query_as::<_, (String, String, String, String, String, i32)>(
        "SELECT id, name, command, args, env, enabled FROM mcp_servers ORDER BY created_at DESC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, server_id);
    assert_eq!(rows[0].1, "test-mcp-server");
    assert_eq!(rows[0].2, "npx");
    assert_eq!(rows[0].5, 1); // enabled

    // 验证 args 和 env 的 JSON 可正确解析
    let args: Vec<String> = serde_json::from_str(&rows[0].3).unwrap();
    assert_eq!(args, vec!["-y", "@test/mcp-server"]);
    let env: std::collections::HashMap<String, String> = serde_json::from_str(&rows[0].4).unwrap();
    assert_eq!(env.get("API_KEY").unwrap(), "test-key");

    // 删除 MCP 服务器
    sqlx::query("DELETE FROM mcp_servers WHERE id = ?")
        .bind(&server_id)
        .execute(&pool)
        .await
        .unwrap();

    // 验证清理完成
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mcp_servers")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    // 验证 UNIQUE 约束 — 同名服务器不能插入两次
    let id1 = Uuid::new_v4().to_string();
    let id2 = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO mcp_servers (id, name, command, args, env, enabled, created_at) VALUES (?, ?, ?, ?, ?, 1, ?)"
    )
    .bind(&id1)
    .bind("unique-server")
    .bind("cmd1")
    .bind("[]")
    .bind("{}")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let dup_result = sqlx::query(
        "INSERT INTO mcp_servers (id, name, command, args, env, enabled, created_at) VALUES (?, ?, ?, ?, ?, 1, ?)"
    )
    .bind(&id2)
    .bind("unique-server") // 同名
    .bind("cmd2")
    .bind("[]")
    .bind("{}")
    .bind(&now)
    .execute(&pool)
    .await;

    assert!(dup_result.is_err(), "同名 MCP 服务器应当被 UNIQUE 约束拒绝");
}

// ============================================================
// 测试 5: SKILL.md MCP 服务器依赖解析
// ============================================================

#[tokio::test]
async fn test_skill_config_mcp_dependency() {
    let content = r#"---
name: test-mcp-skill
description: Test MCP dependency
mcp-servers:
  - name: brave-search
    command: npx
    args: ["@anthropic/mcp-server-brave-search"]
    env: ["BRAVE_API_KEY"]
  - name: memory
---
Test skill with MCP dependencies."#;

    let config = runtime_lib::agent::skill_config::SkillConfig::parse(content);
    assert_eq!(config.mcp_servers.len(), 2);
    assert_eq!(config.mcp_servers[0].name, "brave-search");
    assert_eq!(
        config.mcp_servers[0].env,
        Some(vec!["BRAVE_API_KEY".to_string()])
    );
    assert_eq!(config.mcp_servers[1].name, "memory");
    assert_eq!(config.mcp_servers[1].command, None);
}

// ============================================================
// 测试 6: 完整 Claude Code 格式 SKILL.md 解析（含参数替换）
// ============================================================

#[tokio::test]
async fn test_skill_config_claude_code_compat() {
    let (pool, _tmp_db) = setup_test_db().await;

    // 创建完整 Claude Code 格式的 SKILL.md
    let full_skill_md = "\
---
name: code-review
description: 审查代码并提供改进建议
allowed_tools:
  - ReadFile
  - Glob
  - Grep
model: claude-sonnet-4-20250514
max_iterations: 15
argument-hint: <file_path> [focus_area]
disable-model-invocation: false
user-invocable: true
context: fork
agent: Explore
---
你是一个代码审查专家。

请审查以下文件: $ARGUMENTS[0]
关注领域: $ARGUMENTS[1]
所有参数: $ARGUMENTS
会话 ID: ${CLAUDE_SESSION_ID}
简写: $0 和 $1
";

    // 解析 frontmatter
    let config = SkillConfig::parse(full_skill_md);

    // 验证所有字段
    assert_eq!(config.name.as_deref(), Some("code-review"));
    assert_eq!(
        config.description.as_deref(),
        Some("审查代码并提供改进建议")
    );
    assert_eq!(
        config.allowed_tools,
        Some(vec![
            "ReadFile".to_string(),
            "Glob".to_string(),
            "Grep".to_string()
        ])
    );
    assert_eq!(config.model.as_deref(), Some("claude-sonnet-4-20250514"));
    assert_eq!(config.max_iterations, Some(15));
    assert_eq!(
        config.argument_hint.as_deref(),
        Some("<file_path> [focus_area]")
    );
    assert!(!config.disable_model_invocation);
    assert!(config.user_invocable);
    assert_eq!(config.context.as_deref(), Some("fork"));
    assert_eq!(config.agent.as_deref(), Some("Explore"));

    // 验证 system prompt 不含 frontmatter
    assert!(config.system_prompt.contains("你是一个代码审查专家。"));
    assert!(!config.system_prompt.contains("---"));

    // 测试参数替换
    let mut config_with_args = config.clone();
    config_with_args.substitute_arguments(&["main.rs", "error-handling"], "sess-e2e-001");

    assert!(config_with_args
        .system_prompt
        .contains("请审查以下文件: main.rs"));
    assert!(config_with_args
        .system_prompt
        .contains("关注领域: error-handling"));
    assert!(config_with_args
        .system_prompt
        .contains("所有参数: main.rs error-handling"));
    assert!(config_with_args
        .system_prompt
        .contains("会话 ID: sess-e2e-001"));
    assert!(config_with_args
        .system_prompt
        .contains("简写: main.rs 和 error-handling"));

    // 将解析后的 Skill 存入数据库并验证往返一致性
    let skill_id = "local-code-review";
    let manifest_json = serde_json::json!({
        "id": skill_id,
        "name": config.name.as_deref().unwrap_or(""),
        "description": config.description.as_deref().unwrap_or(""),
        "version": "local",
        "author": "",
        "recommended_model": config.model.as_deref().unwrap_or(""),
        "tags": [],
        "created_at": Utc::now().to_rfc3339(),
        "username_hint": null,
        "encrypted_verify": "",
    });

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO installed_skills (id, manifest, installed_at, username, pack_path, source_type) VALUES (?, ?, ?, ?, ?, 'local')"
    )
    .bind(skill_id)
    .bind(manifest_json.to_string())
    .bind(&now)
    .bind("")
    .bind("/fake/path/code-review")
    .execute(&pool)
    .await
    .unwrap();

    // 读回并验证
    let (read_manifest,): (String,) =
        sqlx::query_as("SELECT manifest FROM installed_skills WHERE id = ?")
            .bind(skill_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&read_manifest).unwrap();
    assert_eq!(parsed["name"].as_str().unwrap(), "code-review");
    assert_eq!(
        parsed["description"].as_str().unwrap(),
        "审查代码并提供改进建议"
    );
    assert_eq!(
        parsed["recommended_model"].as_str().unwrap(),
        "claude-sonnet-4-20250514"
    );
    assert_eq!(parsed["version"].as_str().unwrap(), "local");
}
