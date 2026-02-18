//! Goal engine crate for klyntbot.

pub mod store;
pub mod types;

pub use store::GoalStore;
pub use types::{Goal, GoalProgress, GoalStatus, Metric};
