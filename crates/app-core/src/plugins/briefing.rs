use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use tracing::info;

/// Plugin that spawns the morning briefing: surfacing unsurfaced cross-domain
/// insights shortly after startup.
pub struct BriefingPlugin;

#[async_trait]
impl AppCorePlugin for BriefingPlugin {
    fn name(&self) -> &str {
        "briefing"
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let pool = ctx.deps.storage_pool.clone();
        let bus = ctx
            .deps
            .domain_event_bus
            .as_ref()
            .ok_or_else(|| common::KlyntbotError::Storage("no domain event bus".into()))?
            .clone();
        let token = ctx.deps.shutdown_token.clone();

        ctx.spawn_background(async move {
            // Small delay to let BrainVoice finish subscribing.
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if token.is_cancelled() {
                return;
            }

            let svc = feature_insights::nightly_batch::NightlyBatchService::new(pool);
            match svc.get_unsurfaced_insights().await {
                Ok(insights) if !insights.is_empty() => {
                    for insight in &insights {
                        bus.publish(bus::DomainEvent::CrossDomainDotReady {
                            source_kind: "insight".into(),
                            source_id: insight.id.to_string(),
                            source_title: "Cross-domain insight".into(),
                            target_kind: "briefing".into(),
                            target_id: insight.date.clone(),
                            target_title: insight.date.clone(),
                            confidence: 1.0,
                            tooltip: insight.insight_text.clone(),
                            detail_route: None,
                        });
                        if let Err(e) = svc.mark_surfaced(insight.id).await {
                            tracing::warn!("failed to mark insight {} surfaced: {e}", insight.id);
                        }
                    }
                    info!(
                        count = insights.len(),
                        "morning briefing: surfaced cross-domain insights"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::debug!("morning briefing insight check failed: {e}");
                }
            }
        });

        Ok(())
    }
}
