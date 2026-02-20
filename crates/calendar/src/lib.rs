pub mod caldav;
pub mod error;
pub mod provider;
pub mod providers;
pub mod state;
pub mod sync_engine;
pub mod types;

pub use error::CalendarError;

pub use caldav::{generate_vevent, parse_vevent, CalDavAuth, CalDavClient};
pub use provider::CalendarProvider;
pub use providers::{AppleCalendarProvider, GenericCalDavProvider, GoogleCalendarProvider};
pub use state::{load_provider_sync_state, save_provider_sync_state};
pub use sync_engine::{detect_conflict, resolve_conflict};
pub use types::{CalendarEvent, ConflictResolutionStrategy, EventSource, SyncState};
