use crate::privacy::PrivacyGuard;
use common::{KlyntbotError, ToolError};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsResolveError {
    #[error("path {path:?} is outside session cwd {cwd:?}")]
    OutsideCwd { path: PathBuf, cwd: PathBuf },
    #[error("privacy guard denied {path:?}")]
    PrivacyDenied { path: PathBuf },
}

fn canonicalize_cwd(cwd: &Path) -> PathBuf {
    cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf())
}

/// Expand `~`, resolve relative paths against cwd, and canonicalize
/// ancestors so symlinked prefixes match (macOS `/tmp` → `/private/tmp`).
/// Does **not** enforce cwd restriction or privacy checks — use
/// [`resolve_under_cwd`] for that.
pub fn resolve_path(raw: &str, cwd: &Path) -> PathBuf {
    let expanded = shellexpand::tilde(raw).into_owned();
    let cwd_canonical = canonicalize_cwd(cwd);
    let candidate = if Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        cwd_canonical.join(&expanded)
    };
    // Try canonicalize; if the file (or any ancestor) doesn't exist, walk up
    // until we find an ancestor that does, then re-append the trailing
    // components so symlinked prefixes still match.
    candidate.canonicalize().unwrap_or_else(|_| {
        let mut suffix: Vec<&std::ffi::OsStr> = Vec::new();
        let mut cursor = candidate.as_path();
        loop {
            if let Ok(c) = cursor.canonicalize() {
                let mut out = c;
                for part in suffix.iter().rev() {
                    out.push(part);
                }
                return out;
            }
            match (cursor.parent(), cursor.file_name()) {
                (Some(p), Some(n)) if p != cursor => {
                    suffix.push(n);
                    cursor = p;
                }
                _ => return candidate.clone(),
            }
        }
    })
}

/// Expand `~`, resolve relative paths against cwd, ensure result is inside cwd,
/// and check it's not in the privacy exclude list.
pub fn resolve_under_cwd(
    raw: &str,
    cwd: &Path,
    privacy: &PrivacyGuard,
) -> Result<PathBuf, FsResolveError> {
    let resolved = resolve_path(raw, cwd);
    let cwd_canonical = canonicalize_cwd(cwd);

    // cwd-restriction
    if !resolved.starts_with(&cwd_canonical) {
        return Err(FsResolveError::OutsideCwd {
            path: resolved,
            cwd: cwd_canonical,
        });
    }
    // Privacy
    if privacy.is_excluded(&resolved) {
        return Err(FsResolveError::PrivacyDenied { path: resolved });
    }
    Ok(resolved)
}

/// Ensure the parent directory of `path` exists, creating it if necessary.
/// Returns a tool error with a descriptive message on failure.
pub async fn ensure_parent_dir(path: &Path) -> Result<(), KlyntbotError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                KlyntbotError::Tool(ToolError::ExecutionFailed(format!(
                    "failed to create parent directory {}: {}",
                    parent.display(),
                    e
                )))
            })?;
        }
    }
    Ok(())
}
