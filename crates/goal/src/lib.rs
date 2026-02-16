//! Goal engine crate for klyntbot.

pub mod store;
pub mod suggestion;
pub mod types;

pub use store::GoalStore;
pub use suggestion::{GoalSuggestion, GoalSuggestionEngine};
pub use types::{Goal, GoalProgress, GoalStatus, Metric};
