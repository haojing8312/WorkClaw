use super::send_feishu_text_message_with_pool;
use super::types::FeishuPairingRequestRecord;
use crate::commands::openclaw_plugins::get_openclaw_plugin_feishu_channel_snapshot_with_pool;
use crate::im::types::ImEvent;
use sqlx::SqlitePool;
use uuid::Uuid;

pub(crate) fn resolve_feishu_pairing_account_id(
    event: &ImEvent,
    snapshot: Option<&crate::commands::openclaw_plugins::OpenClawPluginChannelSnapshotResult>,
) -> String {
    if let Some(account_id) = snapshot
        .and_then(|value| super::select_feishu_channel_account_snapshot(value, event))
        .map(|account| account.account_id.trim())
        .filter(|value| !value.is_empty())
    {
        return account_id.to_string();
    }

    event
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default")
        .to_string()
}

pub(crate) fn generate_feishu_pairing_code() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn normalize_explicit_feishu_pairing_code(code: Option<&str>) -> Option<String> {
    let normalized = code
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| !value.eq_ignore_ascii_case("PAIRING"))?;
    Some(normalized.to_ascii_uppercase())
}

pub(crate) async fn upsert_feishu_pairing_request_with_pool(
    pool: &SqlitePool,
    account_id: &str,
    sender_id: &str,
    chat_id: &str,
    explicit_code: Option<&str>,
) -> Result<(FeishuPairingRequestRecord, bool), String> {
    let normalized_account_id = account_id.trim();
    let normalized_sender_id = sender_id.trim();
    let normalized_chat_id = chat_id.trim();
    let normalized_explicit_code = normalize_explicit_feishu_pairing_code(explicit_code);
    if normalized_account_id.is_empty() || normalized_sender_id.is_empty() {
        return Err("pairing request requires account_id and sender_id".to_string());
    }

    if let Some(existing) = sqlx::query_as::<_, FeishuPairingRequestRecord>(
        "SELECT id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
         FROM feishu_pairing_requests
         WHERE channel = 'feishu' AND account_id = ? AND sender_id = ? AND status = 'pending'
         LIMIT 1",
    )
    .bind(normalized_account_id)
    .bind(normalized_sender_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    {
        let now = chrono::Utc::now().to_rfc3339();
        let next_chat_id = if normalized_chat_id.is_empty() {
            existing.chat_id.clone()
        } else {
            normalized_chat_id.to_string()
        };
        let next_code = normalized_explicit_code
            .clone()
            .unwrap_or_else(|| existing.code.clone());
        sqlx::query(
            "UPDATE feishu_pairing_requests
             SET chat_id = ?, code = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&next_chat_id)
        .bind(&next_code)
        .bind(&now)
        .bind(&existing.id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        return Ok((
            FeishuPairingRequestRecord {
                chat_id: next_chat_id,
                code: next_code,
                updated_at: now,
                ..existing
            },
            false,
        ));
    }

    let id = Uuid::new_v4().to_string();
    let code = normalized_explicit_code.unwrap_or_else(generate_feishu_pairing_code);
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO feishu_pairing_requests (
            id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
        ) VALUES (?, 'feishu', ?, ?, ?, ?, 'pending', ?, ?, NULL, '')",
    )
    .bind(&id)
    .bind(normalized_account_id)
    .bind(normalized_sender_id)
    .bind(normalized_chat_id)
    .bind(&code)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok((
        FeishuPairingRequestRecord {
            id,
            channel: "feishu".to_string(),
            account_id: normalized_account_id.to_string(),
            sender_id: normalized_sender_id.to_string(),
            chat_id: normalized_chat_id.to_string(),
            code,
            status: "pending".to_string(),
            created_at: now.clone(),
            updated_at: now,
            resolved_at: None,
            resolved_by_user: String::new(),
        },
        true,
    ))
}

pub(crate) async fn list_feishu_pairing_allow_from_with_pool(
    pool: &SqlitePool,
    account_id: &str,
) -> Result<Vec<String>, String> {
    let normalized_account_id = account_id.trim();
    if normalized_account_id.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT sender_id
         FROM feishu_pairing_allow_from
         WHERE channel = 'feishu' AND account_id = ?
         ORDER BY approved_at DESC",
    )
    .bind(normalized_account_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(|(sender_id,)| sender_id).collect())
}

fn build_feishu_pairing_request_text(record: &FeishuPairingRequestRecord) -> String {
    format!(
        "已收到你的配对申请。\n配对码：{code}\n发送者：{sender}\n请在 WorkClaw 桌面端审核通过后继续私聊本机器人。",
        code = record.code,
        sender = record.sender_id
    )
}

