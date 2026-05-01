use common::{ChannelName, ChatId};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::grep::GrepTool;
use std::sync::Arc;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn finds_pattern_across_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn foo() {}\nfn bar() {}\n").unwrap();
    std::fs::write(dir.path().join("b.rs"), "fn baz() {}\n").unwrap();
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let tool = GrepTool::new(dir.path().to_path_buf(), privacy);
    let out = tool
        .execute(
            serde_json::json!({"pattern": "fn (foo|baz)"}),
            &RoutingContext::new(ChannelName::new("system"), ChatId::new("system")),
        )
        .await
        .unwrap();
    assert!(out.contains("a.rs:1:fn foo"));
    assert!(out.contains("b.rs:1:fn baz"));
    assert!(!out.contains("bar"));
}

#[tokio::test]
async fn case_insensitive_flag() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.rs"), "FOO\n").unwrap();
    let tool = GrepTool::new(
        dir.path().to_path_buf(),
        Arc::new(PrivacyGuard::from_globs(&[]).unwrap()),
    );
    let out = tool
        .execute(
            serde_json::json!({"pattern":"foo","case_insensitive":true}),
            &RoutingContext::new(ChannelName::new("system"), ChatId::new("system")),
        )
        .await
        .unwrap();
    assert!(out.contains("FOO"));
}
