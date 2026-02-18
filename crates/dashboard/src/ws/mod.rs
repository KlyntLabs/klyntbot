//! WebSocket upgrade handler for the dashboard server.
//!
//! Exports the chat WebSocket handler and any shared WS utilities.

pub mod chat;

pub use chat::ws_chat_handler;
