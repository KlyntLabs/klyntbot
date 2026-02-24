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
