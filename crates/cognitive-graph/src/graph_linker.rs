//! Graph linker service (KCA Track 2). The trait is implemented in the agent crate
//! using `DynProvider`; this module only declares the contract and a heuristic
//! fallback for tests / non-LLM environments.

use async_trait::async_trait;

use crate::graph_linker_types::{GraphLinkInput, GraphLinkOutput};

#[async_trait]
pub trait GraphLinkHandler: Send + Sync {
    /// Returns operations to apply to the graph. Errors are non-fatal: callers should
    /// log and continue (this is best-effort enrichment).
    async fn link(&self, input: GraphLinkInput) -> common::Result<GraphLinkOutput>;
}

/// Heuristic implementation that produces an empty output. Used as a fallback when
/// no LLM provider is configured for cognitive, or when the gate decides to skip.
pub struct NoopGraphLinkHandler;

#[async_trait]
impl GraphLinkHandler for NoopGraphLinkHandler {
    async fn link(&self, _input: GraphLinkInput) -> common::Result<GraphLinkOutput> {
        Ok(GraphLinkOutput::default())
    }
}
