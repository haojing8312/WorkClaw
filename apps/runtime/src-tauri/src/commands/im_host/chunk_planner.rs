use super::contract::ImReplyChunkPlan;

pub(crate) fn plan_text_chunks(text: &str, limit: usize) -> Vec<ImReplyChunkPlan> {
    if limit == 0 || text.is_empty() {
        return Vec::new();
    }

    let chars = text.chars().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < chars.len() {
        let end = (start + limit).min(chars.len());
        let chunk = chars[start..end].iter().collect::<String>();
        chunks.push(ImReplyChunkPlan {
            index: chunks.len(),
            text: chunk,
        });
        start = end;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::plan_text_chunks;

    #[test]
    fn chunk_planner_preserves_full_text() {
        let text = "你好，世界。".repeat(500);
        let chunks = plan_text_chunks(&text, 1800);
        assert!(chunks.len() > 1);
        let rebuilt = chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<String>();
        assert_eq!(rebuilt, text);
    }

    #[test]
    fn chunk_planner_keeps_short_text_in_one_chunk() {
        let text = "短消息";
        let chunks = plan_text_chunks(text, 1800);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].index, 0);
        assert_eq!(chunks[0].text, text);
    }
}
