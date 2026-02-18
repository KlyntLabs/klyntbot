//! Klyntbot Web Dashboard — public library API.
//!
//! The dashboard crate embeds in `klyntbot serve` and exposes:
//! - A GraphQL API via axum + async-graphql
//! - WebSocket endpoints for real-time subscriptions and chat
//! - Observable store wrappers for event-driven data changes
//! - A config file watcher for hot-reload

pub mod config_watcher;
pub mod events;
pub mod graphql;
pub mod server;
pub mod ws;

// Public re-exports for convenience.
pub use config_watcher::ConfigWatcher;
pub use events::{ChangeAction, DashboardEvent, DashboardEventBus};
pub use events::store_events::{ObservableProjectStore, ObservableTodoStore};
pub use server::{DashboardConfig, DashboardServer};