fn build_feishu_pairing_resolution_text(record: &FeishuPairingRequestRecord) -> String {
    match record.status.as_str() {
        "approved" => format!(
            "配对已通过。你现在可以直接私聊本机器人。\n配对码：{code}",
            code = record.code
        ),
        "denied" => format!(
            "配对申请未通过。\n配对码：{code}\n如需继续使用，请联系管理员后重新发起配对。",
            code = record.code
        ),
        _ => format!("配对请求状态已更新：{}", record.status),
    }
}

pub(crate) async fn maybe_create_feishu_pairing_request_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<Option<FeishuPairingRequestRecord>, String> {
    if !super::is_direct_feishu_chat(event) {
        return Ok(None);
    }
    let snapshot =
        get_openclaw_plugin_feishu_channel_snapshot_with_pool(pool, "openclaw-lark")
            .await
            .ok();
    let account_id = resolve_feishu_pairing_account_id(event, snapshot.as_ref());
    let Some(sender_id) = event
        .sender_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let (record, created) =
        upsert_feishu_pairing_request_with_pool(pool, &account_id, sender_id, &event.thread_id, None)
            .await?;
    if created && !record.chat_id.trim().is_empty() {
        let _ = send_feishu_text_message_with_pool(
            pool,
            &record.chat_id,
            &build_feishu_pairing_request_text(&record),
            None,
        )
        .await;
    }
    Ok(Some(record))
}

pub(crate) async fn list_feishu_pairing_requests_with_pool(
    pool: &SqlitePool,
    status: Option<String>,
) -> Result<Vec<FeishuPairingRequestRecord>, String> {
    let normalized_status = status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let records = if let Some(status) = normalized_status {
        sqlx::query_as::<_, FeishuPairingRequestRecord>(
            "SELECT id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
             FROM feishu_pairing_requests
             WHERE channel = 'feishu' AND status = ?
             ORDER BY updated_at DESC, created_at DESC",
        )
        .bind(status)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as::<_, FeishuPairingRequestRecord>(
            "SELECT id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
             FROM feishu_pairing_requests
             WHERE channel = 'feishu'
             ORDER BY updated_at DESC, created_at DESC",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(records)
}

pub(crate) async fn resolve_feishu_pairing_request_with_pool(
    pool: &SqlitePool,
    request_id: &str,
    status: &str,
    resolved_by_user: Option<String>,
) -> Result<FeishuPairingRequestRecord, String> {
    let normalized_request_id = request_id.trim();
    if normalized_request_id.is_empty() {
        return Err("request_id is required".to_string());
    }
    if status != "approved" && status != "denied" {
        return Err("status must be approved or denied".to_string());
    }

    let mut record = sqlx::query_as::<_, FeishuPairingRequestRecord>(
        "SELECT id, channel, account_id, sender_id, chat_id, code, status, created_at, updated_at, resolved_at, resolved_by_user
         FROM feishu_pairing_requests
         WHERE id = ?
         LIMIT 1",
    )
    .bind(normalized_request_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("pairing request not found: {normalized_request_id}"))?;

    if record.status != "pending" {
        return Ok(record);
    }

    let now = chrono::Utc::now().to_rfc3339();
    let resolved_by_user = resolved_by_user.unwrap_or_default().trim().to_string();
    sqlx::query(
        "UPDATE feishu_pairing_requests
         SET status = ?, updated_at = ?, resolved_at = ?, resolved_by_user = ?
         WHERE id = ?",
    )
    .bind(status)
    .bind(&now)
    .bind(&now)
    .bind(&resolved_by_user)
    .bind(normalized_request_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    if status == "approved" {
        sqlx::query(
            "INSERT INTO feishu_pairing_allow_from (
                channel, account_id, sender_id, source_request_id, approved_at, approved_by_user
            ) VALUES ('feishu', ?, ?, ?, ?, ?)
            ON CONFLICT(channel, account_id, sender_id) DO UPDATE SET
                source_request_id = excluded.source_request_id,
                approved_at = excluded.approved_at,
                approved_by_user = excluded.approved_by_user",
        )
        .bind(&record.account_id)
        .bind(&record.sender_id)
        .bind(&record.id)
        .bind(&now)
        .bind(&resolved_by_user)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    record.status = status.to_string();
    record.updated_at = now.clone();
    record.resolved_at = Some(now);
    record.resolved_by_user = resolved_by_user;

    if !record.chat_id.trim().is_empty() {
        let _ = send_feishu_text_message_with_pool(
            pool,
            &record.chat_id,
            &build_feishu_pairing_resolution_text(&record),
            None,
        )
        .await;
    }

    Ok(record)
}
