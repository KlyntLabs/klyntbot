pub mod coding;
pub mod cognitive_commands;
pub mod commands;
pub mod entity_link_types;
pub mod errors;
pub mod events;
pub mod permissions;
pub mod specta_helpers;
pub mod types;

pub use entity_link_types::*;
pub use errors::{ApiError, CommandResult};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct HooksTomlSnapshot {
    pub path: String,
    pub exists: bool,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RewindResult {
    pub messages_removed: u64,
    pub files_restored: usize,
    pub files_deleted: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SessionExportResult {
    pub path: String,
    pub bytes_written: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ExportFormat {
    Md,
    Json,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SessionForkResult {
    pub new_session_key: String,
}

/// Args for `coding_sessions_export`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionExportArgs {
    pub session_key: String,
    pub format: ExportFormat,
}

/// Args for `coding_sessions_fork`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionForkArgs {
    pub session_key: String,
    pub up_to_message: Option<String>,
}

/// Args for `coding_sessions_rewind`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionRewindArgs {
    pub session_key: String,
    pub message_id: String,
}

/// Args for `coding_permissions_clear_mirror`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ClearMirrorCacheArgs {
    pub tool: String,
    pub repo_id: Option<String>,
}

#[cfg(test)]
mod phase5_helper_tests {
    use crate::specta_helpers::{JsonValue, Timestamp};
    use specta::Type;

    #[test]
    fn json_value_specta_type_renders_as_unknown() {
        let mut type_map = specta::TypeMap::default();
        let dt = JsonValue::inline(&mut type_map, specta::Generics::Provided(&[]));
        // We only require this not to panic; the precise rendering is checked in
        // bindings_are_current.
        let _ = dt;
    }

    #[test]
    fn timestamp_specta_type_renders_as_string() {
        let mut type_map = specta::TypeMap::default();
        let dt = Timestamp::inline(&mut type_map, specta::Generics::Provided(&[]));
        let _ = dt;
    }
}
