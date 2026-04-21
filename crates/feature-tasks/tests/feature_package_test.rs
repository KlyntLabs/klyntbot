use tools_core::FeaturePackage;

#[tokio::test]
async fn tasks_migration_version_matches_trait_and_bootstrap() {
    let f = feature_tasks::TasksFeature::new();
    let migrations = f.migrations();
    assert_eq!(migrations.len(), 1);
    let m = &migrations[0];
    assert_eq!(m.version, 2, "trait must return current schema version");

    // Verify bootstrap path uses the trait (no manual FeatureMigration literals)
    let bootstrap_src = include_str!("../../../crates/app-core/src/init/storage.rs");
    assert!(
        !bootstrap_src.contains("FeatureMigration { feature_name: \"tasks\""),
        "storage.rs must not construct TasksFeature migrations manually"
    );
}
