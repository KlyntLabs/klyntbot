//! CodingTodoContextBuilder — pushes ContextUpdate when todo events fire.

use std::sync::Arc;

use bus::{ContextUpdate, ContextUpdateQueue, ContextUpdateReason, DomainEvent, UpdatePriority};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub struct CodingTodoContextBuilder {
    queue: Option<Arc<ContextUpdateQueue>>,
}

impl CodingTodoContextBuilder {
    pub fn new(queue: Option<Arc<ContextUpdateQueue>>) -> Self {
        Self { queue }
    }

    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        info!("CodingTodoContextBuilder: subscriber started");
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("CodingTodoContextBuilder: shutdown received");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::Todo(evt)) => {
                            let reason = match evt {
                                bus::domain_events::TodoEvent::PlanRatified { .. } => {
                                    ContextUpdateReason::CodingPlanRatified
                                }
                                _ => ContextUpdateReason::CodingTodoChanged,
                            };
                            if let Some(ref queue) = self.queue {
                                let summary = match &evt {
                                    bus::domain_events::TodoEvent::StateChanged { item_id, from, to, .. } => {
                                        format!("todo {}: {:?} -> {:?}", item_id, from, to)
                                    }
                                    bus::domain_events::TodoEvent::Cancelled { item_id, .. } => {
                                        format!("todo {} cancelled", item_id)
                                    }
                                    bus::domain_events::TodoEvent::PlanProposed { item_ids, .. } => {
                                        format!("plan proposed ({} items)", item_ids.len())
                                    }
                                    bus::domain_events::TodoEvent::PlanRatified { .. } => {
                                        "plan ratified".to_string()
                                    }
                                    bus::domain_events::TodoEvent::PlanCancelled { .. } => {
                                        "plan cancelled".to_string()
                                    }
                                };
                                queue.push(ContextUpdate {
                                    reason,
                                    content: Some(summary),
                                    metadata: None,
                                    priority: UpdatePriority::Normal,
                                    timestamp: jiff::Timestamp::now(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
