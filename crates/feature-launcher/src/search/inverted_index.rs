use crate::types::FileKind;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;

use std::path::PathBuf;

use super::token_index::TokenIndex;

#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: FileKind,
    pub depth: u8,
    pub name_lc: SmolStr,
    pub name_compact: SmolStr,
}

#[derive(Debug, Clone, Copy)]
pub struct ScoredEntry {
    pub entry_idx: u32,
    pub score: i32,
}

#[derive(Clone)]
pub struct InvertedFileIndex {
    pub(crate) entries: Vec<IndexEntry>,
    pub(crate) token_index: TokenIndex,
    /// Fast path-to-index lookup for incremental updates.
    pub(crate) path_to_idx: FxHashMap<PathBuf, u32>,
}

impl InvertedFileIndex {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            token_index: TokenIndex::build(vec![]),
            path_to_idx: FxHashMap::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entry_name(&self, idx: u32) -> Option<&str> {
        self.entries.get(idx as usize).map(|e| e.name.as_str())
    }

    pub fn clone_for_patch(&self) -> Self {
        self.clone()
    }
}

fn make_name_lc(name: &str) -> SmolStr {
    if name.bytes().all(|b| !b.is_ascii_uppercase()) {
        SmolStr::new(name)
    } else {
        SmolStr::new(name.to_lowercase())
    }
}

fn make_name_compact(name_lc: &str) -> SmolStr {
    if name_lc.bytes().all(|b| b.is_ascii_alphanumeric()) {
        SmolStr::new(name_lc)
    } else {
        SmolStr::new(
            name_lc
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>(),
        )
    }
}

/// Tokenize a name or path component on whitespace, dot, dash, underscore, slash.
pub(crate) fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| c.is_whitespace() || matches!(c, '.' | '-' | '_' | '/'))
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

use ignore::WalkBuilder;
use std::path::Path;

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "__pycache__",
    ".cache",
    ".Trash",
    "Library",
];
const SKIP_EXTENSIONS: &[&str] = &["pyc", "pyo", "class", "o", "obj", "dylib", "so"];

#[derive(Default, Clone)]
pub struct SkipSet {
    pub dirs: Vec<&'static str>,
    pub exts: Vec<&'static str>,
}

impl SkipSet {
    pub fn defaults() -> Self {
        Self {
            dirs: SKIP_DIRS.to_vec(),
            exts: SKIP_EXTENSIONS.to_vec(),
        }
    }
    fn skip_dir(&self, name: &str) -> bool {
        self.dirs.contains(&name)
    }
    fn skip_ext(&self, ext: &str) -> bool {
        self.exts.contains(&ext)
    }
}

pub(crate) fn classify_extension(path: &Path) -> FileKind {
    match path.extension().and_then(|e| e.to_str()) {
        Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "heic") => FileKind::Image,
        Some("pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "pages" | "odt") => FileKind::Document,
        Some(
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "rb" | "java" | "c" | "cpp" | "h"
            | "swift" | "kt" | "sh" | "toml" | "yaml" | "json" | "html" | "css",
        ) => FileKind::Code,
        Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "dmg") => FileKind::Archive,
        _ => FileKind::File,
    }
}

impl InvertedFileIndex {
    pub fn build(roots: &[PathBuf], skip: &SkipSet, max_entries: usize) -> Self {
        let mut entries: Vec<IndexEntry> = Vec::new();
        let mut token_map: FxHashMap<SmolStr, roaring::RoaringBitmap> = FxHashMap::default();

        'outer: for root in roots {
            if !root.exists() {
                continue;
            }
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
                if entries.len() >= max_entries {
                    break 'outer;
                }
                let entry = match result {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::debug!("inverted_index walk error: {e}");
                        continue;
                    }
                };
                if entry.file_type().is_none_or(|ft| ft.is_dir()) {
                    continue;
                }
                let path = entry.path();
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if skip.skip_ext(ext) {
                        continue;
                    }
                }
                let depth = entry.depth().min(255) as u8;
                let idx = entries.len() as u32;
                let name_lc = make_name_lc(&name);
                let name_compact = make_name_compact(&name_lc);
                entries.push(IndexEntry {
                    path: path.to_path_buf(),
                    name: name.clone(),
                    kind: classify_extension(path),
                    depth,
                    name_lc,
                    name_compact,
                });

                Self::index_entry_tokens(&mut token_map, idx, &name, path);
            }
        }

        let n = entries.len() as f32;
        let tokens: Vec<(SmolStr, roaring::RoaringBitmap, f32)> = token_map
            .into_iter()
            .map(|(token, bitmap)| {
                let df = bitmap.len() as f32;
                let w = ((n - df + 0.5) / (df + 0.5) + 1.0).ln().max(0.05);
                (token, bitmap, w)
            })
            .collect();

        let token_index = TokenIndex::build(tokens);

        let mut path_to_idx =
            FxHashMap::with_capacity_and_hasher(entries.len(), Default::default());
        for (i, e) in entries.iter().enumerate() {
            path_to_idx.insert(e.path.clone(), i as u32);
        }

        Self {
            entries,
            token_index,
            path_to_idx,
        }
    }

    fn index_entry_tokens(
        token_map: &mut FxHashMap<SmolStr, roaring::RoaringBitmap>,
        idx: u32,
        name: &str,
        path: &Path,
    ) {
        for token in tokenize(name) {
            token_map
                .entry(SmolStr::new(&token))
                .or_default()
                .insert(idx);
        }
        for component in path.components().filter_map(|c| c.as_os_str().to_str()) {
            if component == name {
                continue;
            }
            for token in tokenize(component) {
                token_map
                    .entry(SmolStr::new(&token))
                    .or_default()
                    .insert(idx);
            }
        }
    }
}

