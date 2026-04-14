use common::Result;
use skills_marketplace::{InstalledSkill, InstalledSkillsRepo, SourceType};
use tracing::debug;

const BUNDLED_SKILLS: &[&str] = &[
    "task-management",
    "finance-management",
    "automation",
    "notebook",
    "learning",
    "workspace",
];

pub async fn seed_bundled(repo: &InstalledSkillsRepo) -> Result<()> {
    let existing = repo.list().await?;
    let existing_names: std::collections::HashSet<_> =
        existing.iter().map(|s| s.name.clone()).collect();
    let now = chrono::Utc::now().to_rfc3339();
    for name in BUNDLED_SKILLS {
        if existing_names.contains(*name) {
            continue;
        }
        let row = InstalledSkill {
            name: (*name).into(),
            source_type: SourceType::Bundled,
            source_ref: "bundled".into(),
            installed_version: env!("CARGO_PKG_VERSION").into(),
            installed_sha: format!("bundled-{}", env!("CARGO_PKG_VERSION")),
            enabled: true,
            is_adapted: false,
            bootstrapped_databases: vec![],
            installed_at: now.clone(),
            updated_at: now.clone(),
        };
        repo.insert(&row).await?;
        debug!(name = %name, "seeded bundled skill row");
    }
    Ok(())
}
