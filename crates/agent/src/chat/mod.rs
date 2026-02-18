//! Chat-first adaptations for rich conversational AI.
//!
//! - `progress` — typing indicators and step progress reporting
//! - `interrupt` — interrupt handling with queue/cancel/merge policies
//! - `formatter` — channel-aware response formatting
//! - `pre_warm` — context pre-warming for faster first responses

pub mod formatter;
pub mod interrupt;
pub mod pre_warm;
pub mod progress;

pub use formatter::format_for_channel;
pub use interrupt::{InterruptHandler, InterruptPolicy};
pub use pre_warm::{pre_warm, PreWarmHandle};
pub use progress::ProgressReporter;