fn score_entry_match(query_tokens: &[(String, f32)], entry: &IndexEntry) -> i32 {
    let name_lc = entry.name_lc.as_str();
    let name_cmp = entry.name_compact.as_str();
    let mut score: i32 = 0;
    for (q, weight) in query_tokens {
        let tier = if name_lc == *q {
            140
        } else if name_lc.starts_with(q) {
            118
        } else if name_cmp.starts_with(q) {
            106
        } else if name_lc.contains(q.as_str()) {
            60
        } else if is_subsequence_fast(q, name_lc) {
            44
        } else {
            0
        };
        score += (tier as f32 * weight) as i32;
    }
    score - (entry.depth as i32) * 2
}

/// Fast subsequence check using byte-level scan for ASCII strings.
/// Falls back to char-level for non-ASCII.
fn is_subsequence_fast(needle: &str, haystack: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    // ASCII fast path: operate on bytes directly.
    if needle.is_ascii() && haystack.is_ascii() {
        let hay = haystack.as_bytes();
        let mut pos = 0;
        for &n in needle.as_bytes() {
            while pos < hay.len() && hay[pos] != n {
                pos += 1;
            }
            if pos >= hay.len() {
                return false;
            }
            pos += 1;
        }
        return true;
    }
    // Unicode fallback.
    let mut h = haystack.chars();
    'outer: for nc in needle.chars() {
        for hc in h.by_ref() {
            if hc == nc {
                continue 'outer;
            }
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
        if tokens.is_empty() {
            return Vec::new();
        }

        // For each token, get candidates and IDF weight.
        let mut per_token: Vec<(std::borrow::Cow<'_, roaring::RoaringBitmap>, f32)> =
            Vec::with_capacity(tokens.len());
        let mut token_weights: Vec<(String, f32)> = Vec::with_capacity(tokens.len());
        for t in &tokens {
            match self.token_index.candidates_for_cow(t) {
                Some((bitmap, idf)) => {
                    per_token.push((bitmap, idf));
                    token_weights.push((t.clone(), idf));
                }
                None => return Vec::new(),
            }
        }

        // Sort by bitmap length ascending; intersect.
        per_token.sort_by_key(|(b, _)| b.len());

        // Single-token fast path: avoid cloning the bitmap.
        if per_token.len() == 1 {
            let (bitmap, _) = per_token.into_iter().next().unwrap();
            return self.score_candidates(bitmap.as_ref(), &token_weights, limit);
        }

        let mut per_token_iter = per_token.into_iter();
        let candidates: roaring::RoaringBitmap = if let Some(first) = per_token_iter.next() {
            let mut c = first.0.into_owned();
            for (bitmap, _) in per_token_iter {
                c &= bitmap.as_ref();
                if c.is_empty() {
                    return Vec::new();
                }
            }
            c
        } else {
            return Vec::new();
        };

        self.score_candidates(&candidates, &token_weights, limit)
    }

    fn score_candidates(
        &self,
        candidates: &roaring::RoaringBitmap,
        token_weights: &[(String, f32)],
        limit: usize,
    ) -> Vec<ScoredEntry> {
        // Score with bounded min-heap.
        let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(i32, i32, u32)>> =
            std::collections::BinaryHeap::with_capacity(limit + 1);
        for idx in candidates.iter() {
            let s = score_entry_match(token_weights, &self.entries[idx as usize]);
            let name_len = self.entries[idx as usize].name.len() as i32;
            if heap.len() < limit {
                heap.push(std::cmp::Reverse((s, -name_len, idx)));
            } else if let Some(&std::cmp::Reverse((worst_score, worst_neg_len, _))) = heap.peek() {
                if s > worst_score || (s == worst_score && -name_len > worst_neg_len) {
                    heap.pop();
                    heap.push(std::cmp::Reverse((s, -name_len, idx)));
                }
            }
        }

        let mut out: Vec<ScoredEntry> = heap
            .into_sorted_vec()
            .into_iter()
            .map(|std::cmp::Reverse((score, _, entry_idx))| ScoredEntry { entry_idx, score })
            .collect();
        out.retain(|s| !self.entries[s.entry_idx as usize].name.is_empty());
        out
    }
}

impl InvertedFileIndex {
    /// Add a single file to the index. Idempotent — duplicate paths are skipped.
    pub fn apply_event_create(&mut self, path: &Path) {
        if !path.is_file() {
            return;
        }
        // Skip if already indexed
        if self.path_to_idx.contains_key(path) {
            return;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => return,
        };
        let depth = path.components().count().min(255) as u8;
        let idx = self.entries.len() as u32;
        let name_lc = make_name_lc(&name);
        let name_compact = make_name_compact(&name_lc);
        self.entries.push(IndexEntry {
            path: path.to_path_buf(),
            name: name.clone(),
            kind: classify_extension(path),
            depth,
            name_lc,
            name_compact,
        });
        self.path_to_idx.insert(path.to_path_buf(), idx);
        for token in tokenize(&name) {
            self.token_index.insert_pending(SmolStr::new(&token), idx);
        }
        for component in path.components().filter_map(|c| c.as_os_str().to_str()) {
            if component == name {
                continue;
            }
            for token in tokenize(component) {
                self.token_index.insert_pending(SmolStr::new(&token), idx);
            }
        }

        if self.token_index.should_compact() {
            self.token_index.compact(self.entries.len());
        }
    }

    /// Remove a path from the index. Tombstones the entry (we don't compact mid-flight).
    pub fn apply_event_remove(&mut self, path: &Path) {
        let target = self.path_to_idx.remove(path);
        let target = match target {
            Some(t) => t,
            None => return,
        };
        // Mark entry as empty (keep slot to preserve indices).
        self.entries[target as usize].name.clear();
        self.entries[target as usize].path = PathBuf::new();
        self.entries[target as usize].name_lc = SmolStr::new("");
        self.entries[target as usize].name_compact = SmolStr::new("");
        self.token_index.remove_doc(target);
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
    if tokens.is_empty() {
        return Vec::new();
    }
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
    for h in handles {
        if let Ok(ps) = h.await {
            paths.extend(ps);
        }
    }
    paths.retain(|p| {
        let in_skip = p
            .components()
            .any(|c| c.as_os_str().to_str().is_some_and(|n| skip.skip_dir(n)));
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
        assert_eq!(
            tokenize("My Document_v2.pdf"),
            vec!["my", "document", "v2", "pdf"]
        );
        assert_eq!(
            tokenize("/home/user/notes/idea.md"),
            vec!["home", "user", "notes", "idea", "md"]
        );
    }

    #[test]
    fn tokenize_skips_empty() {
        assert_eq!(tokenize("..."), Vec::<String>::new());
        assert_eq!(tokenize(""), Vec::<String>::new());
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

        let idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::default(), 1_000_000);
        assert_eq!(idx.len(), 2);
        // "downloads" should be found via range walk on "dow"
        assert!(idx.token_index.candidates_for("downloads").is_some());
        // "hello" exact match
        assert!(idx.token_index.candidates_for("hello").is_some());
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
        assert_eq!(
            idx.entries[results[0].entry_idx as usize].name,
            "invoice.pdf"
        );
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
    fn search_short_prefix_range_walk() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("report.md"), "").unwrap();
        fs::write(base.join("readme.txt"), "").unwrap();
        fs::write(base.join("notes.md"), "").unwrap();
        let idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);
        let results = idx.search("re", 10);
        assert_eq!(results.len(), 2);
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

    #[test]
    fn apply_event_create_caches_lc_and_compact() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        let mut idx = InvertedFileIndex::empty();
        let p = base.join("MyFile_v2.rs");
        fs::write(&p, "").unwrap();
        idx.apply_event_create(&p);
        assert_eq!(idx.len(), 1);
        let entry = &idx.entries[0];
        assert_eq!(entry.name_lc.as_str(), "myfile_v2.rs");
        assert_eq!(entry.name_compact.as_str(), "myfilev2rs");
    }

    #[test]
    fn build_caches_lc_and_compact() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("UPPER.TXT"), "").unwrap();
        let idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);
        let entry = &idx.entries[0];
        assert_eq!(entry.name_lc.as_str(), "upper.txt");
        assert_eq!(entry.name_compact.as_str(), "uppertxt");
    }

    #[test]
    fn idf_downweights_common_tokens() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("parser_module_test.rs"), "").unwrap();
        fs::write(base.join("test_parser_module.rs"), "").unwrap();
        for i in 0..17 {
            fs::write(base.join(format!("test_module_{i}.rs")), "").unwrap();
        }
        let idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);
        let results = idx.search("parser test", 10);
        assert!(!results.is_empty());
        let top_name = &idx.entries[results[0].entry_idx as usize].name;
        assert_eq!(top_name, "parser_module_test.rs");
    }

    #[test]
    fn idf_recomputed_on_incremental_add() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("parser.rs"), "").unwrap();
        let mut idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);
        let idf_before = idx.token_index.candidates_for("parser").unwrap().1;

        for i in 0..20 {
            let p = base.join(format!("parser_{i}.rs"));
            fs::write(&p, "").unwrap();
            idx.apply_event_create(&p);
        }
        // Delta is not flushed until threshold; force compaction to update IDF.
        idx.token_index.compact(idx.entries.len());
        let idf_after = idx.token_index.candidates_for("parser").unwrap().1;
        assert!(
            idf_after < idf_before,
            "IDF should drop after adding more documents with the same token: before={idf_before}, after={idf_after}"
        );
    }

    #[test]
    fn delta_compaction_roundtrip() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        fs::write(base.join("file_a.rs"), "").unwrap();
        fs::write(base.join("file_b.rs"), "").unwrap();
        let mut idx = InvertedFileIndex::build(&[base.to_path_buf()], &SkipSet::defaults(), 1000);

        // Remove one file
        idx.apply_event_remove(&base.join("file_a.rs"));
        assert!(idx.search("file_a", 10).is_empty());

        // Add a new file
        let new_path = base.join("file_c.rs");
        fs::write(&new_path, "").unwrap();
        idx.apply_event_create(&new_path);
        assert!(!idx.search("file_c", 10).is_empty());

        // Verify compaction happened or search still works
        let results = idx.search("file", 10);
        assert_eq!(results.len(), 2); // file_b and file_c
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
        )
        .await;
        assert!(paths.is_empty());
    }
}

