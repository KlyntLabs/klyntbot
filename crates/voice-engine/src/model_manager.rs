//! Voice model lifecycle management.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ModelState {
    NotDownloaded,
    Downloading { progress: f32 },
    Ready { path: PathBuf },
    Failed { error: String },
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
        let dir = self.models_dir.join("qwen3-tts-0.6b");
        dir.is_dir().then_some(dir)
    }

    /// Check if Qwen3-ASR model exists.
    pub fn qwen3_asr_model_dir(&self) -> Option<PathBuf> {
        let dir = self.models_dir.join("qwen3-asr-0.6b");
        dir.is_dir().then_some(dir)
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
}
