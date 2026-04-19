//! L4 notifications crate — quiet-hours-aware, multi-channel dispatcher
//! subscribing to `DomainEvent::AlarmFired` from the `TemporalScheduler`.

pub mod channel;
pub mod dispatcher;
pub mod error;
pub mod held;
pub mod migrations;
pub mod quiet_hours;
pub mod retry;

pub use dispatcher::{NotificationDispatcher, NotificationDispatcherHandle};
pub use error::{NotificationError, Result};
pub use migrations::migration;
