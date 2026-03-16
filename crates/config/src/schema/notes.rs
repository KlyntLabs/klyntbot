use serde::{Deserialize, Serialize};

fn default_max_versions_per_note() -> usize {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotesConfig {
    /// Maximum number of version snapshots to keep per note.
    #[serde(default = "default_max_versions_per_note")]
    pub max_versions_per_note: usize,
}

impl Default for NotesConfig {
    fn default() -> Self {
        Self {
            max_versions_per_note: default_max_versions_per_note(),
        }
    }
}
