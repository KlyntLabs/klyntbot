//! Voice model lifecycle management.
//!
//! Downloads Qwen3 models from HuggingFace on first launch.
//! Models are stored in `{data_dir}/models/{model_name}/`.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

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
    TtsInstruct,
    Asr,
}

impl Qwen3Model {
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Tts => "qwen3-tts-0.6b",
            Self::TtsInstruct => "qwen3-tts-1.7b-instruct",
            Self::Asr => "qwen3-asr-0.6b",
        }
    }

    pub fn repo_id(self) -> &'static str {
        match self {
            Self::Tts => "Qwen/Qwen3-TTS-12Hz-0.6B-Base",
            Self::TtsInstruct => "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice",
            Self::Asr => "Qwen/Qwen3-ASR-0.6B",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Tts => "Qwen3-TTS-12Hz-0.6B",
            Self::TtsInstruct => "Qwen3-TTS-12Hz-1.7B-CustomVoice",
            Self::Asr => "Qwen3-ASR-0.6B",
        }
    }
}

/// File entry from HuggingFace API tree listing.
#[derive(Debug, Deserialize)]
struct HfTreeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    path: String,
    size: Option<u64>,
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

    /// Check if Qwen3-TTS 1.7B instruct model exists.
    pub fn qwen3_tts_instruct_model_dir(&self) -> Option<PathBuf> {
        self.model_dir(Qwen3Model::TtsInstruct)
    }

    /// Check if a model directory exists and has a completion marker.
    fn model_dir(&self, model: Qwen3Model) -> Option<PathBuf> {
        let dir = self.models_dir.join(model.dir_name());
        // Only consider complete if the marker file exists
        if dir.join(".complete").exists() {
            Some(dir)
        } else {
            None
        }
    }

    /// Download a Qwen3 model from HuggingFace if not already present.
    ///
    /// Lists files via the HF API, downloads each one, then writes a
    /// `.complete` marker to signal the model is ready.
    pub async fn download_model(&self, model: Qwen3Model) -> common::Result<PathBuf> {
        let dest = self.models_dir.join(model.dir_name());

        if dest.join(".complete").exists() {
            return Ok(dest);
        }

        // Clean up any incomplete previous download attempt
        if dest.exists() {
            let _ = tokio::fs::remove_dir_all(&dest).await;
        }

        tokio::fs::create_dir_all(&dest)
            .await
            .map_err(common::KlyntbotError::Io)?;

        info!(
            "Downloading {} from HuggingFace ({})...",
            model.display_name(),
            model.repo_id()
        );

        let client = common::build_http_client(std::time::Duration::from_secs(600))?;

        // Recursively list all files in the repo (follows subdirectories)
        let files = list_hf_files_recursive(&client, model.repo_id(), "").await?;

        let total_bytes: u64 = files.iter().filter_map(|f| f.size).sum();
        info!(
            "{}: {} files, {:.1} MB total",
            model.display_name(),
            files.len(),
            total_bytes as f64 / 1_048_576.0
        );

        let mut downloaded_bytes: u64 = 0;

        for entry in &files {
            let file_url = format!(
                "https://huggingface.co/{}/resolve/main/{}",
                model.repo_id(),
                entry.path
            );

            // Create parent dirs for nested paths (e.g., "tokenizer/config.json")
            let file_dest = dest.join(&entry.path);
            if let Some(parent) = file_dest.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(common::KlyntbotError::Io)?;
            }

            debug!("Downloading {}", entry.path);

            let response = client.get(&file_url).send().await.map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::Http(format!(
                    "Failed to download {}: {e}",
                    entry.path
                )))
            })?;

            if !response.status().is_success() {
                warn!("Skipping {} (HTTP {})", entry.path, response.status());
                continue;
            }

            let mut file = tokio::fs::File::create(&file_dest)
                .await
                .map_err(common::KlyntbotError::Io)?;

            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| {
                    common::KlyntbotError::Provider(common::ProviderError::Http(format!(
                        "Download stream error for {}: {e}",
                        entry.path
                    )))
                })?;
                file.write_all(&chunk)
                    .await
                    .map_err(common::KlyntbotError::Io)?;
                downloaded_bytes += chunk.len() as u64;
            }

            file.flush().await.map_err(common::KlyntbotError::Io)?;

            if total_bytes > 0 {
                let progress = downloaded_bytes as f64 / total_bytes as f64 * 100.0;
                debug!(
                    "{}: {:.0}% ({:.1} MB / {:.1} MB)",
                    model.display_name(),
                    progress,
                    downloaded_bytes as f64 / 1_048_576.0,
                    total_bytes as f64 / 1_048_576.0
                );
            }
        }

        // Write completion marker — model_dir() checks for this
        tokio::fs::write(dest.join(".complete"), model.repo_id().as_bytes())
            .await
            .map_err(common::KlyntbotError::Io)?;

        info!(
            "{} downloaded successfully ({:.1} MB)",
            model.display_name(),
            downloaded_bytes as f64 / 1_048_576.0
        );

        Ok(dest)
    }
}

