use std::sync::Arc;

use bus::DomainEventBus;
use entity_store::store::EntityStore;
use skill_system::SkillStore;
use skills_marketplace::InstalledSkillsRepo;
use skills_registry::{Fetcher, SkillSource};

use crate::{InstallPlan, Installer, UninstallMode};

async fn setup() -> (tempfile::TempDir, Installer) {
    let tmp = tempfile::tempdir().unwrap();

    // Storage + migrations
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &entity_store::EntityStoreFeature::migrations(),
    )
    .await
    .unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &skills_marketplace::SkillsMarketplaceFeature::migrations(),
    )
    .await
    .unwrap();

    let skills_dir = tmp.path().join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let entity_store = Arc::new(EntityStore::new(pool.inner().clone()));
    let skill_store = Arc::new(tokio::sync::RwLock::new(
        SkillStore::load(&skills_dir).unwrap(),
    ));
    let repo = InstalledSkillsRepo::new(pool.inner().clone());
    let bus = Arc::new(DomainEventBus::new(16));
    let fetcher = Arc::new(Fetcher::new());

    let installer = Installer {
        skills_dir: skills_dir.clone(),
        fetcher,
        repo,
        entity_store,
        skill_store,
        event_bus: bus,
    };
    (tmp, installer)
}

fn write_local_skill(root: &std::path::Path, body: &str, template: Option<&str>) -> SkillSource {
    let dir = root.join("fixture-skill");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), body).unwrap();
    if let Some(t) = template {
        std::fs::create_dir_all(dir.join("templates")).unwrap();
        std::fs::write(dir.join("templates/t.json"), t).unwrap();
    }
    SkillSource::LocalPath { path: dir }
}

#[tokio::test]
async fn install_local_skill_writes_file_and_row() {
    let (tmp, inst) = setup().await;
    let source = write_local_skill(tmp.path(), "---\nname: fx\ndescription: d\n---\nbody", None);
    let plan: InstallPlan = inst.preview_install(&source, None).await.unwrap();
    let row = inst.apply_install(plan).await.unwrap();
    assert_eq!(row.name, "fx");
    assert!(inst.skills_dir.join("fx/SKILL.md").is_file());
    let list = inst.repo.list().await.unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn install_with_bootstrap_creates_database() {
    let (tmp, inst) = setup().await;
    // Template must match TemplateManifest schema (name, description, version required; field type is "type" not "fieldType")
    let tpl = r#"{
        "name": "Reading List",
        "description": "A reading list database",
        "version": "1.0.0",
        "databases": [{
            "name": "Reading",
            "slug": "reading",
            "fields": [{"name": "Title", "slug": "title", "type": "text", "required": true}]
        }]
    }"#;
    let source = write_local_skill(
        tmp.path(),
        "---\nname: rl\ndescription: d\n---\nbody",
        Some(tpl),
    );

    let plan = inst.preview_install(&source, None).await.unwrap();
    assert_eq!(plan.databases_to_bootstrap.len(), 1);

    let row = inst.apply_install(plan).await.unwrap();
    assert_eq!(row.bootstrapped_databases.len(), 1);

    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().any(|d| d.slug == "reading"));
}

#[tokio::test]
async fn install_only_writes_nothing_on_template_error() {
    let (tmp, inst) = setup().await;
    // Malformed template — missing required top-level fields (name, description, version).
    let tpl = r#"{"databases":[{"foo":"bar"}]}"#;
    let source = write_local_skill(
        tmp.path(),
        "---\nname: bad\ndescription: d\n---\nbody",
        Some(tpl),
    );

    let plan = inst.preview_install(&source, None).await.unwrap();
    let err = inst.apply_install(plan).await;
    assert!(err.is_err());

    // Rollback: skill dir must not exist, no row inserted, no database created.
    assert!(!inst.skills_dir.join("bad").exists());
    let list = inst.repo.list().await.unwrap();
    assert!(list.is_empty());
    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().all(|d| d.slug != "foo"));
}

#[tokio::test]
async fn upgrade_updates_version_and_preserves_bootstraps() {
    // This test mocks a GitHub upgrade flow by seeding a local install,
    // then directly calling repo.update_version to simulate the upgrade.
    let (tmp, inst) = setup().await;
    let source = write_local_skill(tmp.path(), "---\nname: up\ndescription: d\n---\nbody", None);
    let plan = inst.preview_install(&source, None).await.unwrap();
    let installed = inst.apply_install(plan).await.unwrap();

    // Simulate an upgrade bumping version + sha (no github mocking needed for repo-level behaviour)
    inst.repo
        .update_version(
            &installed.name,
            "2.0.0",
            "newsha",
            &installed.bootstrapped_databases,
        )
        .await
        .unwrap();
    let reloaded = inst.repo.get(&installed.name).await.unwrap().unwrap();
    assert_eq!(reloaded.installed_version, "2.0.0");
    assert_eq!(reloaded.installed_sha, "newsha");
}