#[cfg(test)]
mod bench_helpers {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn seed_tree(base: &std::path::Path, n: usize) {
        let prefixes = &[
            "report", "readme", "config", "main", "utils", "test", "lib", "app", "index", "style",
        ];
        let exts = &[
            "rs", "ts", "js", "json", "md", "txt", "html", "css", "py", "go",
        ];
        for i in 0..n {
            let depth = (i % 5) + 1;
            let mut p = base.to_path_buf();
            for d in 0..depth {
                p.push(format!(
                    "{}_{}_{}",
                    prefixes[(i + d) % prefixes.len()],
                    i % 100,
                    d
                ));
            }
            fs::create_dir_all(&p).unwrap();
            let name = format!(
                "{}_{:04}.{}",
                prefixes[i % prefixes.len()],
                i,
                exts[i % exts.len()]
            );
            fs::write(p.join(&name), b"").unwrap();
        }
    }

    #[test]
    fn check_200k_candidates() {
        let dir = TempDir::new().unwrap();
        seed_tree(dir.path(), 200_000);
        let idx = InvertedFileIndex::build(
            &[dir.path().to_path_buf()],
            &SkipSet::defaults(),
            200_000 + 1_000,
        );

        if let Some((bitmap, idf)) = idx.token_index.candidates_for("re") {
            println!(
                "Candidates for 're' at 200k: {}, IDF: {}",
                bitmap.len(),
                idf
            );
        }
        if let Some((bitmap, idf)) = idx.token_index.candidates_for("report") {
            println!(
                "Candidates for 'report' at 200k: {}, IDF: {}",
                bitmap.len(),
                idf
            );
        }
        if let Some((bitmap, idf)) = idx.token_index.candidates_for("co") {
            println!(
                "Candidates for 'co' at 200k: {}, IDF: {}",
                bitmap.len(),
                idf
            );
        }
    }
}
