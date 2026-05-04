use klynt_core::tools::tool_search::ToolSearchTool;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn returns_empty_array() {
    let tool = ToolSearchTool::new();
    let ctx = RoutingContext::new(
        common::ChannelName::new("system"),
        common::ChatId::new("test"),
    );
    let out = tool
        .execute(serde_json::json!({"query":"diff"}), &ctx)
        .await
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(parsed.is_array());
    // tool_search is no longer a stub; "diff" returns curated hits (e.g., apply_patch).
    // Keep this loose so future curated additions don't break the test.
    assert!(!parsed.as_array().unwrap().is_empty());
}
