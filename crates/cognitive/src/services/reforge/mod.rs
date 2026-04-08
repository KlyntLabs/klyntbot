pub mod collector;
pub mod service;
pub mod skill_files;
pub mod types;

use async_trait::async_trait;

/// Trait for LLM-backed Reforge operations. Implemented in the agent crate.
#[async_trait]
pub trait ReforgeHandler: Send + Sync {
    /// Phase 2: Knowledge synthesis from sessions + episodics.
    async fn synthesize(
        &self,
        input: &types::SynthesizeInput,
    ) -> common::Result<types::SynthesizeOutput>;

    /// Phase 3: Skill & behavior review from corrections + routing.
    async fn review(&self, input: &types::ReviewInput) -> common::Result<types::ReviewOutput>;

    /// Phase 4: Generate human-readable narrative.
    async fn narrate(&self, input: &types::NarrateInput) -> common::Result<String>;
}
