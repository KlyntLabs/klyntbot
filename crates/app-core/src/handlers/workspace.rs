use desktop_shared::commands::{WorkspaceFile, WorkspaceFileContent};
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

/// Workspace files the user may view/edit, with human-readable descriptions.
///
/// Note: agent personality/tone is owned by `KLYNTBOT.md` in the data dir
/// (served by `SoulContextSource`). The legacy `SOUL.md` workspace file was
/// retired because two soul-like files at different priorities silently
/// overrode each other. The remaining workspace files cover auxiliary
/// instructions that layer on top of the soul.
const WORKSPACE_FILES: &[(&str, &str)] = &[
    ("AGENTS.md", "Agent instructions and guidelines"),
    ("USER.md", "Your profile and preferences"),
    ("TOOLS.md", "Available tools documentation"),
    ("RESPONSE.md", "Response formatting rules"),
    ("HEARTBEAT.md", "Periodic tasks (checked every 30 min)"),
];

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn workspace_list_files(&self) -> Result<Vec<WorkspaceFile>, ApiError> {
        let workspace = self.config.read().await.workspace_path();
        let mut files = Vec::with_capacity(WORKSPACE_FILES.len());
        for &(name, description) in WORKSPACE_FILES {
            let exists = workspace.join(name).exists();
            files.push(WorkspaceFile {
                name: name.to_string(),
                description: description.to_string(),
                exists,
            });
        }
        Ok(files)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn workspace_read_file(
        &self,
        filename: &str,
    ) -> Result<WorkspaceFileContent, ApiError> {
        if !WORKSPACE_FILES.iter().any(|&(n, _)| n == filename) {
            return Err(ApiError::new(
                "INVALID_FILE",
                format!("'{filename}' is not an editable workspace file"),
            ));
        }

        let path = self.config.read().await.workspace_path().join(filename);
        let content = if path.exists() {
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?
        } else {
            embedded_template(filename).unwrap_or_default().to_string()
        };

        Ok(WorkspaceFileContent {
            name: filename.to_string(),
            content,
        })
    }

    #[tracing::instrument(skip(self, content), err)]
    pub async fn workspace_write_file(
        &self,
        filename: &str,
        content: &str,
    ) -> Result<WorkspaceFileContent, ApiError> {
        if !WORKSPACE_FILES.iter().any(|&(n, _)| n == filename) {
            return Err(ApiError::new(
                "INVALID_FILE",
                format!("'{filename}' is not an editable workspace file"),
            ));
        }

        let workspace = self.config.read().await.workspace_path();
        let path = workspace.join(filename);

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;
        }

        tokio::fs::write(&path, content)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;

        Ok(WorkspaceFileContent {
            name: filename.to_string(),
            content: content.to_string(),
        })
    }
}

fn embedded_template(filename: &str) -> Option<&'static str> {
    match filename {
        "AGENTS.md" => Some(include_str!("../../../../workspace/AGENTS.md")),
        "USER.md" => Some(include_str!("../../../../workspace/USER.md")),
        "TOOLS.md" => Some(include_str!("../../../../workspace/TOOLS.md")),
        "RESPONSE.md" => Some(include_str!("../../../../workspace/RESPONSE.md")),
        "HEARTBEAT.md" => Some(include_str!("../../../../workspace/HEARTBEAT.md")),
        _ => None,
    }
}
