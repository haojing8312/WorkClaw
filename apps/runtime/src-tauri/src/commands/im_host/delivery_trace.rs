use super::contract::ImReplyDeliveryState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ReplyDeliveryTrace {
    pub logical_reply_id: String,
    pub session_id: String,
    pub channel: String,
    pub target_thread_id: String,
    pub planned_chunk_count: usize,
    pub delivered_chunk_count: usize,
    pub failed_chunk_indexes: Vec<usize>,
    pub final_state: Option<ImReplyDeliveryState>,
}

impl ReplyDeliveryTrace {
    pub fn new(
        logical_reply_id: impl Into<String>,
        session_id: impl Into<String>,
        channel: impl Into<String>,
        target_thread_id: impl Into<String>,
        planned_chunk_count: usize,
    ) -> Self {
        Self {
            logical_reply_id: logical_reply_id.into(),
            session_id: session_id.into(),
            channel: channel.into(),
            target_thread_id: target_thread_id.into(),
            planned_chunk_count,
            delivered_chunk_count: 0,
            failed_chunk_indexes: Vec::new(),
            final_state: None,
        }
    }

    pub fn mark_chunk_delivered(&mut self, index: usize) {
        let next_count = index.saturating_add(1);
        if self.delivered_chunk_count < next_count {
            self.delivered_chunk_count = next_count;
        }
    }

    pub fn mark_chunk_failed(&mut self, index: usize) {
        if !self.failed_chunk_indexes.contains(&index) {
            self.failed_chunk_indexes.push(index);
        }
    }

    pub fn finish(&mut self, state: ImReplyDeliveryState) {
        self.final_state = Some(state);
    }
}

#[cfg(test)]
mod tests {
    use super::ReplyDeliveryTrace;
    use crate::commands::im_host::ImReplyDeliveryState;

    #[test]
    fn delivery_trace_tracks_partial_failure() {
        let mut trace = ReplyDeliveryTrace::new("reply-1", "session-1", "feishu", "chat-1", 3);
        trace.mark_chunk_delivered(0);
        trace.mark_chunk_failed(1);
        trace.finish(ImReplyDeliveryState::FailedPartial);

        assert_eq!(trace.delivered_chunk_count, 1);
        assert_eq!(trace.failed_chunk_indexes, vec![1]);
        assert_eq!(trace.final_state, Some(ImReplyDeliveryState::FailedPartial));
    }
}
