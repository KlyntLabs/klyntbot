//! Log + ahead/behind summary. Output respects the frontend's display order
//! (newest commits first) and uses `%s` for short summaries — full bodies
//! are not surfaced through this endpoint by design (the frontend has a
//! separate "show commit" path).

use std::path::Path;

use crate::cmd::run_git;
use crate::errors::GitToolingError;
use crate::types::{GitLogEntry, GitLogResponse};

/// Top-N commits on HEAD plus ahead/behind entries against the configured
/// upstream (if any). When no upstream is set, ahead/behind are 0 and their
/// entry lists empty.
pub async fn collect(repo: &Path, limit: u32) -> Result<GitLogResponse, GitToolingError> {
    let limit = limit.clamp(1, 1000);
    let entries = log_range(repo, "HEAD", Some(limit))
        .await
        .unwrap_or_default();
    let total = count_commits(repo, "HEAD").await.unwrap_or(0);

    let upstream = upstream_ref(repo).await.ok().flatten();
    let (ahead, ahead_entries, behind, behind_entries) = match upstream.as_deref() {
        Some(up) => {
            let ahead_entries = log_range(repo, &format!("{up}..HEAD"), Some(50))
                .await
                .unwrap_or_default();
            let behind_entries = log_range(repo, &format!("HEAD..{up}"), Some(50))
                .await
                .unwrap_or_default();
            let ahead = count_commits(repo, &format!("{up}..HEAD"))
                .await
                .unwrap_or(0);
            let behind = count_commits(repo, &format!("HEAD..{up}"))
                .await
                .unwrap_or(0);
            (ahead, ahead_entries, behind, behind_entries)
        }
        None => (0, Vec::new(), 0, Vec::new()),
    };

    Ok(GitLogResponse {
        total,
        entries,
        ahead,
        behind,
        ahead_entries,
        behind_entries,
        upstream,
    })
}

/// Return the upstream ref name (e.g. `origin/main`) if HEAD has one configured.
pub async fn upstream_ref(repo: &Path) -> Result<Option<String>, GitToolingError> {
    match run_git(repo, &["rev-parse", "--abbrev-ref", "@{upstream}"]).await {
        Ok(s) => {
            let trimmed = s.trim();
            Ok(if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            })
        }
        // No upstream configured — git exits non-zero. Treat as `None`,
        // *not* an error.
        Err(_) => Ok(None),
    }
}

async fn log_range(
    repo: &Path,
    rev_range: &str,
    max: Option<u32>,
) -> Result<Vec<GitLogEntry>, GitToolingError> {
    // Format: `<sha>\x1f<unix-ts>\x1f<author>\x1f<summary>\x1e`
    // Using ASCII record/field separators avoids collisions with any author
    // name or commit message containing tabs/newlines.
    let format = "%H\x1f%at\x1f%an\x1f%s\x1e";
    let mut args: Vec<String> = vec!["log".into(), format!("--pretty=format:{format}")];
    if let Some(n) = max {
        args.push(format!("-n{n}"));
    }
    args.push(rev_range.to_string());
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let raw = run_git(repo, &arg_refs).await?;
    Ok(parse_log_records(&raw))
}

async fn count_commits(repo: &Path, rev_range: &str) -> Result<u32, GitToolingError> {
    let raw = run_git(repo, &["rev-list", "--count", rev_range]).await?;
    Ok(raw.trim().parse::<u32>().unwrap_or(0))
}

fn parse_log_records(raw: &str) -> Vec<GitLogEntry> {
    let mut out = Vec::new();
    for record in raw.split('\x1e') {
        let record = record.trim_start_matches('\n');
        if record.is_empty() {
            continue;
        }
        let mut cols = record.splitn(4, '\x1f');
        let sha = cols.next().unwrap_or("").to_string();
        let ts = cols.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        let author = cols.next().unwrap_or("").to_string();
        let summary = cols.next().unwrap_or("").trim().to_string();
        if sha.is_empty() {
            continue;
        }
        out.push(GitLogEntry {
            sha,
            summary,
            author,
            timestamp: ts,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_records_handles_two_commits() {
        let raw = "abc\x1f1700000000\x1fAlice\x1ffirst\x1edef\x1f1700000100\x1fBob\x1fsecond\x1e";
        let entries = parse_log_records(raw);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sha, "abc");
        assert_eq!(entries[0].author, "Alice");
        assert_eq!(entries[1].timestamp, 1_700_000_100);
    }

    #[test]
    fn parse_log_records_skips_empty_records() {
        let raw = "\x1eabc\x1f1\x1fA\x1fmsg\x1e\x1e";
        let entries = parse_log_records(raw);
        assert_eq!(entries.len(), 1);
    }
}
