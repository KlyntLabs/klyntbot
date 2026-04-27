//! `CausalEdgeDetector` — three deterministic rules. Stub until Tasks 16-19.

use crate::causal::CausalEdgeRepo;
use cognitive::EpisodicMemoryRepo;
use std::sync::Arc;

/// Detector handle.
#[derive(Debug)]
pub struct CausalEdgeDetector {
    pub(crate) edges: Arc<CausalEdgeRepo>,
    pub(crate) episodes: Arc<EpisodicMemoryRepo>,
}

impl CausalEdgeDetector {
    /// Construct.
    #[must_use]
    pub fn new(edges: Arc<CausalEdgeRepo>, episodes: Arc<EpisodicMemoryRepo>) -> Self {
        Self { edges, episodes }
    }

    /// Run all three detection rules for one session. Returns count of edges inserted.
    pub async fn detect_for_session(&self, _session_id: &str) -> common::Result<u32> {
        Ok(0)
    }
}
