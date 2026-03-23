const DEFAULT_SESSION_TITLE: &str = "New Chat";
const MAX_SESSION_TITLE_CHARS: usize = 28;
const GENERIC_SESSION_TITLE_INPUTS: &[&str] = &[
    "",
    "hi",
    "hello",
    "hey",
    "start",
    "continue",
    "continueprevious",
    "continuefrombefore",
    "helpme",
    "needhelp",
    "你好",
    "您好",
    "在吗",
    "继续",
    "开始",
    "帮我一下",
    "帮我处理",
    "请帮我一下",
    "继续上次",
    "继续刚才",
];

fn canonicalize_session_title_match(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn trim_title_punctuation(value: &str) -> &str {
    value.trim_matches(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                ',' | '.'
                    | ':'
                    | ';'
                    | '!'
                    | '?'
                    | '-'
                    | '，'
                    | '。'
                    | '：'
                    | '；'
                    | '！'
                    | '？'
                    | '、'
                    | '…'
                    | '·'
                    | '|'
                    | '/'
                    | '\\'
                    | '"'
                    | '\''
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
            )
    })
}

pub(crate) fn is_generic_session_title(value: &str) -> bool {
    let normalized = canonicalize_session_title_match(value);
    normalized.is_empty()
        || normalized == canonicalize_session_title_match(DEFAULT_SESSION_TITLE)
        || GENERIC_SESSION_TITLE_INPUTS
            .iter()
            .any(|candidate| normalized == canonicalize_session_title_match(candidate))
}

pub(crate) fn normalize_candidate_session_title(value: &str) -> Option<String> {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = trim_title_punctuation(&collapsed);
    if trimmed.is_empty() || is_generic_session_title(trimmed) {
        return None;
    }
    let title: String = trimmed.chars().take(MAX_SESSION_TITLE_CHARS).collect();
    let title = trim_title_punctuation(&title).trim().to_string();
    if title.is_empty() || is_generic_session_title(&title) {
        None
    } else {
        Some(title)
    }
}

pub(crate) fn derive_meaningful_session_title_from_messages<'a, I>(messages: I) -> Option<String>
where
    I: IntoIterator<Item = &'a str>,
{
    messages
        .into_iter()
        .find_map(normalize_candidate_session_title)
}

pub(crate) async fn maybe_update_session_title_from_first_user_message_with_pool(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    user_message: &str,
) -> Result<(), String> {
    let msg_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM messages WHERE session_id = ?")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    if msg_count.0 <= 1 {
        let Some(title) = normalize_candidate_session_title(user_message) else {
            return Ok(());
        };
        sqlx::query(
            "UPDATE sessions SET title = ? WHERE id = ? AND (title = 'New Chat' OR title = '')",
        )
        .bind(&title)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        derive_meaningful_session_title_from_messages, is_generic_session_title,
        normalize_candidate_session_title,
    };

    #[test]
    fn normalize_candidate_session_title_skips_generic_inputs() {
        assert_eq!(normalize_candidate_session_title("继续"), None);
        assert_eq!(normalize_candidate_session_title("  hello  "), None);
    }

    #[test]
    fn normalize_candidate_session_title_trims_punctuation_and_limits_length() {
        let title = normalize_candidate_session_title(
            "  !!! Build a much better release notes workflow for enterprise users ??? ",
        )
        .expect("title");
        assert_eq!(title, "Build a much better release");
    }

    #[test]
    fn derive_meaningful_session_title_from_messages_uses_first_non_generic_message() {
        let title = derive_meaningful_session_title_from_messages([
            "继续",
            "  hello ",
            "帮我分析这个编译错误",
        ])
        .expect("meaningful title");
        assert_eq!(title, "帮我分析这个编译错误");
        assert!(is_generic_session_title("New Chat"));
    }
}
