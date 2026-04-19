use crate::commands::feishu_gateway::{
    send_feishu_text_message_with_pool,
};
use crate::commands::im_host::{
    build_im_ask_user_request_text, prepare_channel_interactive_session_thread_with_pool,
};
use crate::commands::openclaw_plugins::im_host_contract::ImReplyLifecyclePhase;
use sqlx::SqlitePool;

pub(crate) fn build_feishu_ask_user_request_text(question: &str, options: &[String]) -> String {
    build_im_ask_user_request_text(question, options)
}

pub async fn notify_feishu_ask_user_requested_with_pool(
    pool: &SqlitePool,
    session_id: &str,
    question: &str,
    options: &[String],
    sidecar_base_url: Option<String>,
) -> Result<(), String> {
    let Some(thread_id) = prepare_channel_interactive_session_thread_with_pool(
        pool,
        "feishu",
        session_id,
        Some("ask_user"),
        ImReplyLifecyclePhase::AskUserRequested,
    )
    .await?
    else {
        return Ok(());
    };
    send_feishu_text_message_with_pool(
        pool,
        &thread_id,
        &build_feishu_ask_user_request_text(question, options),
        sidecar_base_url,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build_feishu_ask_user_request_text;

    #[test]
    fn build_feishu_ask_user_request_text_includes_options() {
        let text = build_feishu_ask_user_request_text(
            "请选择方案",
            &["方案A".to_string(), "方案B".to_string()],
        );

        assert!(text.contains("请选择方案"));
        assert!(text.contains("可选项：方案A / 方案B"));
        assert!(text.contains("请直接回复你的选择或补充信息。"));
    }

    #[test]
    fn build_feishu_ask_user_request_text_skips_empty_options() {
        let text =
            build_feishu_ask_user_request_text("请补充背景", &["".to_string(), "  ".to_string()]);

        assert_eq!(text, "请补充背景\n请直接回复你的选择或补充信息。");
    }
}
