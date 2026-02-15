// CalendarTool - Calendar operations tool with dependency injection

use async_trait::async_trait;
use common::{Result, ToolError};
use serde_json::{json, Value};
use std::sync::Arc;

use super::{RoutingContext, Tool};
use crate::params::ParamExtractor;

/// CalendarHandler trait for dependency inversion
/// Implemented by CalendarSyncAdapter in agent crate (Layer 5)
/// Defined here in tools crate (Layer 3) to break circular dependency
#[async_trait]
pub trait CalendarHandler: Send + Sync {
    /// Synchronize calendar with CalDAV server
    async fn sync_calendar(&self) -> Result<Value>;

    /// List upcoming calendar events
    async fn list_events(&self, limit: usize) -> Result<Value>;

    /// Create a new calendar event
    async fn create_event(
        &self,
        summary: String,
        description: Option<String>,
        start: String, // ISO 8601 format
        end: String,   // ISO 8601 format
    ) -> Result<Value>;

    /// Get sync status
    async fn get_status(&self) -> Result<Value>;
}

/// CalendarTool - Tool interface for calendar operations
pub struct CalendarTool {
    handler: Arc<dyn CalendarHandler>,
}

impl CalendarTool {
    pub fn new(handler: Arc<dyn CalendarHandler>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl Tool for CalendarTool {
    fn name(&self) -> &str {
        "calendar"
    }

    fn description(&self) -> &str {
        "Manage calendar events and sync with CalDAV. Actions: sync, list_events, create_event, status."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["sync", "list_events", "create_event", "status"],
                    "description": "Action to perform"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max events to list (for list_events, default: 10)"
                },
                "summary": {
                    "type": "string",
                    "description": "Event title (for create_event)"
                },
                "description": {
                    "type": "string",
                    "description": "Event description (for create_event)"
                },
                "start": {
                    "type": "string",
                    "description": "Start time in RFC3339 format with timezone offset (e.g. '2026-02-17T21:00:00+07:00'). Always include the timezone offset. (for create_event)"
                },
                "end": {
                    "type": "string",
                    "description": "End time in RFC3339 format with timezone offset (e.g. '2026-02-17T22:00:00+07:00'). Always include the timezone offset. (for create_event)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "sync" => {
                let result = self.handler.sync_calendar().await?;
                Ok(serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|_| "Calendar sync completed".to_string()))
            }
            "list_events" => {
                let limit = p.optional_u64("limit")?.unwrap_or(10) as usize;
                let result = self.handler.list_events(limit).await?;
                Ok(serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|_| "Calendar events listed".to_string()))
            }
            "create_event" => {
                let summary = p.required_str("summary")?.to_string();
                let description = p.optional_str("description")?.map(String::from);
                let start = p.required_str("start")?.to_string();
                let end = p.required_str("end")?.to_string();

                let result = self
                    .handler
                    .create_event(summary, description, start, end)
                    .await?;
                Ok(serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|_| "Calendar event created".to_string()))
            }
            "status" => {
                let result = self.handler.get_status().await?;
                Ok(serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|_| "Calendar status retrieved".to_string()))
            }
            _ => {
                Err(ToolError::InvalidParams(format!("Unknown calendar action: {}", action)).into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct MockCalendarHandler;

    #[async_trait]
    impl CalendarHandler for MockCalendarHandler {
        async fn sync_calendar(&self) -> Result<Value> {
            Ok(json!({
                "status": "success",
                "events_synced": 5,
                "conflicts": 0
            }))
        }

        async fn list_events(&self, limit: usize) -> Result<Value> {
            Ok(json!({
                "events": [],
                "limit": limit
            }))
        }

        async fn create_event(
            &self,
            summary: String,
            description: Option<String>,
            start: String,
            end: String,
        ) -> Result<Value> {
            Ok(json!({
                "status": "created",
                "summary": summary,
                "description": description,
                "start": start,
                "end": end
            }))
        }

        async fn get_status(&self) -> Result<Value> {
            Ok(json!({
                "last_sync": "2026-02-14T10:00:00Z",
                "events_count": 42
            }))
        }
    }

    #[tokio::test]
    async fn test_calendar_handler_trait() {
        let handler = MockCalendarHandler;

        let result = handler.sync_calendar().await.unwrap();
        assert_eq!(result["status"], "success");

        let result = handler.list_events(10).await.unwrap();
        assert_eq!(result["limit"], 10);

        let result = handler
            .create_event(
                "Meeting".to_string(),
                Some("Discuss plans".to_string()),
                "2026-03-01T10:00:00Z".to_string(),
                "2026-03-01T11:00:00Z".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(result["summary"], "Meeting");

        let result = handler.get_status().await.unwrap();
        assert!(result["events_count"].is_number());
    }

    // Tests for CalendarTool Tool trait implementation
    use super::CalendarTool;
    use crate::{RoutingContext, Tool};
    use std::sync::Arc;

    fn test_ctx() -> RoutingContext {
        RoutingContext::new(
            common::ChannelName::new("terminal"),
            common::ChatId::new("test-chat"),
        )
    }

    #[tokio::test]
    async fn test_calendar_tool_sync_action() {
        let handler = Arc::new(MockCalendarHandler);
        let tool = CalendarTool::new(handler);
        let ctx = test_ctx();

        let args = json!({ "action": "sync" });
        let result = tool.execute(args, &ctx).await.unwrap();

        assert!(result.contains("success"));
        assert!(result.contains("5"));
    }

    #[tokio::test]
    async fn test_calendar_tool_list_action() {
        let handler = Arc::new(MockCalendarHandler);
        let tool = CalendarTool::new(handler);
        let ctx = test_ctx();

        let args = json!({ "action": "list_events", "limit": 15 });
        let result = tool.execute(args, &ctx).await.unwrap();

        assert!(result.contains("15"));
    }

    #[tokio::test]
    async fn test_calendar_tool_create_action() {
        let handler = Arc::new(MockCalendarHandler);
        let tool = CalendarTool::new(handler);
        let ctx = test_ctx();

        let args = json!({
            "action": "create_event",
            "summary": "Team Meeting",
            "description": "Quarterly planning",
            "start": "2026-03-15T14:00:00Z",
            "end": "2026-03-15T15:00:00Z"
        });
        let result = tool.execute(args, &ctx).await.unwrap();

        assert!(result.contains("created"));
        assert!(result.contains("Team Meeting"));
    }

    #[tokio::test]
    async fn test_calendar_tool_status_action() {
        let handler = Arc::new(MockCalendarHandler);
        let tool = CalendarTool::new(handler);
        let ctx = test_ctx();

        let args = json!({ "action": "status" });
        let result = tool.execute(args, &ctx).await.unwrap();

        assert!(result.contains("42"));
    }

    #[tokio::test]
    async fn test_calendar_tool_missing_action() {
        let handler = Arc::new(MockCalendarHandler);
        let tool = CalendarTool::new(handler);
        let ctx = test_ctx();

        let args = json!({});
        let result = tool.execute(args, &ctx).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_calendar_tool_invalid_action() {
        let handler = Arc::new(MockCalendarHandler);
        let tool = CalendarTool::new(handler);
        let ctx = test_ctx();

        let args = json!({ "action": "invalid_action" });
        let result = tool.execute(args, &ctx).await;

        assert!(result.is_err());
    }
}
