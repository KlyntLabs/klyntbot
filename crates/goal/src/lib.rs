//! Goal engine crate for klyntbot.

pub mod conversions;
pub mod error;
pub mod types;

pub use error::GoalError;
pub use types::{Goal, GoalProgress, GoalStatus, Metric};
