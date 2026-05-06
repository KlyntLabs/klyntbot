//! Discovers Claude Code sessions from `~/.claude/projects/<encoded-cwd>/*.jsonl`
//! plus imported sessions from `{klyntbot_data}/coding_memory/imported_claude_code/<uuid>/`.

use common::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveredSession {
    pub session_id: String,
    pub jsonl_path: PathBuf,
    pub source_dir: PathBuf,
    pub encoded_cwd: Option<String>,
    pub imported: bool,
}

pub async fn discover_sessions(
    claude_root: &Path,
    imported_root: &Path,
) -> Result<Vec<DiscoveredSession>> {
    let mut out = Vec::new();
    scan_native(claude_root, &mut out).await?;
    scan_imported(imported_root, &mut out).await?;
    Ok(out)
}

async fn scan_native(root: &Path, out: &mut Vec<DiscoveredSession>) -> Result<()> {
    let mut projects = match tokio::fs::read_dir(root).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(common::KlyntbotError::Storage(format!(
                "read {}: {e}",
                root.display()
            )))
        }
    };
    while let Some(project) = projects
        .next_entry()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
    {
        if !project.file_type().await.is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let encoded = project.file_name().to_string_lossy().to_string();
        if encoded.starts_with('.') {
            continue;
        }
        let mut sessions = match tokio::fs::read_dir(project.path()).await {
            Ok(d) => d,
            Err(_) => continue,
        };
        while let Some(s) = sessions
            .next_entry()
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
        {
            if !s.file_type().await.is_ok_and(|t| t.is_file()) {
                continue;
            }
            let path = s.path();
            if path.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                continue;
            }
            let stem = match path.file_stem().and_then(|x| x.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            out.push(DiscoveredSession {
                session_id: stem,
                jsonl_path: path,
                source_dir: project.path(),
                encoded_cwd: Some(encoded.clone()),
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
        Err(e) => {
            return Err(common::KlyntbotError::Storage(format!(
                "read {}: {e}",
                root.display()
            )))
        }
    };
    while let Some(s) = entries
        .next_entry()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
    {
        if !s.file_type().await.is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let dir = s.path();
        let id = s.file_name().to_string_lossy().to_string();
        let jsonl = dir.join(format!("{id}.jsonl"));
        if tokio::fs::metadata(&jsonl).await.is_err() {
            continue;
        }
        out.push(DiscoveredSession {
            session_id: id,
            jsonl_path: jsonl,
            source_dir: dir.clone(),
            encoded_cwd: None,
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
    async fn finds_native_session() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("projects");
        let project = root.join("-Users-foo-bar");
        fs::create_dir_all(&project).await.unwrap();
        fs::write(project.join("aaaa.jsonl"), "").await.unwrap();
        let out = discover_sessions(&root, &tmp.path().join("imp"))
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "aaaa");
        assert_eq!(out[0].encoded_cwd.as_deref(), Some("-Users-foo-bar"));
        assert!(!out[0].imported);
    }

    #[tokio::test]
    async fn skips_subagent_files_inside_session_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("projects");
        let project = root.join("-x");
        fs::create_dir_all(&project).await.unwrap();
        fs::write(project.join("sess.jsonl"), "").await.unwrap();
        // subagent dir inside session-id-named subdir (correct layout)
        let sub = project.join("sess").join("subagents");
        fs::create_dir_all(&sub).await.unwrap();
        fs::write(sub.join("agent-x.jsonl"), "").await.unwrap();
        let out = discover_sessions(&root, &tmp.path().join("imp"))
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "sess");
    }

    #[tokio::test]
    async fn skips_non_jsonl_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("projects");
        let project = root.join("-x");
        fs::create_dir_all(&project).await.unwrap();
        fs::write(project.join("sess.json"), "").await.unwrap();
        fs::write(project.join("sess.txt"), "").await.unwrap();
        let out = discover_sessions(&root, &tmp.path().join("imp"))
            .await
            .unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn finds_imported_session() {
        let tmp = tempfile::tempdir().unwrap();
        let imp = tmp.path().join("imp");
        let dir = imp.join("uuid-1");
        fs::create_dir_all(&dir).await.unwrap();
        fs::write(dir.join("uuid-1.jsonl"), "").await.unwrap();
        let out = discover_sessions(&tmp.path().join("nope"), &imp)
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "uuid-1");
        assert!(out[0].imported);
        assert_eq!(out[0].encoded_cwd, None);
    }

    #[tokio::test]
    async fn missing_root_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let out = discover_sessions(&tmp.path().join("nope"), &tmp.path().join("nope2"))
            .await
            .unwrap();
        assert!(out.is_empty());
    }
}
