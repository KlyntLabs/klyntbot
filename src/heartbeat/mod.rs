//! Heartbeat service module.

pub mod service;

pub use service::{
    HeartbeatService, DEFAULT_HEARTBEAT_INTERVAL_S, HEARTBEAT_OK_TOKEN, HEARTBEAT_PROMPT,
};
