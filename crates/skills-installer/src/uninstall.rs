use std::sync::Arc;

use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use bus::{DomainEvent, DomainEventBus};
use entity_store::store::EntityStore;
use skill_system::SkillStore;
use skills_marketplace::InstalledSkillsRepo;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UninstallMode {
    SkillOnly,
    ArchiveDatabases,
    DeleteDatabases,
}

pub async fn uninstall(
    mode: UninstallMode,
    name: &str,
    skills_dir: &std::path::Path,
    repo: &InstalledSkillsRepo,
    entity_store: Arc<EntityStore>,
    skill_store: Arc<tokio::sync::RwLock<SkillStore>>,
    event_bus: Arc<DomainEventBus>,
) -> Result<()> {
    let row = repo
        .get(name)
        .await?
        .ok_or_else(|| KlyntbotError::Storage(format!("skill '{name}' not installed")))?;

    match mode {
        UninstallMode::DeleteDatabases => {
            for db_id in &row.bootstrapped_databases {
                let _ = entity_store.delete_database(db_id).await;
            }
        }
        UninstallMode::ArchiveDatabases => {
            for db_id in &row.bootstrapped_databases {
                if let Ok(schema) = entity_store.get_database(db_id).await {
                    let new_name = format!("Archived: {}", schema.name);
                    let _ = entity_store.rename_database(db_id, &new_name).await;
                }
            }
        }
        UninstallMode::SkillOnly => {}
    }

    let dir = skills_dir.join(name);
    let _ = tokio::fs::remove_dir_all(&dir).await;

    repo.delete(name).await?;
    skill_store
        .write()
        .await
        .reload()
        .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

    event_bus.publish(DomainEvent::SkillUninstalled {
        name: name.into(),
        mode: format!("{mode:?}"),
    });
    info!(name = %name, ?mode, "skill uninstalled");
    Ok(())
}
