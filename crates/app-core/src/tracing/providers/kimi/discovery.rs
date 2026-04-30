//! Discover Kimi sessions from `~/.kimi/sessions/<hash>/<uuid>/`
//! plus imported sessions from `{klyntbot_data}/coding_memory/imported_wire/<uuid>/`.

use common::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveredSession {
    pub session_id: String,
    pub source_dir: PathBuf,
    pub hash: Option<String>,
    pub imported: bool,
}

pub async fn discover_sessions(
    kimi_root: &Path,
    imported_root: &Path,
) -> Result<Vec<DiscoveredSession>> {
    let mut out = Vec::new();
    scan_native(kimi_root, &mut out).await?;
    scan_imported(imported_root, &mut out).await?;
    Ok(out)
}

async fn scan_native(root: &Path, out: &mut Vec<DiscoveredSession>) -> Result<()> {
    let mut hashes = match tokio::fs::read_dir(root).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("read {}: {e}", root.display()))),
    };
    while let Some(hash_entry) = hashes.next_entry().await
        .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
    {
        if !hash_entry.file_type().await.map_or(false, |t| t.is_dir()) { continue; }
        let hash = hash_entry.file_name().to_string_lossy().to_string();
        if hash.starts_with('.') { continue; }
        let mut sessions = match tokio::fs::read_dir(hash_entry.path()).await {
            Ok(d) => d,
            Err(_) => continue,
        };
        while let Some(s) = sessions.next_entry().await
            .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
        {
            if !s.file_type().await.map_or(false, |t| t.is_dir()) { continue; }
            let session_dir = s.path();
            if tokio::fs::metadata(session_dir.join("wire.jsonl")).await.is_err() { continue; }
            out.push(DiscoveredSession {
                session_id: s.file_name().to_string_lossy().to_string(),
                source_dir: session_dir,
                hash: Some(hash.clone()),
                imported: false,
            });
        }
    }
    Ok(())
}

async fn scan_imported(root: &Path, out: &mut Vec<DiscoveredSession>) -> Result<()> {
    let mut entries = match tokio::fs::read_dir(root).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(common::KlyntbotError::Storage(format!("read {}: {e}", root.display()))),
    };
    while let Some(s) = entries.next_entry().await
        .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
    {
        if !s.file_type().await.map_or(false, |t| t.is_dir()) { continue; }
        if tokio::fs::metadata(s.path().join("wire.jsonl")).await.is_err() { continue; }
        out.push(DiscoveredSession {
            session_id: s.file_name().to_string_lossy().to_string(),
            source_dir: s.path(),
            hash: None,
            imported: true,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    #[tokio::test]
    async fn finds_native_sessions() {
        let tmp = tempfile::tempdir().unwrap();
        let kimi_root = tmp.path().join("kimi/sessions");
        let session_dir = kimi_root.join("hash1/sessA");
        fs::create_dir_all(&session_dir).await.unwrap();
        fs::write(session_dir.join("wire.jsonl"), "").await.unwrap();
        let imported_root = tmp.path().join("imported");
        let out = discover_sessions(&kimi_root, &imported_root).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "sessA");
        assert_eq!(out[0].hash.as_deref(), Some("hash1"));
        assert!(!out[0].imported);
    }

    #[tokio::test]
    async fn finds_imported_sessions() {
        let tmp = tempfile::tempdir().unwrap();
        let kimi_root = tmp.path().join("kimi/sessions");
        fs::create_dir_all(&kimi_root).await.unwrap();
        let imported_root = tmp.path().join("imported");
        let imp = imported_root.join("imp-uuid-1");
        fs::create_dir_all(&imp).await.unwrap();
        fs::write(imp.join("wire.jsonl"), "").await.unwrap();
        let out = discover_sessions(&kimi_root, &imported_root).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "imp-uuid-1");
        assert!(out[0].imported);
        assert_eq!(out[0].hash, None);
    }

    #[tokio::test]
    async fn skips_dirs_without_wire() {
        let tmp = tempfile::tempdir().unwrap();
        let kimi_root = tmp.path().join("kimi/sessions");
        fs::create_dir_all(kimi_root.join("hash1/sessA")).await.unwrap();
        let imported_root = tmp.path().join("imported");
        fs::create_dir_all(&imported_root).await.unwrap();
        let out = discover_sessions(&kimi_root, &imported_root).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn missing_kimi_root_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let kimi_root = tmp.path().join("nope");
        let imported_root = tmp.path().join("imported");
        let out = discover_sessions(&kimi_root, &imported_root).await.unwrap();
        assert!(out.is_empty());
    }
}
