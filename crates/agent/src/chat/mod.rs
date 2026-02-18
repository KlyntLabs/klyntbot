//! Chat-first adaptations for rich conversational AI.
//!
//! - `formatter` — channel-aware response formatting

pub mod formatter;

pub use formatter::format_for_channel;
