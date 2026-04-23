//! Integration test: fixture plugin emits agent_emit_event; bus consumer receives it.

#![cfg(feature = "plugin-integration")]

use bus::{DomainEvent, DomainEventBus};
use common::{ChannelName, ChatId};
use plugin_runtime::PluginManager;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tools_core::FeaturePackage;

#[tokio::test]
async fn fixture_plugin_emit_published_to_bus() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("hello_plugin");
    assert!(
        fixtures.join("plugin.wasm").exists(),
        "Build the hello plugin first: cd tests/fixtures/hello_plugin && ./build.sh"
    );

    let tmp = tempfile::TempDir::new().unwrap();
    let plugin_dir = tmp.path().join("hello-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::copy(fixtures.join("plugin.wasm"), plugin_dir.join("plugin.wasm")).unwrap();
    std::fs::copy(
        fixtures.join("klyntbot.plugin.json"),
        plugin_dir.join("klyntbot.plugin.json"),
    )
    .unwrap();

    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let config = config::schema::PluginsConfig::default();
    let bus = Arc::new(DomainEventBus::new(256));
    let mut rx = bus.subscribe();

    let manager = PluginManager::load_all(
        tmp.path(),
        pool.inner().clone(),
        &config,
        None,
        Some(Arc::clone(&bus)),
    )
    .unwrap();

    assert_eq!(manager.packages().len(), 1);

    let packages = manager.into_packages();
    let tools = packages[0].tools();
    let emit_tool = tools
        .iter()
        .find(|t: &&tools_core::DynTool| t.name() == "emit_test_event")
        .expect("emit_test_event tool found");

    let routing_ctx = tools_core::RoutingContext::new(ChannelName::new("cli"), ChatId::new("test"));
    let result = emit_tool
        .execute(serde_json::json!({}), &routing_ctx)
        .await
        .expect("plugin invocation succeeded");
    assert_eq!(result, r#"{"ok":true}"#);

    let event = timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for event")
        .expect("event received");

    match event {
        DomainEvent::PluginEvent {
            plugin_id,
            kind,
            payload,
        } => {
            assert_eq!(plugin_id, "hello-plugin");
            assert_eq!(kind, "hello_emitted");
            assert_eq!(payload, serde_json::json!({"hello": "from plugin"}));
        }
        other => panic!("expected PluginEvent, got {:?}", other),
    }
}
