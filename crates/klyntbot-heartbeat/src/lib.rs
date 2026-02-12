//! Klyntbot Heartbeat - Periodic agent wake-up service
//!
//! This crate provides heartbeat service for periodic agent activation.

pub mod service;

pub use service::{
    HeartbeatCallback, HeartbeatService, DEFAULT_HEARTBEAT_INTERVAL_S, HEARTBEAT_OK_TOKEN,
    HEARTBEAT_PROMPT,
};
