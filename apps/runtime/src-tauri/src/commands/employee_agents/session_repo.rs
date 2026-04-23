use sqlx::{Row, SqlitePool};

pub(crate) struct ThreadSessionRecord {
    pub session_id: String,
    pub session_exists: bool,
    pub conversation_id: String,
}

pub(crate) struct ConversationSessionRecord {
    pub session_id: String,
    pub session_exists: bool,
}

pub(crate) struct SessionSeedInput<'a> {
    pub id: &'a str,
    pub skill_id: &'a str,
    pub title: &'a str,
    pub created_at: &'a str,
    pub model_id: &'a str,
    pub work_dir: &'a str,
    pub employee_id: &'a str,
}

pub(crate) struct ThreadSessionLinkInput<'a> {
    pub thread_id: &'a str,
    pub employee_db_id: &'a str,
    pub session_id: &'a str,
    pub route_session_key: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
    pub channel: &'a str,
    pub account_id: &'a str,
    pub conversation_id: &'a str,
    pub base_conversation_id: &'a str,
    pub parent_conversation_candidates_json: &'a str,
    pub scope: &'a str,
    pub peer_kind: &'a str,
    pub peer_id: &'a str,
    pub topic_id: &'a str,
    pub sender_id: &'a str,
}

pub(crate) struct ConversationSessionLinkInput<'a> {
    pub conversation_id: &'a str,
    pub employee_db_id: &'a str,
    pub thread_id: &'a str,
    pub session_id: &'a str,
    pub route_session_key: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
    pub channel: &'a str,
    pub account_id: &'a str,
    pub base_conversation_id: &'a str,
    pub parent_conversation_candidates_json: &'a str,
    pub scope: &'a str,
    pub peer_kind: &'a str,
    pub peer_id: &'a str,
    pub topic_id: &'a str,
    pub sender_id: &'a str,
}

pub(crate) struct InboundEventLinkInput<'a> {
    pub id: &'a str,
    pub thread_id: &'a str,
    pub session_id: &'a str,
    pub employee_db_id: &'a str,
    pub im_event_id: &'a str,
    pub im_message_id: &'a str,
    pub created_at: &'a str,
}

async fn im_thread_sessions_has_conversation_column(pool: &SqlitePool) -> Result<bool, String> {
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('im_thread_sessions')")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(columns.iter().any(|name| name == "conversation_id"))
}

async fn im_conversation_sessions_exists(pool: &SqlitePool) -> Result<bool, String> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'im_conversation_sessions'",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(!tables.is_empty())
}

async fn authoritative_conversation_sessions_exist_for_thread(
    pool: &SqlitePool,
    thread_id: &str,
    employee_db_id: &str,
) -> Result<bool, String> {
    if !im_conversation_sessions_exists(pool).await? {
        return Ok(false);
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM im_conversation_sessions
         WHERE thread_id = ? AND employee_id = ?",
    )
    .bind(thread_id)
    .bind(employee_db_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(count > 0)
}

pub(crate) async fn find_conversation_session_record(
    pool: &SqlitePool,
    conversation_id: &str,
    employee_db_id: &str,
) -> Result<Option<ConversationSessionRecord>, String> {
    if !im_conversation_sessions_exists(pool).await? {
        return Ok(None);
    }

    let row = sqlx::query(
        "SELECT cs.session_id,
                CASE WHEN s.id IS NULL THEN 0 ELSE 1 END AS session_exists
         FROM im_conversation_sessions cs
         LEFT JOIN sessions s ON s.id = cs.session_id
         WHERE cs.conversation_id = ? AND cs.employee_id = ?
         LIMIT 1",
    )
    .bind(conversation_id)
    .bind(employee_db_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|record| ConversationSessionRecord {
        session_id: record
            .try_get(0)
            .expect("conversation session record session_id"),
        session_exists: record
            .try_get::<i64, _>(1)
            .expect("conversation session record session_exists")
            != 0,
    }))
}

