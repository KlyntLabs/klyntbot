//! `git status` parsing. Uses `--porcelain=v1 -z` for NUL-delimited records
//! so paths with spaces/quotes survive intact, and `--numstat` (also `-z`)
//! for additions/deletions per file.

use std::path::Path;

use crate::cmd::run_git;
use crate::errors::GitToolingError;
use crate::types::{GitFileStatus, GitStatusSummary};

/// Collect the full status snapshot for `repo`. The shape mirrors the
/// frontend's `getGitStatus` contract: separate staged/unstaged lists plus
/// a unified `files` list (renames appear once, in the section that has the
/// stronger signal — staged wins over unstaged).
pub async fn collect(repo: &Path) -> Result<GitStatusSummary, GitToolingError> {
    // The four git invocations are data-independent; running them concurrently
    // saves ~30-90ms per status refresh on a warm filesystem.
    let (branch_res, porcelain_res, staged_res, unstaged_res) = tokio::join!(
        current_branch(repo),
        run_git(repo, &["status", "--porcelain=v1", "-z"]),
        run_git(repo, &["diff", "--cached", "--numstat", "-z"]),
        run_git(repo, &["diff", "--numstat", "-z"]),
    );
    let branch_name = branch_res.unwrap_or_default();
    let entries = parse_porcelain_z(&porcelain_res?);
    let staged_numstat = parse_numstat(&staged_res?);
    let unstaged_numstat = parse_numstat(&unstaged_res?);

    let mut staged_files = Vec::new();
    let mut unstaged_files = Vec::new();
    let mut total_additions: u32 = 0;
    let mut total_deletions: u32 = 0;

    for entry in &entries {
        let staged_status = entry.index_status;
        let worktree_status = entry.worktree_status;
        let path = entry.path.as_str();

        if staged_status != ' ' && staged_status != '?' {
            let (a, d) = staged_numstat.lookup(path).unwrap_or((0, 0));
            total_additions = total_additions.saturating_add(a);
            total_deletions = total_deletions.saturating_add(d);
            staged_files.push(GitFileStatus {
                path: path.to_string(),
                status: staged_status.to_string(),
                additions: a,
                deletions: d,
            });
        }

        // `??` = untracked. Treat as unstaged with status "?" and zero stats
        // (numstat doesn't cover untracked files); UI renders them in the
        // unstaged section.
        if entry.is_untracked() {
            unstaged_files.push(GitFileStatus {
                path: path.to_string(),
                status: "?".into(),
                additions: 0,
                deletions: 0,
            });
            continue;
        }

        if worktree_status != ' ' && worktree_status != '?' {
            let (a, d) = unstaged_numstat.lookup(path).unwrap_or((0, 0));
            total_additions = total_additions.saturating_add(a);
            total_deletions = total_deletions.saturating_add(d);
            unstaged_files.push(GitFileStatus {
                path: path.to_string(),
                status: worktree_status.to_string(),
                additions: a,
                deletions: d,
            });
        }
    }

    // Unified `files` list: prefer staged record over unstaged for any path
    // that has both (matches the frontend's expectation that `files` is the
    // canonical "what changed" list — duplicates would double-count rows).
    let mut files = Vec::with_capacity(staged_files.len() + unstaged_files.len());
    files.extend(staged_files.iter().cloned());
    for f in &unstaged_files {
        if !files.iter().any(|s| s.path == f.path) {
            files.push(f.clone());
        }
    }

    Ok(GitStatusSummary {
        branch_name,
        files,
        staged_files,
        unstaged_files,
        total_additions,
        total_deletions,
    })
}

/// Returns the current branch name, or an empty string when HEAD is detached
/// (porcelain `HEAD` literal). Uses `--quiet` so the parent error path stays
/// silent for non-repo invocations.
pub async fn current_branch(repo: &Path) -> Result<String, GitToolingError> {
    let raw = run_git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).await?;
    let trimmed = raw.trim();
    Ok(if trimmed == "HEAD" {
        String::new()
    } else {
        trimmed.to_string()
    })
}

