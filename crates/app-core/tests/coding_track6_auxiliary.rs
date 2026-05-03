//! Integration tests for Track 6 auxiliary commands:
//! coding_review_start, coding_mcp_status, providers_list, provider_status.

use app_core::AppCore;

async fn test_core() -> AppCore {
    let dir = tempfile::TempDir::new().unwrap();
    AppCore::for_test(Some(dir.path().to_path_buf()))
        .await
        .unwrap()
}

#[tokio::test]
async fn coding_review_start_smoke_test() {
    let core = test_core().await;

    // Seed a session so the review has something to reference
    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, approval_mode, total_cost_usd, total_tokens) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("session-1")
    .bind(serde_json::json!({"title": "test-session"}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind("ask_on_risky")
    .bind(0.0)
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    let result = core
        .coding_review_start("session-1", None, Some("inline"))
        .await
        .unwrap();

    assert!(!result.review_id.is_empty());
    assert_eq!(result.thread_id, "session-1");
    assert!(result.summary.contains("stub") || result.summary.contains("Review"));
    assert!(result.issues.is_empty());
}

#[tokio::test]
async fn coding_review_start_with_target() {
    let core = test_core().await;

    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, approval_mode, total_cost_usd, total_tokens) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("session-2")
    .bind(serde_json::json!({}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind("ask_on_risky")
    .bind(0.0)
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    let result = core
        .coding_review_start("session-2", Some("src/main.rs"), None)
        .await
        .unwrap();

    assert!(result.summary.contains("src/main.rs"));
}

#[tokio::test]
async fn coding_mcp_status_returns_configured_servers() {
    let core = test_core().await;

    let result = core.coding_mcp_status(None).await.unwrap();

    // The test config may have zero or more MCP servers configured.
    // We just verify the shape is correct.
    let _total: u32 = result.total_tools;
    // servers is a Vec — length depends on test config
    let _ = result.servers;
}

#[tokio::test]
async fn providers_list_returns_known_providers() {
    let core = test_core().await;

    let result = core.providers_list().await.unwrap();

    // Should contain at least the well-known provider entries
    assert!(!result.providers.is_empty());

    let ids: Vec<String> = result.providers.iter().map(|p| p.id.clone()).collect();
    assert!(ids.contains(&"anthropic".into()), "expected anthropic in provider list");
    assert!(ids.contains(&"openai".into()), "expected openai in provider list");

    // Each provider should have a name and id
    for p in &result.providers {
        assert!(!p.id.is_empty());
        assert!(!p.name.is_empty());
    }
}

#[tokio::test]
async fn provider_status_reflects_key_presence() {
    let core = test_core().await;

    // Test with a known provider ID
    let status = core.provider_status("anthropic").await.unwrap();
    assert_eq!(status.id, "anthropic");

    // In the test environment, API keys are typically empty,
    // so available should be false with an error message.
    if !status.available {
        assert!(
            status.error.is_some(),
            "unavailable provider should report an error"
        );
    }
}

#[tokio::test]
async fn provider_status_unknown_provider_returns_unavailable() {
    let core = test_core().await;

    let status = core.provider_status("nonexistent-provider").await.unwrap();
    assert_eq!(status.id, "nonexistent-provider");
    assert!(!status.available);
}
