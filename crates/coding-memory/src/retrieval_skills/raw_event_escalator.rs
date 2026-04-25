//! `RawEventEscalator` — bypasses summaries; surfaces raw `ingest_event_log` rows.

use super::*;
use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;

/// Closure returning the provenance event ids attached to current top-k.
pub type ProvenanceIdsFn = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

/// Closure looking up raw ingest events by id.
pub type EventLookupFn = Arc<
    dyn Fn(Vec<String>) -> Pin<Box<dyn std::future::Future<Output = common::Result<Vec<serde_json::Value>>> + Send>>
        + Send
        + Sync,
>;

/// Skill.
pub struct RawEventEscalator {
    provenance: ProvenanceIdsFn,
    lookup: EventLookupFn,
}

impl std::fmt::Debug for RawEventEscalator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawEventEscalator").finish()
    }
}

impl RawEventEscalator {
    /// Construct.
    #[must_use]
    pub fn new(provenance: ProvenanceIdsFn, lookup: EventLookupFn) -> Self {
        Self { provenance, lookup }
    }
}

#[async_trait]
impl RetrievalSkill for RawEventEscalator {
    fn name(&self) -> &'static str { "raw_event_escalator" }
    fn description(&self) -> &'static str { "Surface raw ingest events for top-k provenance." }
    fn tier(&self) -> BudgetTier { BudgetTier::Ultra }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let ids = (self.provenance)();
        if ids.is_empty() {
            return Ok(EscalationOutcome {
                succeeded: false,
                coverage_after: ctx.coverage_score,
                added_context: String::new(),
                added_ids: vec![],
            });
        }
        let events = (self.lookup)(ids).await?;
        let mut buf = String::from("# Raw event payload\n\n");
        for e in &events {
            buf.push_str(&serde_json::to_string_pretty(e).unwrap_or_default());
            buf.push_str("\n\n");
        }
        Ok(EscalationOutcome {
            succeeded: !events.is_empty(),
            coverage_after: ctx.coverage_score + 0.2,
            added_context: buf,
            added_ids: vec![],
        })
    }
}
