use crate::privacy::PrivacyGuard;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsResolveError {
    #[error("path {path:?} is outside session cwd {cwd:?}")]
    OutsideCwd { path: PathBuf, cwd: PathBuf },
    #[error("privacy guard denied {path:?}")]
    PrivacyDenied { path: PathBuf },
}

/// Expand `~`, resolve relative paths against cwd, ensure result is inside cwd,
/// and check it's not in the privacy exclude list.
pub fn resolve_under_cwd(
    raw: &str,
    cwd: &Path,
    privacy: &PrivacyGuard,
) -> Result<PathBuf, FsResolveError> {
    let expanded = shellexpand::tilde(raw).into_owned();
    let candidate = if Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        cwd.join(&expanded)
    };
    let resolved = candidate.canonicalize().unwrap_or(candidate);

    // cwd-restriction
    let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    if !resolved.starts_with(&cwd_canonical) {
        return Err(FsResolveError::OutsideCwd { path: resolved, cwd: cwd_canonical });
    }
    // Privacy
    if privacy.is_excluded(&resolved) {
        return Err(FsResolveError::PrivacyDenied { path: resolved });
    }
    Ok(resolved)
}
