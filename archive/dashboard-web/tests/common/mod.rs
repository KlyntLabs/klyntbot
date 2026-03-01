//! Shared test infrastructure for dashboard integration tests.
//!
//! Provides `build_test_app()` which wires together an in-memory storage pool,
//! a mock agent loop, and the full Axum router — matching production wiring
//! without requiring any external services.

// TODO: implement

// pub async fn build_test_app() -> axum::Router {
//     let pool = storage::StoragePool::connect_in_memory().await.unwrap();
//     let repos = storage::Repos::from_pool(&pool);
//     // Construct mock AgentLoop via test builder (MockProvider)
//     // Build AppState
//     // Return dashboard::router::build(state)
// }
