//! Voice model lifecycle management.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ModelState {
    NotDownloaded,
    Downloading { progress: f32 },
    Ready { path: PathBuf },
    Failed { error: String },
}

/// Known Qwen3 model variants with their HuggingFace repo IDs.
#[derive(Debug, Clone, Copy)]
pub enum Qwen3Model {
    Tts,
    Asr,
}

impl Qwen3Model {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Tts => "qwen3-tts-0.6b",
            Self::Asr => "qwen3-asr-0.6b",
        }
    }

    fn repo_id(self) -> &'static str {
        match self {
            Self::Tts => "Qwen/Qwen3-TTS-0.6B",
            Self::Asr => "Qwen/Qwen3-ASR-0.6B",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Tts => "Qwen3-TTS-0.6B",
            Self::Asr => "Qwen3-ASR-0.6B",
        }
    }
}

pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(data_dir: &Path) -> Self {
        let models_dir = data_dir.join("models");
        Self { models_dir }
    }

    /// Returns the base models directory.
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// Check if Qwen3-TTS model exists.
    pub fn qwen3_tts_model_dir(&self) -> Option<PathBuf> {
        self.model_dir(Qwen3Model::Tts)
    }

    /// Check if Qwen3-ASR model exists.
    pub fn qwen3_asr_model_dir(&self) -> Option<PathBuf> {
        self.model_dir(Qwen3Model::Asr)
    }

    /// Check if a model directory exists.
    fn model_dir(&self, model: Qwen3Model) -> Option<PathBuf> {
        let dir = self.models_dir.join(model.dir_name());
        dir.is_dir().then_some(dir)
    }

    /// Download a Qwen3 model from HuggingFace if not already present.
    ///
    /// Downloads model files to `{models_dir}/{model_dir_name}/`.
    /// Uses the HuggingFace API to list and fetch files.
    pub async fn download_model(&self, model: Qwen3Model) -> common::Result<PathBuf> {
        let dest = self.models_dir.join(model.dir_name());

        if dest.is_dir() {
            return Ok(dest);
        }

        info!(
            "Downloading {} from HuggingFace ({})...",
            model.display_name(),
            model.repo_id()
        );

        // TODO: Implement actual HuggingFace download.
        // Will create dest dir only after all files are fetched + verified.
        // For now, return an error so callers know the model isn't ready.
        Err(common::KlyntbotError::Config(
            common::ConfigError::NotFound(format!(
                "{} not available — download not yet implemented",
                model.display_name()
            )),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn models_dir_resolves_correctly() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert_eq!(mgr.models_dir(), tmp.path().join("models"));
    }

    #[test]
    fn model_dir_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert!(mgr.qwen3_tts_model_dir().is_none());
        assert!(mgr.qwen3_asr_model_dir().is_none());
    }

    #[test]
    fn model_dir_returns_some_when_present() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("models").join("qwen3-tts-0.6b");
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert_eq!(mgr.qwen3_tts_model_dir(), Some(dir));
    }

    #[test]
    fn qwen3_model_metadata() {
        assert_eq!(Qwen3Model::Tts.dir_name(), "qwen3-tts-0.6b");
        assert_eq!(Qwen3Model::Asr.dir_name(), "qwen3-asr-0.6b");
        assert!(!Qwen3Model::Tts.repo_id().is_empty());
    }
}
