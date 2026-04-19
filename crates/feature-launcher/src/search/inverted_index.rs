use crate::types::FileKind;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::path::PathBuf;

const MAX_PREFIX_LEN: usize = 12;

#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: FileKind,
    pub depth: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct ScoredEntry {
    pub entry_idx: u32,
    pub score: i32,
}

pub struct InvertedFileIndex {
    pub(crate) entries: Vec<IndexEntry>,
    pub(crate) postings: HashMap<SmolStr, Vec<u32>>,
}

impl InvertedFileIndex {
    pub fn empty() -> Self {
        Self { entries: Vec::new(), postings: HashMap::new() }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Tokenize a name or path component on whitespace, dot, dash, underscore, slash.
pub(crate) fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| c.is_whitespace() || matches!(c, '.' | '-' | '_' | '/'))
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// Generate prefix keys for a token: "downloads" → ["d","do","dow",...,"downloads"] capped at MAX_PREFIX_LEN.
pub(crate) fn prefixes(token: &str) -> impl Iterator<Item = SmolStr> + '_ {
    let max = token.len().min(MAX_PREFIX_LEN);
    (1..=max).map(move |n| SmolStr::new(&token[..n]))
}

use ignore::WalkBuilder;
use std::path::Path;

const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "__pycache__", ".cache", ".Trash", "Library",
];
const SKIP_EXTENSIONS: &[&str] = &["pyc", "pyo", "class", "o", "obj", "dylib", "so"];

#[derive(Default, Clone)]
pub struct SkipSet {
    pub dirs: Vec<&'static str>,
    pub exts: Vec<&'static str>,
}

impl SkipSet {
    pub fn defaults() -> Self {
        Self { dirs: SKIP_DIRS.to_vec(), exts: SKIP_EXTENSIONS.to_vec() }
    }
    fn skip_dir(&self, name: &str) -> bool { self.dirs.contains(&name) }
    fn skip_ext(&self, ext: &str) -> bool { self.exts.contains(&ext) }
}

fn classify_extension(path: &Path) -> FileKind {
    match path.extension().and_then(|e| e.to_str()) {
        Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "heic") => FileKind::Image,
        Some("pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "pages" | "odt") => FileKind::Document,
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "rb" | "java" | "c" | "cpp"
            | "h" | "swift" | "kt" | "sh" | "toml" | "yaml" | "json" | "html" | "css") => FileKind::Code,
        Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "dmg") => FileKind::Archive,
        _ => FileKind::File,
    }
}

impl InvertedFileIndex {
    pub fn build(roots: &[PathBuf], skip: &SkipSet, max_entries: usize) -> Self {
        let mut entries: Vec<IndexEntry> = Vec::new();
        let mut postings: HashMap<SmolStr, Vec<u32>> = HashMap::new();

        'outer: for root in roots {
            if !root.exists() { continue; }
            let walker = WalkBuilder::new(root)
                .hidden(true)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .require_git(false)
                .filter_entry({
                    let skip = skip.clone();
                    move |entry| {
                        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                            if let Some(name) = entry.file_name().to_str() {
                                return !skip.skip_dir(name);
                            }
                        }
                        true
                    }
                })
                .build();

            for result in walker {
                if entries.len() >= max_entries { break 'outer; }
                let entry = match result {
                    Ok(e) => e,
                    Err(e) => { tracing::debug!("inverted_index walk error: {e}"); continue; }
                };
                if entry.file_type().is_none_or(|ft| ft.is_dir()) { continue; }
                let path = entry.path();
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if skip.skip_ext(ext) { continue; }
                }
                let depth = entry.depth().min(255) as u8;
                let idx = entries.len() as u32;
                entries.push(IndexEntry { path: path.to_path_buf(), name: name.clone(), kind: classify_extension(path), depth });

                // Index name tokens
                for token in tokenize(&name) {
                    for prefix in prefixes(&token) {
                        postings.entry(prefix).or_default().push(idx);
                    }
                }
                // Index path component tokens (ii: from spec Q3)
                for component in path.components().filter_map(|c| c.as_os_str().to_str()) {
                    if component == name { continue; }
                    for token in tokenize(component) {
                        for prefix in prefixes(&token) {
                            postings.entry(prefix).or_default().push(idx);
                        }
                    }
                }
            }
        }

        // Sort + dedupe each posting list (a name + path tokens can yield duplicates)
        for list in postings.values_mut() {
            list.sort_unstable();
            list.dedup();
        }

        Self { entries, postings }
    }
}

