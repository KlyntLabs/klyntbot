//! Klyntbot Session - Conversation session persistence
//!
//! This crate provides session management and JSONL persistence.

pub mod manager;

pub use manager::{Session, SessionInfo, SessionManager, SessionMessage};
