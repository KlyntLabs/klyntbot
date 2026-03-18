//! Scope resolution for insight generation.
//!
//! Resolves a `ScopeConfig` into a list of related note IDs that will be
//! fed into the insight context. Four scope types are supported:
//! - Backlinks: wikilink references (current default)
//! - Semantic: LanceDB embedding similarity
//! - Project: all notes in the same notebook
//! - Manual: user-selected note IDs

use async_trait::async_trait;

use crate::types::ScopeConfig;

/// Resolves note IDs from a scope configuration.
///
/// Defined here in `feature-insights` (L4), implemented in `app-core` (L7)
/// where `NoteRepo` and `VectorStore` are available. Injected into
/// `InsightService` as `Arc<dyn ScopeResolver>`.
#[async_trait]
pub trait ScopeResolver: Send + Sync {
    /// Resolve the scope config into a list of related note IDs.
    /// The returned IDs should NOT include `note_id` itself.
    async fn resolve(&self, note_id: &str, config: &ScopeConfig) -> Vec<String>;
}

/// No-op resolver for testing — returns empty scope.
pub struct NoopScopeResolver;

#[async_trait]
impl ScopeResolver for NoopScopeResolver {
    async fn resolve(&self, _note_id: &str, _config: &ScopeConfig) -> Vec<String> {
        Vec::new()
    }
}

/// Test helper: returns a fixed set of IDs.
#[cfg(test)]
pub struct FixedScopeResolver(pub Vec<String>);

#[cfg(test)]
#[async_trait]
impl ScopeResolver for FixedScopeResolver {
    async fn resolve(&self, _note_id: &str, _config: &ScopeConfig) -> Vec<String> {
        self.0.clone()
    }
}