pub(crate) async fn find_thread_session_record(
    pool: &SqlitePool,
    thread_id: &str,
    employee_db_id: &str,
) -> Result<Option<ThreadSessionRecord>, String> {
    if authoritative_conversation_sessions_exist_for_thread(pool, thread_id, employee_db_id).await?
    {
        return Ok(None);
    }

    let row = if im_thread_sessions_has_conversation_column(pool).await? {
        sqlx::query(
            "SELECT ts.session_id,
                    COALESCE(ts.conversation_id, ''),
                    CASE WHEN s.id IS NULL THEN 0 ELSE 1 END AS session_exists
             FROM im_thread_sessions ts
             LEFT JOIN sessions s ON s.id = ts.session_id
             WHERE ts.thread_id = ? AND ts.employee_id = ?
             LIMIT 1",
        )
        .bind(thread_id)
        .bind(employee_db_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query(
            "SELECT ts.session_id,
                    '' AS conversation_id,
                    CASE WHEN s.id IS NULL THEN 0 ELSE 1 END AS session_exists
             FROM im_thread_sessions ts
             LEFT JOIN sessions s ON s.id = ts.session_id
             WHERE ts.thread_id = ? AND ts.employee_id = ?
             LIMIT 1",
        )
        .bind(thread_id)
        .bind(employee_db_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(row.map(|record| ThreadSessionRecord {
        session_id: record.try_get(0).expect("thread session record session_id"),
        conversation_id: record
            .try_get(1)
            .expect("thread session record conversation_id"),
        session_exists: record
            .try_get::<i64, _>(2)
            .expect("thread session record session_exists")
            != 0,
    }))
}

pub(crate) async fn upsert_thread_session_link(
    pool: &SqlitePool,
    input: &ThreadSessionLinkInput<'_>,
) -> Result<(), String> {
    if im_thread_sessions_has_conversation_column(pool).await? {
        sqlx::query(
            "INSERT INTO im_thread_sessions (
                thread_id,
                employee_id,
                session_id,
                route_session_key,
                created_at,
                updated_at,
                channel,
                account_id,
                conversation_id,
                base_conversation_id,
                parent_conversation_candidates_json,
                scope,
                peer_kind,
                peer_id,
                topic_id,
                sender_id
             )
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(thread_id, employee_id) DO UPDATE SET
                session_id = excluded.session_id,
                route_session_key = excluded.route_session_key,
                channel = excluded.channel,
                account_id = excluded.account_id,
                conversation_id = excluded.conversation_id,
                base_conversation_id = excluded.base_conversation_id,
                parent_conversation_candidates_json = excluded.parent_conversation_candidates_json,
                scope = excluded.scope,
                peer_kind = excluded.peer_kind,
                peer_id = excluded.peer_id,
                topic_id = excluded.topic_id,
                sender_id = excluded.sender_id,
                updated_at = excluded.updated_at",
        )
        .bind(input.thread_id)
        .bind(input.employee_db_id)
        .bind(input.session_id)
        .bind(input.route_session_key)
        .bind(input.created_at)
        .bind(input.updated_at)
        .bind(input.channel)
        .bind(input.account_id)
        .bind(input.conversation_id)
        .bind(input.base_conversation_id)
        .bind(input.parent_conversation_candidates_json)
        .bind(input.scope)
        .bind(input.peer_kind)
        .bind(input.peer_id)
        .bind(input.topic_id)
        .bind(input.sender_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    } else {
        sqlx::query(
            "INSERT INTO im_thread_sessions (
                thread_id,
                employee_id,
                session_id,
                route_session_key,
                created_at,
                updated_at
             )
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(thread_id, employee_id) DO UPDATE SET
                session_id = excluded.session_id,
                route_session_key = excluded.route_session_key,
                updated_at = excluded.updated_at",
        )
        .bind(input.thread_id)
        .bind(input.employee_db_id)
        .bind(input.session_id)
        .bind(input.route_session_key)
        .bind(input.created_at)
        .bind(input.updated_at)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) async fn upsert_conversation_session_link(
    pool: &SqlitePool,
    input: &ConversationSessionLinkInput<'_>,
) -> Result<(), String> {
    if !im_conversation_sessions_exists(pool).await? {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO im_conversation_sessions (
            conversation_id,
            employee_id,
            thread_id,
            session_id,
            route_session_key,
            created_at,
            updated_at,
            channel,
            account_id,
            base_conversation_id,
            parent_conversation_candidates_json,
            scope,
            peer_kind,
            peer_id,
            topic_id,
            sender_id
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(conversation_id, employee_id) DO UPDATE SET
            thread_id = excluded.thread_id,
            session_id = excluded.session_id,
            route_session_key = excluded.route_session_key,
            updated_at = excluded.updated_at,
            channel = excluded.channel,
            account_id = excluded.account_id,
            base_conversation_id = excluded.base_conversation_id,
            parent_conversation_candidates_json = excluded.parent_conversation_candidates_json,
            scope = excluded.scope,
            peer_kind = excluded.peer_kind,
            peer_id = excluded.peer_id,
            topic_id = excluded.topic_id,
            sender_id = excluded.sender_id",
    )
    .bind(input.conversation_id)
    .bind(input.employee_db_id)
    .bind(input.thread_id)
    .bind(input.session_id)
    .bind(input.route_session_key)
    .bind(input.created_at)
    .bind(input.updated_at)
    .bind(input.channel)
    .bind(input.account_id)
    .bind(input.base_conversation_id)
    .bind(input.parent_conversation_candidates_json)
    .bind(input.scope)
    .bind(input.peer_kind)
    .bind(input.peer_id)
    .bind(input.topic_id)
    .bind(input.sender_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn insert_session_seed(
    pool: &SqlitePool,
    input: &SessionSeedInput<'_>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO sessions (id, skill_id, title, created_at, model_id, permission_mode, work_dir, employee_id)
         VALUES (?, ?, ?, ?, ?, 'standard', ?, ?)",
    )
    .bind(input.id)
    .bind(input.skill_id)
    .bind(input.title)
    .bind(input.created_at)
    .bind(input.model_id)
    .bind(input.work_dir)
    .bind(input.employee_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn update_session_employee_id(
    pool: &SqlitePool,
    session_id: &str,
    employee_id: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET employee_id = ? WHERE id = ?")
        .bind(employee_id)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn insert_inbound_event_link(
    pool: &SqlitePool,
    input: &InboundEventLinkInput<'_>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO im_message_links (id, thread_id, session_id, employee_id, direction, im_event_id, im_message_id, app_message_id, created_at)
         VALUES (?, ?, ?, ?, 'inbound', ?, ?, '', ?)",
    )
    .bind(input.id)
    .bind(input.thread_id)
    .bind(input.session_id)
    .bind(input.employee_db_id)
    .bind(input.im_event_id)
    .bind(input.im_message_id)
    .bind(input.created_at)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}
