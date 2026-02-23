//! Browser integration tests — require a running agent-browser daemon.
//!
//! Run with: cargo nextest run --features browser-integration --test browser_integration_tests

#[cfg(feature = "browser-integration")]
mod browser_integration {
    use klyntbot::tools::browser::BrowserTool;
    use klyntbot::tools::{RoutingContext, Tool};
    use config::TrustLevel;

    fn ctx() -> RoutingContext {
        RoutingContext::new("cli".into(), "test".into())
    }

    /// Requires: agent-browser daemon running, internet access.
    #[tokio::test]
    async fn test_navigate_and_snapshot() {
        let tool = BrowserTool::new(TrustLevel::Full)
            .expect("agent-browser must be installed");

        let args = serde_json::json!({"action": "navigate", "url": "https://example.com"});
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_ok(), "navigate failed: {:?}", result);

        let args = serde_json::json!({"action": "snapshot"});
        let snapshot = tool.execute(args, &ctx()).await.unwrap();
        assert!(!snapshot.is_empty(), "snapshot returned empty");
        assert!(snapshot.contains("@e"), "snapshot has no @e refs: {}", snapshot);
    }

    #[tokio::test]
    async fn test_write_guard_blocks_in_autonomous_mode() {
        let tool = BrowserTool::new(TrustLevel::Autonomous)
            .expect("agent-browser must be installed");

        let args = serde_json::json!({"action": "navigate", "url": "https://example.com"});
        tool.execute(args, &ctx()).await.unwrap();

        // A click on a "submit" label should return the guard message
        let args = serde_json::json!({"action": "click", "element": "@e1 button Submit"});
        let result = tool.execute(args, &ctx()).await.unwrap();
        assert!(
            result.contains("[CONFIRMATION_REQUIRED]"),
            "Expected guard message, got: {}",
            result
        );
    }

    #[tokio::test]
    async fn test_fill_form_fills_fields() {
        let tool = BrowserTool::new(TrustLevel::Full)
            .expect("agent-browser must be installed");

        let navigate = serde_json::json!({
            "action": "navigate",
            "url": "https://httpbin.org/forms/post"
        });
        tool.execute(navigate, &ctx()).await.unwrap();

        let fill = serde_json::json!({
            "action": "fill_form",
            "fields": {
                "Customer name": "Test User",
                "Telephone": "555-1234"
            }
        });
        let result = tool.execute(fill, &ctx()).await.unwrap();
        assert!(result.contains("Filled"), "fill_form result: {}", result);
    }
}

// Provide a compile-time stub so the file always compiles
#[cfg(not(feature = "browser-integration"))]
#[test]
fn browser_integration_tests_require_feature_flag() {
    // Run with: cargo nextest run --features browser-integration --test browser_integration_tests
    println!("Skipped: compile with --features browser-integration to run browser tests");
}
