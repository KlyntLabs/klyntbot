use common::{ChannelName, ChatId};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::glob::GlobTool;
use std::sync::Arc;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn finds_files_by_pattern() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "").unwrap();
    std::fs::write(dir.path().join("b.rs"), "").unwrap();
    std::fs::write(dir.path().join("c.txt"), "").unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = GlobTool::new(dir.path().to_path_buf(), privacy);
    let args = serde_json::json!({ "pattern": "*.rs" });
    let ctx = RoutingContext::new(ChannelName::new("system"), ChatId::new("system"));
    let out = tool.execute(args, &ctx).await.unwrap();
    assert!(out.contains("a.rs"));
    assert!(out.contains("b.rs"));
    assert!(!out.contains("c.txt"));
}

#[tokio::test]
async fn respects_max_results() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..50 {
        std::fs::write(dir.path().join(format!("f{i}.rs")), "").unwrap();
    }
    let tool = GlobTool::new(
        dir.path().to_path_buf(),
        Arc::new(PrivacyGuard::from_globs(&[]).unwrap()),
    );
    let out = tool
        .execute(
            serde_json::json!({"pattern":"*.rs","max_results":10}),
            &RoutingContext::new(ChannelName::new("system"), ChatId::new("system")),
        )
        .await
        .unwrap();
    assert_eq!(out.lines().count(), 10);
}

#[tokio::test]
async fn skips_privacy_excluded() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".env"), "secret").unwrap();
    std::fs::write(dir.path().join("ok.rs"), "").unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&["**/.env"]).unwrap());
    let tool = GlobTool::new(dir.path().to_path_buf(), privacy);
    let out = tool
        .execute(
            serde_json::json!({"pattern":"*"}),
            &RoutingContext::new(ChannelName::new("system"), ChatId::new("system")),
        )
        .await
        .unwrap();
    assert!(out.contains("ok.rs"));
    assert!(!out.contains(".env"));
}
