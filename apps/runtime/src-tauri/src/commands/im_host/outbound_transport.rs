use super::{ImReplyDeliveryPlan, ImReplyDeliveryState, ReplyDeliveryTrace};
use async_trait::async_trait;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct ImReplyPlanTransportResult<T> {
    pub trace: ReplyDeliveryTrace,
    pub deliveries: Vec<T>,
}

#[async_trait]
pub(crate) trait ImReplyPlanTransport {
    type Delivery: Send;

    async fn on_processing_started(&self, plan: &ImReplyDeliveryPlan) -> Result<(), String>;

    async fn send_chunk(
        &self,
        plan: &ImReplyDeliveryPlan,
        chunk_index: usize,
        text: &str,
    ) -> Result<Self::Delivery, String>;

    async fn handle_delivery(
        &self,
        _plan: &ImReplyDeliveryPlan,
        _chunk_index: usize,
        _delivery: &Self::Delivery,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn on_processing_finished(
        &self,
        plan: &ImReplyDeliveryPlan,
        final_state: &str,
    ) -> Result<(), String>;

    fn classify_failure(&self, delivered_count: usize, _error: &str) -> ImReplyDeliveryState {
        if delivered_count == 0 {
            ImReplyDeliveryState::Failed
        } else {
            ImReplyDeliveryState::FailedPartial
        }
    }
}

pub(crate) async fn execute_reply_plan_with_transport<T>(
    transport: &T,
    plan: &ImReplyDeliveryPlan,
) -> Result<ImReplyPlanTransportResult<T::Delivery>, String>
where
    T: ImReplyPlanTransport + Sync,
{
    let mut trace = ReplyDeliveryTrace::new(
        plan.logical_reply_id.clone(),
        plan.session_id.clone(),
        plan.channel.clone(),
        plan.thread_id.clone(),
        plan.chunks.len(),
    );
    let mut deliveries = Vec::with_capacity(plan.chunks.len());

    let _ = transport.on_processing_started(plan).await;

    for chunk in &plan.chunks {
        match transport.send_chunk(plan, chunk.index, &chunk.text).await {
            Ok(delivery) => {
                trace.mark_chunk_delivered(chunk.index);
                transport
                    .handle_delivery(plan, chunk.index, &delivery)
                    .await?;
                deliveries.push(delivery);
            }
            Err(error) => {
                trace.mark_chunk_failed(chunk.index);
                let _ = transport.on_processing_finished(plan, "failed").await;
                trace.finish(transport.classify_failure(deliveries.len(), &error));
                return Err(format!(
                    "{}\ntrace={}",
                    error,
                    serde_json::to_string(&trace)
                        .unwrap_or_else(|_| "{\"error\":\"trace_serialize_failed\"}".to_string())
                ));
            }
        }
    }

    let _ = transport.on_processing_finished(plan, "completed").await;
    trace.finish(ImReplyDeliveryState::Completed);
    Ok(ImReplyPlanTransportResult { trace, deliveries })
}
