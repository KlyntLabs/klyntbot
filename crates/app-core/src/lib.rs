pub mod adapters;
pub mod brain_voice;
pub mod coding_memory;
pub mod errors;
pub mod events;
pub mod focus;
pub mod handlers;
pub mod infrastructure;
pub mod init;
pub mod journey;
pub mod state;
pub mod wake_orchestrator;

pub use events::AppEventEmitter;
pub use init::EventChannels;
pub use state::{AppCore, EntityUpdate, HandlerResult};
