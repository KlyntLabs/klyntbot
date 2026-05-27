use crate::journey::{JourneyTracker, Milestone};
use desktop_shared::events::{MessageSegment, TransparencyData};
use std::sync::Arc;
use storage::Repos;

/// Performs the terminal side-effects of a Done turn: persist the
/// segments+transparency metadata, publish `ChatTurnCompleted`, and advance the
/// FirstChatResponse journey milestone. Error/Cancelled have no finalization.
pub struct TurnFinalizer<'a> {
    pub repos: Option<&'a Repos>,
    pub domain_event_bus: Option<&'a Arc<bus::DomainEventBus>>,
    pub journey_tracker: Option<&'a JourneyTracker>,
}

impl TurnFinalizer<'_> {
    pub async fn finalize_done(
        &self,
        session_key: &str,
        user_message: Option<&str>,
        message_id: Option<&str>,
        segments: &[MessageSegment],
        transparency: &TransparencyData,
    ) {
        // 1. Persist segments + transparency to the assistant message metadata.
        if let Some(repos) = self.repos {
            let mut meta = serde_json::Map::new();
            if !segments.is_empty() {
                meta.insert(
                    "segments".to_string(),
                    serde_json::to_value(segments).unwrap_or_default(),
                );
            }
            meta.insert(
                "transparency".to_string(),
                serde_json::to_value(transparency).unwrap_or_default(),
            );
            let meta_value = serde_json::Value::Object(meta);

            let persist_outcome = if let Some(mid) = message_id {
                repos
                    .sessions
                    .update_assistant_metadata_by_id(mid, None, Some(&meta_value))
                    .await
            } else {
                repos
                    .sessions
                    .update_last_assistant_metadata(session_key, None, Some(&meta_value))
                    .await
            };
            if let Err(e) = &persist_outcome {
                tracing::warn!("metadata persist sync failed for {session_key}: {e}");
            }
            if matches!(persist_outcome, Ok(false)) {
                let repos_clone = repos.clone();
                let sk_owned = session_key.to_string();
                let meta_clone = meta_value.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    match repos_clone
                        .sessions
                        .update_last_assistant_metadata(&sk_owned, None, Some(&meta_clone))
                        .await
                    {
                        Ok(true) => {}
                        Ok(false) => tracing::warn!("metadata persist retry: no row {sk_owned}"),
                        Err(e) => tracing::warn!("metadata persist retry failed {sk_owned}: {e}"),
                    }
                });
            }
        }

        // 2. Publish ChatTurnCompleted AFTER the response is saved.
        if let Some(bus) = self.domain_event_bus {
            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                session_key: session_key.to_string(),
                user_message: user_message.map(String::from),
            });
        }

        // 3. FirstChatResponse journey milestone.
        if let Some(tracker) = self.journey_tracker {
            if !tracker.is_complete(Milestone::FirstChatResponse).await {
                tracker.mark_complete(Milestone::FirstChatResponse).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn finalize_done_publishes_chat_turn_completed() {
        let bus = Arc::new(bus::DomainEventBus::new(64));
        let mut rx = bus.subscribe();

        let finalizer = TurnFinalizer {
            repos: None,
            domain_event_bus: Some(&bus),
            journey_tracker: None,
        };

        finalizer
            .finalize_done("sess-1", Some("hi there"), None, &[], &Default::default())
            .await;

        // ChatTurnCompleted reached the bus with the right session + user message.
        let evt = rx.try_recv().expect("expected a published domain event");
        match evt {
            bus::DomainEvent::ChatTurnCompleted {
                session_key,
                user_message,
            } => {
                assert_eq!(session_key, "sess-1");
                assert_eq!(user_message.as_deref(), Some("hi there"));
            }
            other => panic!("expected ChatTurnCompleted, got {:?}", other),
        }
    }
}