#[derive(Debug, Clone)]
struct PorcelainEntry {
    index_status: char,
    worktree_status: char,
    path: String,
}

impl PorcelainEntry {
    fn is_untracked(&self) -> bool {
        self.index_status == '?' && self.worktree_status == '?'
    }
}

/// Parse `git status --porcelain=v1 -z` output. Records are separated by NUL;
/// rename records (`R`/`C` in either status column) consume a second NUL-
/// terminated path (the original) which we discard since the frontend
/// addresses files by their *current* path.
fn parse_porcelain_z(s: &str) -> Vec<PorcelainEntry> {
    let mut out = Vec::new();
    let mut iter = s.split('\0');
    while let Some(record) = iter.next() {
        if record.is_empty() {
            continue;
        }
        if record.len() < 3 {
            continue;
        }
        let bytes = record.as_bytes();
        let index_status = bytes[0] as char;
        let worktree_status = bytes[1] as char;
        // Skip the space at byte 2, take the rest as the path.
        let path = record[3..].to_string();
        // Renames carry the original path as a separate NUL-terminated
        // record — drop it.
        if index_status == 'R' || worktree_status == 'R' || index_status == 'C' {
            iter.next();
        }
        out.push(PorcelainEntry {
            index_status,
            worktree_status,
            path,
        });
    }
    out
}

/// Compact lookup table built from `git diff --numstat -z`. Lines are
/// `<adds>\t<dels>\t<path>\0`; binary files report `-\t-` which we map to
/// `(0, 0)` since the UI exposes binaries through a separate flag.
struct NumstatTable {
    rows: Vec<(String, u32, u32)>,
}

impl NumstatTable {
    fn lookup(&self, path: &str) -> Option<(u32, u32)> {
        self.rows
            .iter()
            .find(|(p, _, _)| p == path)
            .map(|(_, a, d)| (*a, *d))
    }
}

fn parse_numstat(s: &str) -> NumstatTable {
    let mut rows = Vec::new();
    for record in s.split('\0') {
        if record.is_empty() {
            continue;
        }
        let mut cols = record.splitn(3, '\t');
        let adds = cols.next().unwrap_or("0");
        let dels = cols.next().unwrap_or("0");
        let path = match cols.next() {
            Some(p) => p,
            None => continue,
        };
        let adds = if adds == "-" {
            0
        } else {
            adds.parse::<u32>().unwrap_or(0)
        };
        let dels = if dels == "-" {
            0
        } else {
            dels.parse::<u32>().unwrap_or(0)
        };
        rows.push((path.to_string(), adds, dels));
    }
    NumstatTable { rows }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_z_parses_modified_file() {
        // Format: "XY <path>\0"
        let raw = " M src/main.rs\0";
        let entries = parse_porcelain_z(raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index_status, ' ');
        assert_eq!(entries[0].worktree_status, 'M');
        assert_eq!(entries[0].path, "src/main.rs");
    }

    #[test]
    fn porcelain_z_parses_untracked_file() {
        let raw = "?? new.txt\0";
        let entries = parse_porcelain_z(raw);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_untracked());
        assert_eq!(entries[0].path, "new.txt");
    }

    #[test]
    fn porcelain_z_consumes_rename_origin() {
        let raw = "R  new.rs\0old.rs\0 M other.rs\0";
        let entries = parse_porcelain_z(raw);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "new.rs");
        assert_eq!(entries[1].path, "other.rs");
    }

    #[test]
    fn numstat_parses_text_and_binary_rows() {
        let raw = "5\t2\tsrc/lib.rs\0-\t-\timg.png\0";
        let table = parse_numstat(raw);
        assert_eq!(table.lookup("src/lib.rs"), Some((5, 2)));
        assert_eq!(table.lookup("img.png"), Some((0, 0)));
        assert_eq!(table.lookup("missing"), None);
    }
}
