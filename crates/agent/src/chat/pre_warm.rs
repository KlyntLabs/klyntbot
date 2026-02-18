//! Context pre-warming for faster first responses.
//!
//! Pre-loads session history and tool definitions before the user message
//! is fully classified, reducing latency for the first LLM call.

use std::sync::Arc;

use tokio::sync::RwLock;

use common::SessionKey;
use session::{SessionManager, SessionMessage};
use tools::registry::ToolRegistry;

/// Pre-loaded context ready for immediate use by an execution engine.
pub struct PreWarmHandle {
    /// Session messages loaded from persistent storage.
    pub session_messages: Vec<SessionMessage>,
    /// Tool definitions pre-serialized as JSON schemas.
    pub tool_definitions: Vec<serde_json::Value>,
}

/// Pre-warm the context by loading session history and tool definitions concurrently.
///
/// This runs before classification completes, so the execution engine can start
/// immediately once the strategy is known.
pub async fn pre_warm(
    session_key: &SessionKey,
    session_manager: Arc<tokio::sync::Mutex<SessionManager>>,
    tool_registry: Arc<RwLock<ToolRegistry>>,
) -> PreWarmHandle {
    // Load session and tools concurrently
    let (session_messages, tool_definitions) = tokio::join!(
        load_session(session_key, session_manager),
        load_tool_definitions(tool_registry),
    );

    PreWarmHandle {
        session_messages,
        tool_definitions,
    }
}

async fn load_session(
    session_key: &SessionKey,
    session_manager: Arc<tokio::sync::Mutex<SessionManager>>,
) -> Vec<SessionMessage> {
    let mut mgr = session_manager.lock().await;
    match mgr.get_or_create(session_key.as_str()).await {
        Ok(session) => session.messages.clone(),
        Err(e) => {
            tracing::warn!(
                session_key = %session_key,
                error = %e,
                "pre-warm: failed to load session, starting fresh"
            );
            Vec::new()
        }
    }
}

async fn load_tool_definitions(tool_registry: Arc<RwLock<ToolRegistry>>) -> Vec<serde_json::Value> {
    let registry = tool_registry.read().await;
    registry.get_definitions()
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{ChannelName, ChatId};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_pre_warm_empty_session() {
        let dir = tempdir().unwrap();
        let session_mgr = Arc::new(tokio::sync::Mutex::new(
            SessionManager::new(dir.path()).await,
        ));
        let tool_reg = Arc::new(RwLock::new(ToolRegistry::new()));

        let handle = pre_warm(
            &SessionKey::new(&ChannelName::new("test"), &ChatId::new("123")),
            session_mgr,
            tool_reg,
        )
        .await;

        assert!(handle.session_messages.is_empty());
        assert!(handle.tool_definitions.is_empty());
    }

    #[tokio::test]
    async fn test_pre_warm_loads_tools() {
        use async_trait::async_trait;
        use serde_json::Value;
        use tools::{RoutingContext, Tool};

        struct DummyTool;

        #[async_trait]
        impl Tool for DummyTool {
            fn name(&self) -> &str {
                "dummy"
            }
            fn description(&self) -> &str {
                "A dummy tool"
            }
            fn parameters(&self) -> Value {
                serde_json::json!({"type": "object", "properties": {}})
            }
            async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> common::Result<String> {
                Ok("ok".to_string())
            }
        }

        let dir = tempdir().unwrap();
        let session_mgr = Arc::new(tokio::sync::Mutex::new(
            SessionManager::new(dir.path()).await,
        ));

        let mut registry = ToolRegistry::new();
        registry.register(DummyTool);
        let tool_reg = Arc::new(RwLock::new(registry));

        let handle = pre_warm(
            &SessionKey::new(&ChannelName::new("test"), &ChatId::new("456")),
            session_mgr,
            tool_reg,
        )
        .await;

        assert_eq!(handle.tool_definitions.len(), 1);
        let tool_def = &handle.tool_definitions[0];
        assert_eq!(tool_def["function"]["name"], "dummy");
    }
}
