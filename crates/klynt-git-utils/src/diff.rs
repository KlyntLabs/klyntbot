//! Working-tree and commit diffs. The frontend's `GitDiffViewer` parses the
//! unified-diff string for hunks; we additionally surface `oldLines`/`newLines`
//! arrays for binary-or-image cases where the viewer falls back to a side-by-
//! side render. Image bytes are inlined as base64 with the appropriate MIME so
//! the renderer can do `<img src="data:...">` without a second IPC.

use std::path::Path;

use base64::Engine;

use crate::cmd::{run_git, run_git_bytes};
use crate::errors::GitToolingError;
use crate::types::{GitCommitDiff, GitFileDiff};

/// All working-tree changes (staged + unstaged) for `repo`. One `GitFileDiff` per
/// changed path; renamed files appear under their new path with the rename
/// header present in the unified diff payload.
pub async fn working_tree(repo: &Path) -> Result<Vec<GitFileDiff>, GitToolingError> {
    // `git diff HEAD` covers both staged and unstaged in one call. When the
    // repo has no commits yet, fall back to `--cached`/empty-tree.
    let head_exists = run_git(repo, &["rev-parse", "--verify", "HEAD"])
        .await
        .is_ok();
    let combined = if head_exists {
        run_git(repo, &["diff", "HEAD"]).await?
    } else {
        run_git(repo, &["diff", "--cached"])
            .await
            .unwrap_or_default()
    };

    // Split on the `diff --git` header. The first chunk before the first
    // header is empty (or noise); skip it.
    let mut diffs = Vec::new();
    for chunk in split_unified_diff(&combined) {
        let path = match parse_diff_path(&chunk) {
            Some(p) => p,
            None => continue,
        };
        let (is_binary, is_image) = classify_path(&path, &chunk);
        let mut entry = GitFileDiff {
            path: path.clone(),
            diff: chunk.clone(),
            old_lines: None,
            new_lines: None,
            is_binary: is_binary.then_some(true),
            is_image: is_image.then_some(true),
            old_image_data: None,
            new_image_data: None,
            old_image_mime: None,
            new_image_mime: None,
        };
        if is_image {
            // Try to inline both sides — old from HEAD, new from the
            // working tree. Either may be missing for adds/deletes.
            if head_exists {
                entry.old_image_data = read_blob_at_revision(repo, "HEAD", &path)
                    .await
                    .ok()
                    .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes));
                entry.old_image_mime = mime_for_path(&path);
            }
            entry.new_image_data = read_worktree_bytes(repo, &path)
                .await
                .ok()
                .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes));
            entry.new_image_mime = mime_for_path(&path);
        }
        diffs.push(entry);
    }
    Ok(diffs)
}

/// Per-file diffs for a single commit (`git show <sha>`). Returns one
/// `GitCommitDiff` per file changed by the commit, with the same
/// binary/image surfacing rules as `working_tree`.
pub async fn for_commit(repo: &Path, sha: &str) -> Result<Vec<GitCommitDiff>, GitToolingError> {
    if sha.is_empty() {
        return Err(GitToolingError::GitCommand {
            command: "git show".into(),
            status: std::process::ExitStatus::default(),
            stderr: "empty sha".into(),
        });
    }
    let raw = run_git(repo, &["show", "--patch", "--format=", sha]).await?;
    let raw_status = run_git(repo, &["show", "--name-status", "--format=", sha])
        .await
        .unwrap_or_default();
    let status_table = parse_name_status(&raw_status);

    let mut out = Vec::new();
    for chunk in split_unified_diff(&raw) {
        let path = match parse_diff_path(&chunk) {
            Some(p) => p,
            None => continue,
        };
        let (is_binary, is_image) = classify_path(&path, &chunk);
        let status = status_table
            .iter()
            .find(|(_, p)| p == &path)
            .map(|(s, _)| s.clone())
            .unwrap_or_else(|| "M".to_string());
        let mut entry = GitCommitDiff {
            path: path.clone(),
            status,
            diff: chunk.clone(),
            old_lines: None,
            new_lines: None,
            is_binary: is_binary.then_some(true),
            is_image: is_image.then_some(true),
            old_image_data: None,
            new_image_data: None,
            old_image_mime: None,
            new_image_mime: None,
        };
        if is_image {
            // Old = parent commit's blob, new = this commit's blob. Both via
            // `git show <ref>:<path>`; either may be missing for add/delete.
            entry.old_image_data = read_blob_at_revision(repo, &format!("{sha}^"), &path)
                .await
                .ok()
                .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes));
            entry.old_image_mime = mime_for_path(&path);
            entry.new_image_data = read_blob_at_revision(repo, sha, &path)
                .await
                .ok()
                .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes));
            entry.new_image_mime = mime_for_path(&path);
        }
        out.push(entry);
    }
    Ok(out)
}

