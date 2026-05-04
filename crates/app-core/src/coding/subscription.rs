use crate::state::ThreadSubscription;
use crate::AppCore;
use common::Result;

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

        Ok(sub_id)
    }

    /// Unsubscribe from thread events.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_unsubscribe(&self, subscription_id: &str) -> Result<()> {
        self.thread_subscriptions.remove(subscription_id);
        Ok(())
    }
}
