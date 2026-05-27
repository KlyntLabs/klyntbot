use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

/// Plugin wrapper for the BrainVoice signal router.
pub struct BrainVoicePlugin;

#[async_trait]
impl AppCorePlugin for BrainVoicePlugin {
    fn name(&self) -> &str {
        "brain-voice"
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let journey_tracker = crate::journey::JourneyTracker::new(ctx.deps.storage_pool.clone());
        ctx.insert_handle(Arc::new(journey_tracker.clone()));

        let feedback_repo =
            ::storage::repos::BrainSignalFeedbackRepo::new(ctx.deps.storage_pool.inner().clone());
        let emitter_for_brain: Arc<dyn crate::events::AppEventEmitter> = ctx
            .deps
            .event_emitter
            .clone()
            .unwrap_or_else(|| Arc::new(crate::events::NoopEmitter));

        let rx = ctx
            .deps
            .domain_event_bus
            .as_ref()
            .expect("domain event bus initialized above")
            .subscribe();
        let bv = crate::brain_voice::BrainVoice::start(
            rx,
            feedback_repo,
            emitter_for_brain,
            crate::brain_voice::BrainVoiceConfig::default(),
            Some(journey_tracker),
        );
        tracing::info!("BrainVoice signal router started");
        ctx.insert_handle(Arc::new(bv));

        Ok(())
    }
}
