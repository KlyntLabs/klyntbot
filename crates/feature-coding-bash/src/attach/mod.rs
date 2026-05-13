//! Phase 2.3c: PTY attach support — token issuance + WebSocket bridge.

pub mod bridge;
pub mod token;

pub use bridge::{ControlFrame, PtyAttachBridge};
pub use token::{generate_attach_token, tokens_eq_constant_time};
