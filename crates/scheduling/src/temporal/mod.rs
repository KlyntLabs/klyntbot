//! Unified temporal scheduler: persistent, wall-clock-anchored, VALARM-style rules.
pub mod fire_store;
pub mod misfire;
pub mod rules;

pub use fire_store::{FireSpec, FireStore};
pub use misfire::{Decision, MisfirePolicy};
