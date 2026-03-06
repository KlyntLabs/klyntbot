//! Test mocks for integration tests.
//!
//! Centralizes all mock implementations to avoid compiling them
//! as separate (empty) test binaries.

pub mod conversation_embedding;
pub mod embedding;
pub mod embedding_utils;
pub mod learning;
pub mod provider;

// Re-exports for convenience (not all test binaries use every mock)
#[allow(unused_imports)]
pub use conversation_embedding::MockConversationEmbeddingHandler;
#[allow(unused_imports)]
pub use embedding::MockEmbeddingHandler;
#[allow(unused_imports)]
pub use learning::MockLearningHandler;
#[allow(unused_imports)]
pub use provider::{ErrorProvider, MockProvider};
