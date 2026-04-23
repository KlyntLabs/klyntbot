//! Plugin integration tests — require a pre-built hello_plugin.wasm.
//!
//! Build the test plugin first:
//!   cd tests/fixtures/hello_plugin && ./build.sh
//!
//! Run with:
//!   cargo nextest run --features plugin-integration --test plugins

#[cfg(feature = "plugin-integration")]
mod plugin_integration {
    use std::path::PathBuf;

    use common::ChannelName;
    use config::schema::PluginsConfig;
    use plugin_runtime::{PluginManager, PluginManifest, PluginPermission};
    use tools_core::{DynTool, FeaturePackage, RoutingContext};

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("hello_plugin")
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new(ChannelName::new("cli"), "test".into())
    }

    // ── Plugin Execution ────────────────────────────────────────

    #[tokio::test]
    async fn hello_plugin_executes() {
        let fixtures = fixtures_dir();
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
        let config = PluginsConfig::default();
        let manager =
            PluginManager::load_all(tmp.path(), pool.inner().clone(), &config, None, None).unwrap();

        assert_eq!(manager.packages().len(), 1);

        let packages = manager.into_packages();
        let tools: Vec<DynTool> = packages[0].tools();
        let hello_idx = tools.iter().position(|t| t.name() == "hello_tool").unwrap();

        let args = serde_json::json!({"name": "klyntbot"});
        let result = tools[hello_idx]
            .execute(args, &routing_ctx())
            .await
            .unwrap();
        assert!(result.contains("hello from wasm"));
        assert!(result.contains("klyntbot"));
    }

    // ── Manifest Parsing ────────────────────────────────────────

    #[test]
    fn manifest_parses_correctly() {
        let manifest_path = fixtures_dir().join("klyntbot.plugin.json");
        let manifest = PluginManifest::from_file(&manifest_path).unwrap();
        assert_eq!(manifest.id, "hello-plugin");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.tools.len(), 2);
        assert_eq!(manifest.tools[0].name, "hello_tool");
        assert_eq!(manifest.tools[1].name, "emit_test_event");
        assert!(manifest.has_permission(&PluginPermission::Agent));
    }

    #[test]
    fn permission_level_agent_permission_present() {
        let manifest_path = fixtures_dir().join("klyntbot.plugin.json");
        let manifest = PluginManifest::from_file(&manifest_path).unwrap();
        assert!(!manifest.has_permission(&PluginPermission::Network));
        assert!(manifest.has_permission(&PluginPermission::Agent));
        assert_eq!(manifest.permissions.len(), 1);
    }

    // ── Plugin Manager ──────────────────────────────────────────

    #[tokio::test]
    async fn manager_full_loading_path() {
        use config::schema::PluginsConfig;
        use plugin_runtime::PluginManager;

        let fixtures = fixtures_dir();
        let wasm_path = fixtures.join("plugin.wasm");
        assert!(
            wasm_path.exists(),
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
        let config = PluginsConfig::default();
        let manager =
            PluginManager::load_all(tmp.path(), pool.inner().clone(), &config, None, None).unwrap();

        assert_eq!(manager.packages().len(), 1);
        assert_eq!(manager.packages()[0].name(), "hello-plugin");

        let packages = manager.into_packages();
        let tools: Vec<DynTool> = packages[0].tools();
        let hello_idx = tools.iter().position(|t| t.name() == "hello_tool").unwrap();
        assert_eq!(tools.len(), 2);

        let args = serde_json::json!({"name": "integration-test"});
        let result = tools[hello_idx]
            .execute(args, &routing_ctx())
            .await
            .unwrap();
        assert!(result.contains("hello from wasm"));
        assert!(result.contains("integration-test"));
    }

    #[test]
    fn manager_scan_discovers_manifests() {
        use plugin_runtime::PluginManager;

        let tmp = tempfile::TempDir::new().unwrap();

        for id in &["plugin-a", "plugin-b"] {
            let dir = tmp.path().join(id);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("klyntbot.plugin.json"),
                serde_json::json!({
                    "id": id,
                    "name": format!("Plugin {}", id),
                    "version": "1.0.0",
                    "description": "test",
                    "author": "test"
                })
                .to_string(),
            )
            .unwrap();
        }

        let results = PluginManager::scan_manifests(tmp.path()).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn cli_install_list_remove_flow() {
        use plugin_runtime::PluginManager;

        let tmp = tempfile::TempDir::new().unwrap();
        let plugins_dir = tmp.path();
        let fixtures = fixtures_dir();

        let dest = plugins_dir.join("hello-plugin");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::copy(fixtures.join("plugin.wasm"), dest.join("plugin.wasm")).unwrap();
        std::fs::copy(
            fixtures.join("klyntbot.plugin.json"),
            dest.join("klyntbot.plugin.json"),
        )
        .unwrap();

        let manifests = PluginManager::scan_manifests(plugins_dir).unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].0.id, "hello-plugin");

        std::fs::remove_dir_all(&dest).unwrap();
        let after_remove = PluginManager::scan_manifests(plugins_dir).unwrap();
        assert!(after_remove.is_empty());
    }
}

#[cfg(not(feature = "plugin-integration"))]
#[test]
fn plugin_tests_require_feature_flag() {
    println!(
        "Skipped: run with --features plugin-integration after building the hello plugin.\n\
         See: tests/fixtures/hello_plugin/build.sh"
    );
}