/// Split a multi-file unified diff into one chunk per file, each beginning
/// with `diff --git ...`. Preserves all headers and trailing newlines.
fn split_unified_diff(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in s.split_inclusive('\n') {
        if line.starts_with("diff --git ") && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Pull the `b/<path>` from a `diff --git a/foo b/foo` header — the new path
/// is what the frontend keys on. Falls back to the `a/` side when missing
/// (rare; happens for pure deletions in some git versions).
fn parse_diff_path(chunk: &str) -> Option<String> {
    let first_line = chunk.lines().next()?;
    // Format: `diff --git a/<old> b/<new>`
    let rest = first_line.strip_prefix("diff --git ")?;
    let mut tokens = rest.split_whitespace();
    let _a = tokens.next()?;
    let b = tokens.next()?;
    let path = b.strip_prefix("b/").unwrap_or(b);
    Some(path.to_string())
}

/// Classify a diff chunk: returns `(is_binary, is_image)`. Binary detection
/// is based on the diff header (`Binary files … differ` line); image is
/// inferred from the file extension once we know it's binary or even when
/// it's text (a tracked SVG with an inline diff is still an "image" for
/// preview purposes, but we don't inline-render text — frontend handles).
fn classify_path(path: &str, chunk: &str) -> (bool, bool) {
    let is_binary = chunk.contains("Binary files ") && chunk.contains(" differ");
    let is_image = mime_for_path(path).is_some();
    (is_binary || is_image, is_image)
}

fn mime_for_path(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "tiff" | "tif" => "image/tiff",
        "heic" => "image/heic",
        "heif" => "image/heif",
        _ => return None,
    };
    Some(mime.to_string())
}

async fn read_blob_at_revision(
    repo: &Path,
    rev: &str,
    path: &str,
) -> Result<Vec<u8>, GitToolingError> {
    run_git_bytes(repo, &["show", &format!("{rev}:{path}")]).await
}

async fn read_worktree_bytes(repo: &Path, path: &str) -> Result<Vec<u8>, GitToolingError> {
    let abs = repo.join(path);
    Ok(tokio::fs::read(&abs).await?)
}

fn parse_name_status(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut cols = line.splitn(2, '\t');
        let status = cols.next().unwrap_or("M").to_string();
        let path = cols.next().unwrap_or("").to_string();
        // Rename status looks like `R100\told\tnew` — split into 3 cols and
        // take the third as the path.
        if status.starts_with('R') || status.starts_with('C') {
            let mut parts = line.splitn(3, '\t');
            let s = parts.next().unwrap_or("M").to_string();
            let _old = parts.next();
            let new = parts.next().unwrap_or("").to_string();
            out.push((s.chars().next().unwrap_or('M').to_string(), new));
        } else {
            out.push((status, path));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_unified_diff_handles_multifile() {
        let s = "diff --git a/a b/a\nfoo\ndiff --git a/b b/b\nbar\n";
        let parts = split_unified_diff(s);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].starts_with("diff --git a/a"));
        assert!(parts[1].starts_with("diff --git a/b"));
    }

    #[test]
    fn parse_diff_path_extracts_b_side() {
        assert_eq!(
            parse_diff_path("diff --git a/old.rs b/new.rs\nrest\n"),
            Some("new.rs".into())
        );
    }

    #[test]
    fn classify_marks_image_extension() {
        let (binary, image) = classify_path("logo.png", "diff --git a/logo.png b/logo.png\n");
        assert!(binary);
        assert!(image);
    }

    #[test]
    fn classify_marks_binary_without_image() {
        let (binary, image) = classify_path(
            "data.bin",
            "diff --git a/data.bin b/data.bin\nBinary files a/data.bin and b/data.bin differ\n",
        );
        assert!(binary);
        assert!(!image);
    }

    #[test]
    fn name_status_parses_rename() {
        let raw = "R092\told.rs\tnew.rs\nM\tother.rs\n";
        let table = parse_name_status(raw);
        assert_eq!(table[0], ("R".into(), "new.rs".into()));
        assert_eq!(table[1], ("M".into(), "other.rs".into()));
    }
}
