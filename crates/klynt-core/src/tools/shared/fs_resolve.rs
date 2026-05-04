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
    // Canonicalize cwd up front so symlinked prefixes (macOS /tmp → /private/tmp)
    // don't cause false `starts_with` mismatches against the resolved candidate.
    let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let candidate = if Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        cwd_canonical.join(&expanded)
    };
    // Try canonicalize; if the file (or any ancestor) doesn't exist, walk up
    // until we find an ancestor that does, then re-append the trailing
    // components so symlinked prefixes (macOS /tmp → /private/tmp) still match.
    let resolved = candidate.canonicalize().unwrap_or_else(|_| {
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
    });

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
