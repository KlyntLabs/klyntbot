//! Shared application state injected into every Axum handler.

use std::sync::{Arc, RwLock};

use agent::AgentLoop;
use scheduling::CronService;

/// Shared state available to all dashboard handlers.
///
/// Uses `Arc<AgentLoop>` (not `Arc<Mutex<AgentLoop>>`) — the two-phase
/// construction refactor (Task #8) makes this safe by separating the
/// inbound receiver from the loop itself before Arc wrapping.
#[derive(Clone)]
pub struct AppState {
    pub repos: storage::Repos,
    pub agent_loop: Arc<AgentLoop>,
    pub cron_service: Arc<CronService>,
    pub config: Arc<RwLock<config::Config>>,
    pub started_at: std::time::Instant,
}
