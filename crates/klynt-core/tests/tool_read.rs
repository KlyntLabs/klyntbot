use common::{ChannelName, ChatId};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::read::ReadTool;
use std::sync::Arc;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn reads_file_inside_cwd() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = ReadTool::new(dir.path().to_path_buf(), privacy);
    let args = serde_json::json!({ "path": "hello.txt" });
    let ctx = RoutingContext::new(ChannelName::new("system"), ChatId::new("system"));
    let out = tool.execute(args, &ctx).await.unwrap();
    assert!(out.contains("hello world"));
}

#[tokio::test]
async fn reads_with_offset_and_limit() {
    let dir = tempfile::tempdir().unwrap();
    let content = (0..10).map(|i| format!("line{i}\n")).collect::<String>();
    std::fs::write(dir.path().join("multi.txt"), &content).unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = ReadTool::new(dir.path().to_path_buf(), privacy);
    let args = serde_json::json!({ "path": "multi.txt", "offset": 3, "limit": 2 });
    let ctx = RoutingContext::new(ChannelName::new("system"), ChatId::new("system"));
    let out = tool.execute(args, &ctx).await.unwrap();
    assert!(out.contains("line3"));
    assert!(out.contains("line4"));
    assert!(!out.contains("line5"));
    assert!(!out.contains("line0"));
}

#[tokio::test]
async fn read_outside_cwd_denied() {
    let dir = tempfile::tempdir().unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = ReadTool::new(dir.path().to_path_buf(), privacy);
    let args = serde_json::json!({ "path": "/etc/passwd" });
    let ctx = RoutingContext::new(ChannelName::new("system"), ChatId::new("system"));
    assert!(tool.execute(args, &ctx).await.is_err());
}

#[test]
fn is_concurrency_safe() {
    let dir = tempfile::tempdir().unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = ReadTool::new(dir.path().to_path_buf(), privacy);
    assert!(<ReadTool as Tool>::is_concurrency_safe(
        &tool,
        &serde_json::json!({})
    ));
}
