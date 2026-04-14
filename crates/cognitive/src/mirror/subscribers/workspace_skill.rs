//! WorkspaceSkillSubscriber — keeps `skills/workspace/SKILL.md` in sync with
//! the current set of databases.
//!
//! On every `DatabaseCreated` / `DatabaseDeleted` event, regenerates the
//! marker-delimited "Known Databases" block in the workspace skill file and
//! asks the `SkillStore` to reload from disk.

use std::path::PathBuf;
use std::sync::Arc;

use bus::DomainEvent;
use entity_store::store::EntityStore;
use skill_system::SkillStore;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

const START_MARKER: &str = "<!-- databases:start -->";
const END_MARKER: &str = "<!-- databases:end -->";

pub struct WorkspaceSkillSubscriber {
    skills_dir: PathBuf,
    entity_store: Arc<EntityStore>,
    skill_store: Arc<RwLock<SkillStore>>,
}

impl WorkspaceSkillSubscriber {
    pub fn new(
        skills_dir: PathBuf,
        entity_store: Arc<EntityStore>,
        skill_store: Arc<RwLock<SkillStore>>,
    ) -> Self {
        Self {
            skills_dir,
            entity_store,
            skill_store,
        }
    }

    pub async fn run(self, mut rx: broadcast::Receiver<DomainEvent>, shutdown: CancellationToken) {
        // Regenerate once on start so a newly-added marker gets filled in.
        self.regenerate().await;
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    debug!("WorkspaceSkillSubscriber: shutdown");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::DatabaseCreated { .. })
                        | Ok(DomainEvent::DatabaseDeleted { .. }) => {
                            self.regenerate().await;
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("WorkspaceSkillSubscriber lagged {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    }

    async fn regenerate(&self) {
        let skill_path = self.skills_dir.join("workspace").join("SKILL.md");

        let existing = match tokio::fs::read_to_string(&skill_path).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!(path = %skill_path.display(), "workspace skill missing — skipping");
                return;
            }
            Err(e) => {
                warn!(
                    "WorkspaceSkillSubscriber: read {} failed: {e}",
                    skill_path.display()
                );
                return;
            }
        };

        let databases = match self.entity_store.list_databases().await {
            Ok(dbs) => dbs,
            Err(e) => {
                warn!("WorkspaceSkillSubscriber: list_databases failed: {e}");
                return;
            }
        };

        let rendered = render_block(&databases);
        let Some(updated) = replace_block(&existing, &rendered) else {
            debug!("workspace skill has no database markers — skipping");
            return;
        };
        if updated == existing {
            return;
        }

        // Atomic write: tmp → rename.
        let tmp = skill_path.with_extension("md.tmp");
        if let Err(e) = tokio::fs::write(&tmp, &updated).await {
            warn!("WorkspaceSkillSubscriber: write tmp failed: {e}");
            return;
        }
        if let Err(e) = tokio::fs::rename(&tmp, &skill_path).await {
            warn!("WorkspaceSkillSubscriber: rename failed: {e}");
            return;
        }

        if let Err(e) = self.skill_store.write().await.reload() {
            warn!("WorkspaceSkillSubscriber: skill_store reload failed: {e}");
            return;
        }
        info!(
            "workspace skill regenerated ({} database(s))",
            databases.len()
        );
    }
}

fn render_block(databases: &[entity_store::DatabaseSchema]) -> String {
    if databases.is_empty() {
        return "_No databases yet._\n".to_string();
    }
    let mut out = String::new();
    for db in databases {
        let desc = db.description.as_deref().unwrap_or("");
        let suffix = if desc.is_empty() {
            String::new()
        } else {
            format!(" — {desc}")
        };
        out.push_str(&format!("- **{}** (`{}`){}\n", db.name, db.slug, suffix));
    }
    out
}

/// Replace content between START_MARKER and END_MARKER. Returns None if markers absent.
fn replace_block(source: &str, new_content: &str) -> Option<String> {
    let start = source.find(START_MARKER)?;
    let end_rel = source[start..].find(END_MARKER)?;
    let end = start + end_rel;
    let after_start = start + START_MARKER.len();
    let mut result = String::with_capacity(source.len() + new_content.len());
    result.push_str(&source[..after_start]);
    result.push('\n');
    result.push_str(new_content);
    result.push_str(&source[end..]);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_block_inserts_between_markers() {
        let src = "# Skill\n<!-- databases:start -->\nold\n<!-- databases:end -->\ntail\n";
        let out = replace_block(src, "- **A** (`a`)\n").unwrap();
        assert!(out.contains("- **A** (`a`)"));
        assert!(!out.contains("old"));
        assert!(out.contains("tail"));
    }

    #[test]
    fn replace_block_missing_markers_returns_none() {
        assert!(replace_block("no markers here", "x").is_none());
    }
}
