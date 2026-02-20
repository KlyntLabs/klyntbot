//! Klyntbot Session - Conversation session persistence
//!
//! This crate provides session management backed by SQL (via `storage::SessionRepo`).

pub mod manager;

pub use manager::{Session, SessionInfo, SessionManager, SessionMessage};
