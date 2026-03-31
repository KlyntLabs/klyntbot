//! Whisper model download and lifecycle management.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhisperModelSize {
    Small,
    Medium,
}

impl WhisperModelSize {
    pub fn filename(&self) -> &str {
        match self {
            Self::Small => "ggml-small.bin",
            Self::Medium => "ggml-medium.bin",
        }
    }

    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::Small => 488_000_000,
            Self::Medium => 1_530_000_000,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Small => "whisper-small (multilingual)",
            Self::Medium => "whisper-medium (multilingual)",
        }
    }
}

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
    state_tx: watch::Sender<ModelState>,
    state_rx: watch::Receiver<ModelState>,
}

impl ModelManager {
    pub fn new(data_dir: &Path) -> Self {
        let models_dir = data_dir.join("models");
        let initial = if models_dir.join(WhisperModelSize::Small.filename()).exists() {
            ModelState::Ready {
                path: models_dir.join(WhisperModelSize::Small.filename()),
            }
        } else {
            ModelState::NotDownloaded
        };

        let (state_tx, state_rx) = watch::channel(initial);
        Self {
            models_dir,
            state_tx,
            state_rx,
        }
    }

    pub fn state(&self) -> ModelState {
        self.state_rx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<ModelState> {
        self.state_rx.clone()
    }

    pub fn model_path(&self, size: WhisperModelSize) -> Option<PathBuf> {
        let path = self.models_dir.join(size.filename());
        path.exists().then_some(path)
    }

    pub fn is_available(&self, size: WhisperModelSize) -> bool {
        self.models_dir.join(size.filename()).exists()
    }

    /// Check if a Kokoro model exists in `models/kokoro/`.
    /// Returns the directory path if `kokoro.onnx` is present.
    pub fn kokoro_model_dir(&self) -> Option<PathBuf> {
        let kokoro_dir = self.models_dir.join("kokoro");
        if kokoro_dir.join("kokoro.onnx").exists() {
            Some(kokoro_dir)
        } else {
            None
        }
    }

    pub async fn start_download(&self, size: WhisperModelSize) -> common::Result<()> {
        if self.is_available(size) {
            info!("Model {} already available", size.display_name());
            let _ = self.state_tx.send(ModelState::Ready {
                path: self.models_dir.join(size.filename()),
            });
            return Ok(());
        }

        tokio::fs::create_dir_all(&self.models_dir)
            .await
            .map_err(common::KlyntbotError::Io)?;

        let _ = self
            .state_tx
            .send(ModelState::Downloading { progress: 0.0 });

        let url = match size {
            WhisperModelSize::Small => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
            }
            WhisperModelSize::Medium => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin"
            }
        };

        let dest = self.models_dir.join(size.filename());
        let state_tx = self.state_tx.clone();
        let expected_size = size.size_bytes();

        info!("Downloading {} from {}", size.display_name(), url);

        let response = reqwest::get(url).await.map_err(|e| {
            let _ = state_tx.send(ModelState::Failed {
                error: e.to_string(),
            });
            common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
        })?;

        if !response.status().is_success() {
            let err = format!("HTTP {}", response.status());
            let _ = state_tx.send(ModelState::Failed { error: err.clone() });
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::Http(err),
            ));
        }

        let mut file = tokio::fs::File::create(&dest).await.map_err(|e| {
            let _ = state_tx.send(ModelState::Failed {
                error: e.to_string(),
            });
            common::KlyntbotError::Io(e)
        })?;

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                let _ = state_tx.send(ModelState::Failed {
                    error: e.to_string(),
                });
                common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
            })?;
            file.write_all(&chunk).await.map_err(|e| {
                let _ = state_tx.send(ModelState::Failed {
                    error: e.to_string(),
                });
                common::KlyntbotError::Io(e)
            })?;
            downloaded += chunk.len() as u64;
            let progress = (downloaded as f32 / expected_size as f32).min(1.0);
            let _ = state_tx.send(ModelState::Downloading { progress });
        }

        file.flush().await.map_err(common::KlyntbotError::Io)?;

        let _ = state_tx.send(ModelState::Ready { path: dest });
        info!("Model {} downloaded successfully", size.display_name());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn new_manager_detects_not_downloaded() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert_eq!(mgr.state(), ModelState::NotDownloaded);
        assert!(!mgr.is_available(WhisperModelSize::Small));
    }

    #[test]
    fn new_manager_detects_existing_model() {
        let tmp = TempDir::new().unwrap();
        let models = tmp.path().join("models");
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(models.join("ggml-small.bin"), b"fake model").unwrap();

        let mgr = ModelManager::new(tmp.path());
        assert!(matches!(mgr.state(), ModelState::Ready { .. }));
        assert!(mgr.is_available(WhisperModelSize::Small));
        assert!(!mgr.is_available(WhisperModelSize::Medium));
    }

    #[test]
    fn model_path_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert!(mgr.model_path(WhisperModelSize::Small).is_none());
    }

    #[test]
    fn kokoro_model_detected_when_present() {
        let tmp = TempDir::new().unwrap();
        let models_dir = tmp.path().join("models").join("kokoro");
        std::fs::create_dir_all(&models_dir).unwrap();
        std::fs::write(models_dir.join("kokoro.onnx"), b"fake").unwrap();

        let mm = ModelManager::new(tmp.path());
        assert!(mm.kokoro_model_dir().is_some());
    }

    #[test]
    fn kokoro_model_not_detected_when_absent() {
        let tmp = TempDir::new().unwrap();
        let mm = ModelManager::new(tmp.path());
        assert!(mm.kokoro_model_dir().is_none());
    }

    #[test]
    fn model_sizes_have_correct_filenames() {
        assert_eq!(WhisperModelSize::Small.filename(), "ggml-small.bin");
        assert_eq!(WhisperModelSize::Medium.filename(), "ggml-medium.bin");
    }
}
