pub(crate) use crate::commands::im_host::plan_text_chunks as plan_feishu_text_chunks;

#[cfg(test)]
mod tests {
    use super::plan_feishu_text_chunks;

    #[test]
    fn feishu_chunk_planner_preserves_full_text() {
        let text = "你好，世界。".repeat(500);
        let chunks = plan_feishu_text_chunks(&text, 1800);
        assert!(chunks.len() > 1);
        let rebuilt = chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<String>();
        assert_eq!(rebuilt, text);
    }

    #[test]
    fn feishu_chunk_planner_keeps_short_text_in_one_chunk() {
        let text = "短消息";
        let chunks = plan_feishu_text_chunks(text, 1800);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].index, 0);
        assert_eq!(chunks[0].text, text);
    }
}
