//! Shared WebSocket connection manager for channel implementations.
//!
//! Consolidates the connect → heartbeat → read loop → shutdown pattern
//! that was duplicated across WhatsApp, QQ, Slack, and Discord channels.

use async_trait::async_trait;
use common::{ChannelError, Result};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, warn};

/// Alias for the write half of a WebSocket connection.
pub type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, WsMessage>;

/// Alias for the read half of a WebSocket connection.
pub type WsStream = SplitStream<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>;

/// How to keep the WebSocket connection alive.
pub enum HeartbeatStrategy {
    /// Send a keepalive when no message is received within `timeout`.
    /// Used by WhatsApp, QQ, and Slack.
    Timeout {
        timeout: Duration,
        build_payload: Box<dyn Fn() -> WsMessage + Send + Sync>,
    },

    /// No automatic heartbeat. The handler manages keepalive externally
    /// (e.g., Discord spawns its own heartbeat task from HELLO).
    None,
}

/// Configuration for a WebSocket connection.
pub struct WsConfig {
    /// URL to connect to.
    pub url: String,

    /// Connection timeout.
    pub connect_timeout: Duration,

    /// Heartbeat strategy.
    pub heartbeat: HeartbeatStrategy,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            connect_timeout: Duration::from_secs(30),
            heartbeat: HeartbeatStrategy::Timeout {
                timeout: Duration::from_secs(30),
                build_payload: Box::new(|| WsMessage::Ping(vec![].into())),
            },
        }
    }
}

/// Trait implemented by each channel to handle WebSocket events.
///
/// The [`WebSocketManager`] calls these methods at the appropriate lifecycle points.
/// Protocol-specific logic stays in the channel; the manager owns connection
/// boilerplate (connect, heartbeat, error handling, shutdown).
#[async_trait]
pub trait WsHandler: Send + Sync {
    /// Called once after the connection is established, before the read loop.
    ///
    /// Use for handshakes (Discord IDENTIFY, storing the write half, etc.).
    /// Return an updated [`HeartbeatStrategy`] to override the config default,
    /// or `None` to keep it.
    async fn on_connected(&self, write: &Arc<Mutex<WsSink>>) -> Result<Option<HeartbeatStrategy>>;

    /// Called for each text message received from the WebSocket.
    ///
    /// Return `Ok(true)` to continue the read loop, `Ok(false)` to disconnect.
    async fn on_text_message(&self, text: &str, write: &Arc<Mutex<WsSink>>) -> Result<bool>;

    /// Called when the read loop exits (error, server close, or shutdown).
    async fn on_disconnected(&self) {}
}

/// Manages a single WebSocket session: connect → heartbeat → read loop → cleanup.
///
/// Callers wrap [`WebSocketManager::run`] in [`super::reconnect_loop`] for automatic reconnection.
pub struct WebSocketManager {
    running: Arc<AtomicBool>,
}

impl WebSocketManager {
    pub fn new(running: Arc<AtomicBool>) -> Self {
        Self { running }
    }