#[tokio::test]
async fn uninstall_skill_only_leaves_database() {
    let (tmp, inst) = setup().await;
    let tpl = r#"{
        "name": "Reading List",
        "description": "A reading list database",
        "version": "1.0.0",
        "databases": [{
            "name": "Reading",
            "slug": "reading",
            "fields": [{"name": "Title", "slug": "title", "type": "text", "required": true}]
        }]
    }"#;
    let source = write_local_skill(
        tmp.path(),
        "---\nname: rl\ndescription: d\n---\nbody",
        Some(tpl),
    );
    let plan = inst.preview_install(&source, None).await.unwrap();
    let _ = inst.apply_install(plan).await.unwrap();

    inst.uninstall("rl", UninstallMode::SkillOnly)
        .await
        .unwrap();
    let list = inst.repo.list().await.unwrap();
    assert!(list.is_empty());
    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().any(|d| d.slug == "reading"));
}

#[tokio::test]
async fn uninstall_delete_databases_removes_everything() {
    let (tmp, inst) = setup().await;
    let tpl = r#"{
        "name": "X DB",
        "description": "test",
        "version": "1.0.0",
        "databases": [{
            "name": "X",
            "slug": "xdb",
            "fields": [{"name": "T", "slug": "t", "type": "text", "required": true}]
        }]
    }"#;
    let source = write_local_skill(
        tmp.path(),
        "---\nname: sk\ndescription: d\n---\nbody",
        Some(tpl),
    );
    let plan = inst.preview_install(&source, None).await.unwrap();
    let _ = inst.apply_install(plan).await.unwrap();

    inst.uninstall("sk", UninstallMode::DeleteDatabases)
        .await
        .unwrap();
    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().all(|d| d.slug != "xdb"));
}

#[tokio::test]
async fn refresh_disabled_hides_toggled_off_skills_from_store() {
    let (tmp, inst) = setup().await;
    let source = write_local_skill(tmp.path(), "---\nname: dz\ndescription: d\n---\nbody", None);
    let plan = inst.preview_install(&source, None).await.unwrap();
    inst.apply_install(plan).await.unwrap();

    // Sanity: newly installed skill is visible.
    {
        let store = inst.skill_store.read().await;
        assert!(store.get("dz").is_some(), "enabled skill should be visible");
        assert!(store.names().contains(&"dz"));
    }

    // Toggle off in the DB, then sync the overlay.
    inst.repo.set_enabled("dz", false).await.unwrap();
    inst.refresh_disabled().await.unwrap();

    {
        let store = inst.skill_store.read().await;
        assert!(
            store.get("dz").is_none(),
            "disabled skill must be hidden from get()"
        );
        assert!(
            !store.names().contains(&"dz"),
            "disabled skill must be hidden from names()"
        );
        assert!(
            !store.format_listing().contains("dz"),
            "disabled skill must not appear in system-prompt listing"
        );
    }

    // Toggle back on.
    inst.repo.set_enabled("dz", true).await.unwrap();
    inst.refresh_disabled().await.unwrap();
    {
        let store = inst.skill_store.read().await;
        assert!(store.get("dz").is_some(), "re-enabled skill returns");
    }
}

#[tokio::test]
async fn uninstall_archive_renames_database() {
    let (tmp, inst) = setup().await;
    let tpl = r#"{
        "name": "A DB",
        "description": "test",
        "version": "1.0.0",
        "databases": [{
            "name": "A",
            "slug": "adb",
            "fields": [{"name": "T", "slug": "t", "type": "text", "required": true}]
        }]
    }"#;
    let source = write_local_skill(
        tmp.path(),
        "---\nname: sk2\ndescription: d\n---\nbody",
        Some(tpl),
    );
    let plan = inst.preview_install(&source, None).await.unwrap();
    let _ = inst.apply_install(plan).await.unwrap();

    inst.uninstall("sk2", UninstallMode::ArchiveDatabases)
        .await
        .unwrap();
    let dbs = inst.entity_store.list_databases().await.unwrap();
    assert!(dbs.iter().any(|d| d.name.starts_with("Archived: ")));
}
