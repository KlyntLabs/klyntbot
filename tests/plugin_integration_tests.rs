//! Plugin integration tests — require a pre-built hello_plugin.wasm.
//!
//! Build the test plugin first:
//!   cd tests/fixtures/hello_plugin && ./build.sh
//!
//! Run with:
//!   cargo nextest run --features plugin-integration --test plugin_integration_tests

#[cfg(feature = "plugin-integration")]
mod plugin_integration {
    use std::path::PathBuf;

    use common::ChannelName;
    use plugin_runtime::{PluginManifest, PluginPackage, PluginPermission};
    use tools_core::{FeaturePackage, RoutingContext};

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("hello_plugin")
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new(ChannelName::new("cli"), "test".into())
    }

    #[tokio::test]
    async fn test_hello_plugin_executes() {
        let wasm_path = fixtures_dir().join("plugin.wasm");
        assert!(
            wasm_path.exists(),
            "Build the hello plugin first: cd tests/fixtures/hello_plugin && ./build.sh"
        );

        let manifest_path = fixtures_dir().join("klyntbot.plugin.json");
        let manifest = PluginManifest::from_file(&manifest_path).unwrap();

        let wasm_bytes = std::fs::read(&wasm_path).unwrap();
        let extism_manifest = extism::Manifest::new([extism::Wasm::data(wasm_bytes)]);
        let plugin = extism::Plugin::new(&extism_manifest, [], true).unwrap();

        let mut pkg = PluginPackage::from_manifest(manifest);
        pkg.attach_plugin(plugin);

        let tools = pkg.tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "hello_tool");

        let args = serde_json::json!({"name": "klyntbot"});
        let result = tools[0].execute(args, &routing_ctx()).await.unwrap();
        assert!(result.contains("hello from wasm"));
        assert!(result.contains("klyntbot"));
    }

    #[test]
    fn test_hello_plugin_manifest_parses() {
        let manifest_path = fixtures_dir().join("klyntbot.plugin.json");
        let manifest = PluginManifest::from_file(&manifest_path).unwrap();
        assert_eq!(manifest.id, "hello-plugin");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.tools[0].name, "hello_tool");
    }

    #[test]
    fn test_plugin_permission_level_standard_no_network() {
        let manifest_path = fixtures_dir().join("klyntbot.plugin.json");
        let manifest = PluginManifest::from_file(&manifest_path).unwrap();
        // hello_plugin has no permissions — Standard level
        assert!(!manifest.has_permission(&PluginPermission::Network));
        assert!(!manifest.has_permission(&PluginPermission::Agent));
        assert!(manifest.permissions.is_empty());
    }

    /// Full PluginManager loading path: scan → load → register tools → execute.
    /// Simulates what AgentLoopBuilder does at startup.
    #[tokio::test]
    async fn test_plugin_manager_full_loading_path() {
        use config::schema::PluginsConfig;
        use plugin_runtime::PluginManager;

        let fixtures = fixtures_dir();
        let wasm_path = fixtures.join("plugin.wasm");
        assert!(
            wasm_path.exists(),
            "Build the hello plugin first: cd tests/fixtures/hello_plugin && ./build.sh"
        );

        // Create a temp plugins directory mimicking ~/.klyntbot/plugins/
        let tmp = tempfile::TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("hello-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::copy(fixtures.join("plugin.wasm"), plugin_dir.join("plugin.wasm")).unwrap();
        std::fs::copy(
            fixtures.join("klyntbot.plugin.json"),
            plugin_dir.join("klyntbot.plugin.json"),
        )
        .unwrap();

        // Load via PluginManager (same path as AgentLoopBuilder)
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let config = PluginsConfig::default();
        let manager =
            PluginManager::load_all(tmp.path(), pool.inner().clone(), &config, None).unwrap();

        assert_eq!(manager.packages().len(), 1);
        assert_eq!(manager.packages()[0].name(), "hello-plugin");

        // Extract tools (same as builder does with register_dyn)
        let packages = manager.into_packages();
        let tools = packages[0].tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "hello_tool");

        // Execute the tool
        let args = serde_json::json!({"name": "integration-test"});
        let result = tools[0].execute(args, &routing_ctx()).await.unwrap();
        assert!(result.contains("hello from wasm"));
        assert!(result.contains("integration-test"));
    }

    /// Verify PluginManager scan discovers the correct number of plugins.
    #[test]
    fn test_plugin_manager_scan_manifests() {
        use plugin_runtime::PluginManager;

        let tmp = tempfile::TempDir::new().unwrap();

        // Create two plugin directories
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

    /// CLI install → list → remove flow using PluginManager scan.
    #[test]
    fn test_cli_install_list_remove_flow() {
        use plugin_runtime::PluginManager;

        let tmp = tempfile::TempDir::new().unwrap();
        let plugins_dir = tmp.path();
        let fixtures = fixtures_dir();

        // Simulate "plugin install" — copy files
        let dest = plugins_dir.join("hello-plugin");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::copy(fixtures.join("plugin.wasm"), dest.join("plugin.wasm")).unwrap();
        std::fs::copy(
            fixtures.join("klyntbot.plugin.json"),
            dest.join("klyntbot.plugin.json"),
        )
        .unwrap();

        // Simulate "plugin list" — scan manifests
        let manifests = PluginManager::scan_manifests(plugins_dir).unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].0.id, "hello-plugin");

        // Simulate "plugin remove" — delete directory
        std::fs::remove_dir_all(&dest).unwrap();
        let after_remove = PluginManager::scan_manifests(plugins_dir).unwrap();
        assert!(after_remove.is_empty());
    }
}

// Compile-time stub: always compiles, skipped without the feature
#[cfg(not(feature = "plugin-integration"))]
#[test]
fn plugin_integration_tests_require_feature_flag() {
    println!(
        "Skipped: run with --features plugin-integration after building the hello plugin.\n\
         See: tests/fixtures/hello_plugin/build.sh"
    );
}