/// Recursively list all downloadable files from a HuggingFace repo tree.
fn list_hf_files_recursive<'a>(
    client: &'a reqwest::Client,
    repo_id: &'a str,
    prefix: &'a str,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = common::Result<Vec<HfTreeEntry>>> + Send + 'a>,
> {
    Box::pin(async move { list_hf_files_inner(client, repo_id, prefix).await })
}

async fn list_hf_files_inner(
    client: &reqwest::Client,
    repo_id: &str,
    prefix: &str,
) -> common::Result<Vec<HfTreeEntry>> {
    let tree_url = if prefix.is_empty() {
        format!("https://huggingface.co/api/models/{}/tree/main", repo_id)
    } else {
        format!(
            "https://huggingface.co/api/models/{}/tree/main/{}",
            repo_id, prefix
        )
    };

    let response = client.get(&tree_url).send().await.map_err(|e| {
        common::KlyntbotError::Provider(common::ProviderError::Http(format!(
            "Failed to list {repo_id}/{prefix}: {e}"
        )))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(common::KlyntbotError::Provider(
            common::ProviderError::Http(format!(
                "HuggingFace API returned HTTP {status} for {repo_id}/{prefix}"
            )),
        ));
    }

    let entries: Vec<HfTreeEntry> = response.json().await.map_err(|e| {
        common::KlyntbotError::Provider(common::ProviderError::Http(format!(
            "Failed to parse HF tree response: {e}"
        )))
    })?;

    let mut files = Vec::new();
    for entry in entries {
        if entry.entry_type == "directory" {
            let sub = list_hf_files_recursive(client, repo_id, &entry.path).await?;
            files.extend(sub);
        } else if entry.entry_type == "file"
            && !entry.path.starts_with('.')
            && !entry.path.eq_ignore_ascii_case("README.md")
            && !entry.path.eq_ignore_ascii_case("LICENSE")
        {
            files.push(entry);
        }
    }

    Ok(files)
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
    fn model_dir_returns_some_when_complete() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("models").join("qwen3-tts-0.6b");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".complete"), "done").unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert_eq!(mgr.qwen3_tts_model_dir(), Some(dir));
    }

    #[test]
    fn model_dir_returns_none_when_incomplete() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("models").join("qwen3-tts-0.6b");
        std::fs::create_dir_all(&dir).unwrap();
        // No .complete marker — download was interrupted
        let mgr = ModelManager::new(tmp.path());
        assert!(mgr.qwen3_tts_model_dir().is_none());
    }

    #[test]
    fn qwen3_model_metadata() {
        assert_eq!(Qwen3Model::Tts.dir_name(), "qwen3-tts-0.6b");
        assert_eq!(Qwen3Model::Asr.dir_name(), "qwen3-asr-0.6b");
        assert!(!Qwen3Model::Tts.repo_id().is_empty());
    }

    #[test]
    fn tts_instruct_model_metadata() {
        assert_eq!(Qwen3Model::TtsInstruct.dir_name(), "qwen3-tts-1.7b-instruct");
        assert_eq!(
            Qwen3Model::TtsInstruct.repo_id(),
            "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice"
        );
    }

    #[test]
    fn tts_instruct_model_dir_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert!(mgr.qwen3_tts_instruct_model_dir().is_none());
    }
}
