use crate::state::ThreadSubscription;
use crate::AppCore;
use common::Result;
use desktop_shared::coding::ThreadEvent;

impl AppCore {
    /// Subscribe to thread events. Returns a subscription_id that the frontend
    /// uses to listen on the `agent:thread_event#<sub_id>` Tauri event channel.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_subscribe(&self, thread_id: &str) -> Result<String> {
        let sub_id = uuid::Uuid::new_v4().to_string();
        self.thread_subscriptions.insert(
            sub_id.clone(),
            ThreadSubscription {
                thread_id: thread_id.to_string(),
                created_at: jiff::Timestamp::now().as_millisecond(),
            },
        );

        // Spawn adapter task: TypedBroker<ThreadEvent> → Tauri event
        // This is a no-op in non-Tauri contexts (dev server).
        // The actual Tauri emission is handled by the desktop adapter in main.rs.
        let mut rx = self.thread_events.subscribe();
        let target_thread_id = thread_id.to_string();
        let sid = sub_id.clone();
        let subs = self.thread_subscriptions.clone();

        tokio::spawn(async move {
            while let Ok(evt) = rx.recv().await {
                // Check if subscription still exists
                if !subs.contains_key(&sid) {
                    break;
                }
                let evt_thread_id = match &evt {
                    ThreadEvent::TurnStarted { thread_id, .. }
                    | ThreadEvent::ItemStarted { thread_id, .. }
                    | ThreadEvent::ItemDelta { thread_id, .. }
                    | ThreadEvent::ItemCompleted { thread_id, .. }
                    | ThreadEvent::ToolCallStarted { thread_id, .. }
                    | ThreadEvent::ToolCallCompleted { thread_id, .. }
                    | ThreadEvent::FileChanged { thread_id, .. }
                    | ThreadEvent::CommandExecuted { thread_id, .. }
                    | ThreadEvent::ContextCompressed { thread_id, .. }
                    | ThreadEvent::TurnCompleted { thread_id, .. } => thread_id.as_str(),
                    ThreadEvent::Heartbeat { .. } => &target_thread_id,
                };
                if evt_thread_id == target_thread_id {
                    // In Tauri context, the desktop adapter handles emission.
                    // Here we just ensure the event flows through the broker.
                    // The adapter task in main.rs will pick it up.
                    let _ = &evt; // prevent unused warning
                }
            }
        });

        Ok(sub_id)
    }

    /// Unsubscribe from thread events.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_unsubscribe(&self, subscription_id: &str) -> Result<()> {
        self.thread_subscriptions.remove(subscription_id);
        Ok(())
    }
}
