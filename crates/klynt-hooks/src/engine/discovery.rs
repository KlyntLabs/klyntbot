//! Config layer discovery — thin replacement for Codex's ConfigLayerStack.

use std::path::PathBuf;

/// Minimal config-layer enumeration matching klyntbot's needs.
/// Codex has User/Project/Snapshot stacks; klynt uses User and Project only.
#[derive(Debug, Clone, Default)]
pub struct ConfigLayerStack {
    pub user_dir: Option<PathBuf>,
    pub project_dir: Option<PathBuf>,
}

impl ConfigLayerStack {
    pub fn user_then_project(&self) -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Some(u) = &self.user_dir {
            v.push(u.clone());
        }
        if let Some(p) = &self.project_dir {
            v.push(p.clone());
        }
        v
    }
}