fn score_entry_match(query_tokens: &[String], entry: &IndexEntry) -> i32 {
    let name_lower = entry.name.to_lowercase();
    let mut score: i32 = 0;
    for q in query_tokens {
        if name_lower == *q { score += 140; continue; }
        if name_lower.starts_with(q) { score += 118; continue; }
        // compact-prefix: collapse separators in name
        let compact: String = name_lower.chars().filter(|c| c.is_alphanumeric()).collect();
        if compact.starts_with(q) { score += 106; continue; }
        if name_lower.contains(q) { score += 60; continue; }
        // subsequence
        if is_subsequence(q, &name_lower) { score += 44; continue; }
    }
    // depth penalty
    score -= (entry.depth as i32) * 2;
    score
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut h = haystack.chars();
    'outer: for nc in needle.chars() {
        for hc in h.by_ref() {
            if hc == nc { continue 'outer; }
        }
        return false;
    }
    true
}

impl InvertedFileIndex {
    pub fn search(&self, query: &str, limit: usize) -> Vec<ScoredEntry> {
        let q = query.trim();
        if q.is_empty() || self.entries.is_empty() {
            return Vec::new();
        }
        let tokens = tokenize(q);
        if tokens.is_empty() { return Vec::new(); }

        // For each token, get a posting list using its longest available prefix (up to MAX_PREFIX_LEN).
        let mut lists: Vec<&Vec<u32>> = Vec::with_capacity(tokens.len());
        for t in &tokens {
            let key = SmolStr::new(&t[..t.len().min(MAX_PREFIX_LEN)]);
            match self.postings.get(&key) {
                Some(list) => lists.push(list),
                None => return Vec::new(), // missing token → no results
            }
        }

        // Sort lists by length ascending; intersect.
        lists.sort_by_key(|l| l.len());
        let mut candidates: Vec<u32> = lists[0].clone();
        for list in &lists[1..] {
            candidates.retain(|idx| list.binary_search(idx).is_ok());
            if candidates.is_empty() { return Vec::new(); }
        }

        // Score
        let mut scored: Vec<ScoredEntry> = candidates.iter().map(|&idx| {
            let entry = &self.entries[idx as usize];
            ScoredEntry { entry_idx: idx, score: score_entry_match(&tokens, entry) }
        }).collect();
        scored.sort_by(|a, b| {
            b.score.cmp(&a.score).then_with(|| {
                let a_len = self.entries[a.entry_idx as usize].name.len();
                let b_len = self.entries[b.entry_idx as usize].name.len();
                a_len.cmp(&b_len)
            })
        });
        scored.retain(|s| !self.entries[s.entry_idx as usize].name.is_empty());
        scored.truncate(limit);
        scored
    }
}

impl InvertedFileIndex {
    /// Add a single file to the index. Idempotent — duplicate paths are skipped.
    pub fn apply_event_create(&mut self, path: &Path) {
        if !path.is_file() { return; }
        // Skip if already indexed
        if self.entries.iter().any(|e| e.path == path) { return; }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => return,
        };
        let depth = path.components().count().min(255) as u8;
        let idx = self.entries.len() as u32;
        self.entries.push(IndexEntry {
            path: path.to_path_buf(),
            name: name.clone(),
            kind: classify_extension(path),
            depth,
        });
        for token in tokenize(&name) {
            for prefix in prefixes(&token) {
                let list = self.postings.entry(prefix).or_default();
                list.push(idx);
                list.sort_unstable();
                list.dedup();
            }
        }
        for component in path.components().filter_map(|c| c.as_os_str().to_str()) {
            if component == name { continue; }
            for token in tokenize(component) {
                for prefix in prefixes(&token) {
                    let list = self.postings.entry(prefix).or_default();
                    list.push(idx);
                    list.sort_unstable();
                    list.dedup();
                }
            }
        }
    }

    /// Remove a path from the index. Tombstones the entry (we don't compact mid-flight).
    pub fn apply_event_remove(&mut self, path: &Path) {
        let target = self.entries.iter().position(|e| e.path == path);
        let target = match target { Some(t) => t as u32, None => return };
        // Drop the entry's name → blank; remove from all postings.
        // Cheap approach: scan all postings, drop this idx.
        for list in self.postings.values_mut() {
            list.retain(|&i| i != target);
        }
        // Mark entry as empty (keep slot to preserve indices).
        self.entries[target as usize].name.clear();
        self.entries[target as usize].path = PathBuf::new();
    }

    /// Rename = remove + create.
    pub fn apply_event_rename(&mut self, from: &Path, to: &Path) {
        self.apply_event_remove(from);
        self.apply_event_create(to);
    }
}

