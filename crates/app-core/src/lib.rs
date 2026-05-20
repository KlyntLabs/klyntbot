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
pub mod runtime;
pub mod state;
pub mod tracing;
pub mod tracing_handlers;
pub mod wake_orchestrator;

pub use events::AppEventEmitter;
pub use init::EventChannels;
pub use state::{AppCore, EntityUpdate, HandlerResult};
