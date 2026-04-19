pub(crate) use crate::commands::im_host::ReplyDeliveryTrace;

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