#[cfg(target_os = "macos")]
pub async fn mdfind_paths(
    query: &str,
    roots: &[PathBuf],
    skip: &SkipSet,
    cancel: tokio_util::sync::CancellationToken,
) -> Vec<PathBuf> {
    let tokens = tokenize(query);
    if tokens.is_empty() { return Vec::new(); }
    let mut handles = Vec::new();
    for root in roots {
        let root = root.clone();
        let q = query.to_string();
        let cancel = cancel.clone();
        handles.push(tokio::spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => Vec::<PathBuf>::new(),
                r = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    tokio::process::Command::new("/usr/bin/mdfind")
                        .arg("-onlyin").arg(&root)
                        .arg(&q)
                        .output(),
                ) => match r {
                    Ok(Ok(out)) if out.status.success() =>
                        String::from_utf8_lossy(&out.stdout)
                            .lines().map(|l| PathBuf::from(l.trim())).collect(),
                    _ => Vec::new(),
                },
            }
        }));
    }
    let mut paths = Vec::new();
    for h in handles { if let Ok(ps) = h.await { paths.extend(ps); } }
    paths.retain(|p| {
        let in_skip = p.components().any(|c|
            c.as_os_str().to_str().is_some_and(|n| skip.skip_dir(n))
        );
        !in_skip
    });
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_splits_on_separators() {
        assert_eq!(tokenize("hello-world.rs"), vec!["hello", "world", "rs"]);
        assert_eq!(tokenize("My Document_v2.pdf"), vec!["my", "document", "v2", "pdf"]);
        assert_eq!(tokenize("/home/user/notes/idea.md"), vec!["home", "user", "notes", "idea", "md"]);
    }

    #[test]
    fn tokenize_skips_empty() {
        assert_eq!(tokenize("..."), Vec::<String>::new());
        assert_eq!(tokenize(""), Vec::<String>::new());
    }

    #[test]
    fn prefixes_caps_at_max_len() {
        let p: Vec<_> = prefixes("documentation").collect();
        assert_eq!(p.len(), 12);
        assert_eq!(p[0].as_str(), "d");
        assert_eq!(p[11].as_str(), "documentatio");
    }

    #[test]
    fn prefixes_short_token() {
        let p: Vec<_> = prefixes("rs").collect();
        assert_eq!(p, vec![SmolStr::new("r"), SmolStr::new("rs")]);
    }

    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn build_indexes_filenames_and_path_components() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("hello.rs"), "").unwrap();
        fs::create_dir_all(base.join("downloads")).unwrap();
        fs::write(base.join("downloads/invoice.pdf"), "").unwrap();

        let idx = InvertedFileIndex::build(
            &[base.to_path_buf()],
            &SkipSet::default(),
            1_000_000,
        );
        assert_eq!(idx.len(), 2);
        // "downloads" should be in postings (path component indexing)
        assert!(idx.postings.contains_key(&SmolStr::new("dow")));
        // "hello" prefix should be there too
        assert!(idx.postings.contains_key(&SmolStr::new("hel")));
    }

    #[test]
    fn build_respects_cap() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        for i in 0..50 {
            fs::write(base.join(format!("file{i}.txt")), "").unwrap();
        }
        let idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::default(), 10);
        assert_eq!(idx.len(), 10);
    }

    #[test]
    fn search_intersects_multiple_terms() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::create_dir_all(base.join("downloads")).unwrap();
        fs::write(base.join("downloads/invoice.pdf"), "").unwrap();
        fs::write(base.join("downloads/photo.png"), "").unwrap();
        fs::write(base.join("invoice.txt"), "").unwrap();

        let idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);
        // "downloads invoice" should match only the pdf, not the txt or photo
        let results = idx.search("downloads invoice", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(idx.entries[results[0].entry_idx as usize].name, "invoice.pdf");
    }

    #[test]
    fn search_exact_match_scores_higher() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("hello.rs"), "").unwrap();
        fs::write(base.join("hellothere.rs"), "").unwrap();
        let idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);
        let results = idx.search("hello", 10);
        assert_eq!(idx.entries[results[0].entry_idx as usize].name, "hello.rs");
    }

    #[test]
    fn search_empty_query_returns_empty() {
        let idx = InvertedFileIndex::empty();
        assert!(idx.search("", 10).is_empty());
    }

    #[test]
    fn apply_events_adds_new_file() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        let mut idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);
        assert_eq!(idx.len(), 0);

        let new_path = base.join("newfile.rs");
        fs::write(&new_path, "").unwrap();
        idx.apply_event_create(&new_path);
        assert_eq!(idx.len(), 1);
        assert!(!idx.search("newfile", 10).is_empty());
    }

    #[test]
    fn apply_events_removes_file() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        let p = base.join("doomed.txt");
        fs::write(&p, "").unwrap();
        let mut idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);
        assert_eq!(idx.len(), 1);

        idx.apply_event_remove(&p);
        assert!(idx.search("doomed", 10).is_empty());
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn mdfind_paths_returns_empty_for_unmatched_query() {
        let cancel = tokio_util::sync::CancellationToken::new();
        let paths = mdfind_paths(
            "klyntbot_no_such_unique_marker_xyz_12345",
            &[std::env::temp_dir()],
            &SkipSet::defaults(),
            cancel,
        ).await;
        assert!(paths.is_empty());
    }
}