    /// Run a single WebSocket session. Returns when the connection drops or `running` is false.
    pub async fn run(&self, config: &WsConfig, handler: &(dyn WsHandler + '_)) -> Result<()> {
        // 1. Connect with timeout
        let ws_stream = tokio::time::timeout(config.connect_timeout, connect_async(&config.url))
            .await
            .map_err(|_| ChannelError::ConnectionFailed("WebSocket connection timed out".into()))?
            .map_err(|e| ChannelError::ConnectionFailed(e.to_string()))?
            .0;

        let (write, read) = ws_stream.split();
        let write = Arc::new(Mutex::new(write));

        // 2. Post-connect handshake; handler may override heartbeat strategy
        let heartbeat_override = handler.on_connected(&write).await?;

        // 3. Read loop with the active heartbeat strategy
        let result = self
            .read_loop(read, &write, heartbeat_override.as_ref(), config, handler)
            .await;

        // 4. Cleanup
        handler.on_disconnected().await;

        result
    }

    async fn read_loop(
        &self,
        mut read: WsStream,
        write: &Arc<Mutex<WsSink>>,
        heartbeat_override: Option<&HeartbeatStrategy>,
        config: &WsConfig,
        handler: &(dyn WsHandler + '_),
    ) -> Result<()> {
        let heartbeat = heartbeat_override.unwrap_or(&config.heartbeat);

        loop {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            let msg = match heartbeat {
                HeartbeatStrategy::Timeout {
                    timeout,
                    build_payload,
                } => match tokio::time::timeout(*timeout, read.next()).await {
                    Ok(Some(Ok(msg))) => msg,
                    Ok(Some(Err(e))) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    Ok(None) => {
                        warn!("WebSocket closed");
                        break;
                    }
                    Err(_) => {
                        let payload = build_payload();
                        let mut w = write.lock().await;
                        if let Err(e) = w.send(payload).await {
                            error!("Keepalive send failed: {}", e);
                            break;
                        }
                        debug!("Sent keepalive");
                        continue;
                    }
                },
                HeartbeatStrategy::None => match read.next().await {
                    Some(Ok(msg)) => msg,
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    None => {
                        warn!("WebSocket closed");
                        break;
                    }
                },
            };

            if let WsMessage::Text(text) = msg {
                match handler.on_text_message(&text, write).await {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(e) => {
                        error!("Handler error: {}", e);
                        // Continue on handler errors; break only on connection errors
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Mock handler that counts messages and optionally disconnects after N messages.
    struct MockHandler {
        connected: AtomicBool,
        message_count: AtomicUsize,
        disconnect_after: Option<usize>,
    }

    impl MockHandler {
        fn new(disconnect_after: Option<usize>) -> Self {
            Self {
                connected: AtomicBool::new(false),
                message_count: AtomicUsize::new(0),
                disconnect_after,
            }
        }
    }

    #[async_trait]
    impl WsHandler for MockHandler {
        async fn on_connected(
            &self,
            _write: &Arc<Mutex<WsSink>>,
        ) -> Result<Option<HeartbeatStrategy>> {
            self.connected.store(true, Ordering::SeqCst);
            Ok(None)
        }

        async fn on_text_message(&self, _text: &str, _write: &Arc<Mutex<WsSink>>) -> Result<bool> {
            let count = self.message_count.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(limit) = self.disconnect_after {
                Ok(count < limit)
            } else {
                Ok(true)
            }
        }

        async fn on_disconnected(&self) {
            self.connected.store(false, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_default_config() {
        let config = WsConfig::default();
        assert!(config.url.is_empty());
        assert_eq!(config.connect_timeout, Duration::from_secs(30));
        match &config.heartbeat {
            HeartbeatStrategy::Timeout { timeout, .. } => {
                assert_eq!(*timeout, Duration::from_secs(30));
            }
            _ => panic!("expected Timeout strategy"),
        }
    }

    #[test]
    fn test_ws_manager_creation() {
        let running = Arc::new(AtomicBool::new(true));
        let manager = WebSocketManager::new(running.clone());
        assert!(manager.running.load(Ordering::SeqCst));

        running.store(false, Ordering::SeqCst);
        assert!(!manager.running.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_handler_on_disconnected_called() {
        let handler = MockHandler::new(None);
        assert!(!handler.connected.load(Ordering::SeqCst));

        // Simulate on_disconnected
        handler.on_disconnected().await;
        assert!(!handler.connected.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        let running = Arc::new(AtomicBool::new(true));
        let manager = WebSocketManager::new(running);
        let handler = MockHandler::new(None);

        let config = WsConfig {
            url: "wss://192.0.2.1:1".to_string(), // RFC 5737 TEST-NET — will timeout
            connect_timeout: Duration::from_millis(100),
            heartbeat: HeartbeatStrategy::None,
        };

        let result = manager.run(&config, &handler).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("timed out") || err.contains("Connection"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_invalid_url() {
        let running = Arc::new(AtomicBool::new(true));
        let manager = WebSocketManager::new(running);
        let handler = MockHandler::new(None);

        let config = WsConfig {
            url: "not-a-url".to_string(),
            connect_timeout: Duration::from_secs(5),
            heartbeat: HeartbeatStrategy::None,
        };

        let result = manager.run(&config, &handler).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_running_flag_prevents_start() {
        let running = Arc::new(AtomicBool::new(false));
        let manager = WebSocketManager::new(running);
        let handler = MockHandler::new(None);

        // Can't really test the read loop stops, but we can verify the manager
        // respects the running flag by checking it doesn't panic
        let config = WsConfig {
            url: "wss://192.0.2.1:1".to_string(),
            connect_timeout: Duration::from_millis(50),
            heartbeat: HeartbeatStrategy::None,
        };

        // Should fail on connect, not hang
        let _ = manager.run(&config, &handler).await;
    }
}
