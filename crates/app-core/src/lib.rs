#![recursion_limit = "256"]

pub mod adapters;
pub mod brain_voice;
pub mod desktop_approval_channel;
pub mod errors;
pub mod events;
pub mod focus;
pub mod handlers;
pub mod infrastructure;
pub mod init;
pub mod journey;
pub mod plugin;
pub mod plugins;
pub mod runtime;
pub mod state;
pub mod tracing;
pub mod tracing_handlers;
pub mod wake_orchestrator;

pub use events::AppEventEmitter;
pub use init::event_channels::EventChannels;
pub use state::{AppCore, EntityUpdate, HandlerResult};
