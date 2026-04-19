# Launcher Core Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land four launcher upgrades inspired by SuperCmd — inverted-prefix file index with mdfind fallback, structured execute results with no-view mode, inline argument UI, and a 25-preset window-management source — each as an independently shippable sub-PR.

**Architecture:** Each PR is additive (new types/fields with defaults, no breaking IPC). PR-1 replaces `FileSearchSource`'s linear scan with an `ArcSwap<InvertedFileIndex>`. PR-2 changes `launcher_execute` return type to `LauncherExecuteResult` and adds a status-badge Tauri window. PR-3 adds `ArgSpec` to `LauncherItem` plus `{{name}}` template substitution. PR-4 introduces a data-driven preset table and a new `WindowPresetsSource`.

**Tech Stack:** Rust 1.93, Tokio, `arc-swap` (new), `ignore` (existing), `nucleo-matcher` (existing), `notify-debouncer-mini` (existing), `smol_str` (new), `tempfile` (existing dev-dep), Tauri 2, React 18 + Vite + Bun + Vitest, `parking_lot`.

**Reference spec:** `docs/superpowers/specs/2026-04-19-launcher-core-improvements-design.md`

---

## File Map

### PR-1 InvertedFileIndex
- **Create:** `crates/feature-launcher/src/search/inverted_index.rs` (~600 LOC) — index data structure, build, search, apply_events, mdfind merge.
- **Modify:** `crates/feature-launcher/src/search/mod.rs` (add `pub mod inverted_index; pub use inverted_index::InvertedFileIndex;`).
- **Modify:** `crates/feature-launcher/src/search/file_search.rs` — replace `Arc<RwLock<Vec<FileEntry>>>` with `Arc<ArcSwap<InvertedFileIndex>>`; replace search/refresh logic.
- **Modify:** `crates/feature-launcher/src/search/file_watcher.rs` — when source name == `"files"`, call `apply_events` (new public method) instead of `refresh()`.
- **Modify:** `crates/feature-launcher/Cargo.toml` — add `arc-swap = "1.7"`, `smol_str = "0.3"` (already may be transitively present, verify).
- **Modify:** `crates/config/src/schema/launcher.rs` — add `max_entries`, `mdfind_fallback`, `mdfind_threshold`, `rebuild_interval_min` to `FileSearchConfig`.
- **Modify:** `crates/app-core/src/init/launcher.rs` — pass new config into `FileSearchSource::new`; spawn the 30-min rebuild task.

### PR-2 LauncherExecuteResult + no-view + status badge
- **Modify:** `crates/feature-launcher/src/types.rs` — add `LauncherExecuteResult`, `ExecStatus`, `BadgeKind`, `From<()>` impl, `no_view: bool` to `LauncherItem`, `arguments: Vec<ArgSpec>` placeholder (filled by PR-3 — for PR-2 add as `Vec<ArgSpec>` defaulted to `vec![]` with `ArgSpec` already defined as empty struct stub OR defer this field to PR-3 to keep PR-2 atomic — see Decision below).
- **Decision (atomicity):** PR-2 adds **only** `no_view: bool` and the result types. PR-3 adds `ArgSpec` and `arguments`. This keeps each PR self-contained.
- **Modify:** `crates/app-core/src/handlers/launcher/handlers.rs` — `launcher_execute` returns `LauncherExecuteResult`.
- **Modify:** `crates/app-core/src/handlers/launcher/engine.rs` (or wherever `record_execution` lives) — return `LauncherExecuteResult`.
- **Modify:** `crates/desktop/src/commands/launcher.rs` — change return type, update `dev_dispatch`.
- **Create:** `crates/desktop/src/commands/status_badge.rs` — `show_status_badge` Tauri command, mini-window lifecycle, `DEV_COMMANDS` export.
- **Modify:** `crates/desktop/src/commands/mod.rs` — register module.
- **Modify:** `crates/desktop/src/dev_server/mod.rs` — add `status_badge` to coverage list (per CLAUDE.md gotcha).
- **Create:** `desktop-ui/src/features/launcher/components/StatusBadge.tsx` — webview content (rendered inside the badge window).
- **Create:** `desktop-ui/src/features/launcher/status-badge-window.html` — entry HTML mounted by the new window.
- **Modify:** `desktop-ui/src/features/launcher/hooks/useLauncherExecute.ts` (or wherever execute is called) — handle structured result, trigger badge if `no_view`.
- **Modify:** `desktop-ui/src/shared/lib/ipc.ts` types if needed.

### PR-3 Inline arguments
- **Modify:** `crates/feature-launcher/src/types.rs` — add `ArgSpec`, `ArgKind`, add `arguments: Vec<ArgSpec>` to `LauncherItem`.
- **Create:** `crates/feature-launcher/src/template.rs` — `substitute(s: &str, args: &HashMap<String,String>) -> Result<String, TemplateErr>`.
- **Modify:** `crates/feature-launcher/src/lib.rs` — add `pub mod template;`.
- **Modify:** `crates/feature-launcher/src/search/script_runner.rs` — parse `# arg:` front-matter into `ArgSpec`s.
- **Modify:** `crates/feature-launcher/src/search/system_commands.rs` — DND command declares `duration` arg.
- **Modify:** `crates/app-core/src/handlers/launcher/handlers.rs` — `launcher_execute` accepts `args: HashMap<String, String>`.
- **Modify:** `crates/desktop/src/commands/launcher.rs` — pass through args; update dev dispatch.
- **Create:** `desktop-ui/src/features/launcher/components/ArgChipBar.tsx`.
- **Modify:** `desktop-ui/src/features/launcher/pages/LauncherPage.tsx` (or wherever results list is) — switch to chip bar when item has args.
- **Modify:** `desktop-ui/src/features/launcher/hooks/useLauncherExecute.ts` — send args.

### PR-4 Window presets
- **Create:** `crates/feature-launcher/src/window_mgmt/presets.rs` — `Preset` struct + `PRESETS` const table (25 entries).
- **Modify:** `crates/feature-launcher/src/window_mgmt/mod.rs` — `pub mod presets;`.
- **Modify:** `crates/feature-launcher/src/types.rs` — add `WindowAction::Preset(String)` variant (use `String` not `&'static str` for serde).
- **Modify:** `crates/feature-launcher/src/window_mgmt/actions.rs` — `apply_preset(name)` method.
- **Create:** `crates/feature-launcher/src/search/window_presets.rs` — `WindowPresetsSource` impl.
- **Modify:** `crates/feature-launcher/src/search/mod.rs` — register module + re-export.
- **Modify:** `crates/config/src/schema/launcher.rs` — add `window_presets: bool` (default true) to source flags.
- **Modify:** `crates/app-core/src/init/launcher.rs` — wire source.

---

# PR-1 — InvertedFileIndex

### Task 1.1: Add `arc-swap` and `smol_str` deps

**Files:**
- Modify: `crates/feature-launcher/Cargo.toml`

- [ ] **Step 1: Edit Cargo.toml**

Add under `[dependencies]`:
```toml
arc-swap = "1.7"
smol_str = "0.3"
```

- [ ] **Step 2: Verify build**

```bash
cargo build -p feature-launcher
```
Expected: build succeeds.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/Cargo.toml Cargo.lock
git commit -m "build(feature-launcher): add arc-swap and smol_str deps"
```

### Task 1.2: Skeleton `inverted_index.rs` with types + tokenize

**Files:**
- Create: `crates/feature-launcher/src/search/inverted_index.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Add module to mod.rs**

Edit `crates/feature-launcher/src/search/mod.rs` — add line under existing `pub mod` lines:
```rust
pub mod inverted_index;
```
And under existing `pub use`:
```rust
pub use inverted_index::{InvertedFileIndex, IndexEntry, ScoredEntry};
```

- [ ] **Step 2: Create skeleton with tokenize tests**

Create `crates/feature-launcher/src/search/inverted_index.rs`:
```rust
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
}
```

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p feature-launcher inverted_index
```
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/src/search/inverted_index.rs crates/feature-launcher/src/search/mod.rs
git commit -m "feat(feature-launcher): inverted index skeleton with tokenize/prefixes"
```

### Task 1.3: Implement `build` (BFS walk + index population)

**Files:**
- Modify: `crates/feature-launcher/src/search/inverted_index.rs`

- [ ] **Step 1: Write failing test**

Append to `tests` mod:
```rust
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
```

- [ ] **Step 2: Verify it fails**

```bash
cargo nextest run -p feature-launcher inverted_index::tests::build_
```
Expected: FAIL — `SkipSet`/`build` not found.

- [ ] **Step 3: Implement**

Add to `inverted_index.rs` (above `tests`):
```rust
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
```

- [ ] **Step 4: Run tests**

```bash
cargo nextest run -p feature-launcher inverted_index
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/inverted_index.rs
git commit -m "feat(feature-launcher): InvertedFileIndex::build with name + path-component indexing"
```

### Task 1.4: Implement `search` (intersect + score)

**Files:**
- Modify: `crates/feature-launcher/src/search/inverted_index.rs`

- [ ] **Step 1: Write failing tests**

Append to `tests` mod:
```rust
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
```

- [ ] **Step 2: Verify failure**

```bash
cargo nextest run -p feature-launcher inverted_index::tests::search_
```
Expected: FAIL.

- [ ] **Step 3: Implement search + scorer**

Add to `inverted_index.rs`:
```rust
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
        scored.truncate(limit);
        scored
    }
}
```

(Tie-break: when raw scores match, shorter names win — `"hello.rs"` beats `"hellothere.rs"` for query `"hello"`.)

- [ ] **Step 4: Run tests**

```bash
cargo nextest run -p feature-launcher inverted_index
```
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/inverted_index.rs
git commit -m "feat(feature-launcher): InvertedFileIndex::search with shortest-list intersection"
```

### Task 1.5: Implement `apply_events` for incremental updates

**Files:**
- Modify: `crates/feature-launcher/src/search/inverted_index.rs`

- [ ] **Step 1: Write failing tests**

Append:
```rust
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
```

- [ ] **Step 2: Verify failure**

```bash
cargo nextest run -p feature-launcher inverted_index::tests::apply_events_
```
Expected: FAIL.

- [ ] **Step 3: Implement**

Add to `inverted_index.rs`:
```rust
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
```

Also adjust `search` to skip tombstoned entries — modify the score loop to return `i32::MIN` for empty-name entries, then filter:
```rust
// In search, after computing `scored`:
scored.retain(|s| !self.entries[s.entry_idx as usize].name.is_empty());
```

- [ ] **Step 4: Run tests**

```bash
cargo nextest run -p feature-launcher inverted_index
```
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/inverted_index.rs
git commit -m "feat(feature-launcher): incremental add/remove/rename for InvertedFileIndex"
```

### Task 1.6: Implement mdfind fallback

**Files:**
- Modify: `crates/feature-launcher/src/search/inverted_index.rs`

- [ ] **Step 1: Add async mdfind helper + test (manual gating)**

Add (gated to macos):
```rust
#[cfg(target_os = "macos")]
impl InvertedFileIndex {
    pub async fn mdfind_merge(
        &self,
        query: &str,
        roots: &[PathBuf],
        skip: &SkipSet,
        existing: &mut Vec<ScoredEntry>,
        threshold: usize,
        cancel: tokio_util::sync::CancellationToken,
    ) {
        if existing.len() >= threshold { return; }
        let tokens = tokenize(query);
        if tokens.is_empty() { return; }

        let mut handles = Vec::new();
        for root in roots {
            let root = root.clone();
            let q = query.to_string();
            let cancel = cancel.clone();
            handles.push(tokio::spawn(async move {
                let res = tokio::select! {
                    _ = cancel.cancelled() => return Vec::<PathBuf>::new(),
                    r = tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        tokio::process::Command::new("/usr/bin/mdfind")
                            .arg("-onlyin").arg(&root)
                            .arg(&q)
                            .output(),
                    ) => r,
                };
                let out = match res { Ok(Ok(o)) => o, _ => return Vec::new() };
                if !out.status.success() { return Vec::new(); }
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(|l| PathBuf::from(l.trim()))
                    .collect()
            }));
        }

        let mut new_paths = Vec::new();
        for h in handles {
            if let Ok(paths) = h.await { new_paths.extend(paths); }
        }

        // Filter via SkipSet
        new_paths.retain(|p| {
            let in_skip_dir = p.components().any(|c| {
                c.as_os_str().to_str().is_some_and(|n| skip.skip_dir(n))
            });
            !in_skip_dir
        });

        // Dedupe against entries already in `existing`
        let existing_paths: std::collections::HashSet<&PathBuf> = existing.iter()
            .map(|s| &self.entries[s.entry_idx as usize].path).collect();
        new_paths.retain(|p| !existing_paths.contains(p));

        // Score with same function, -5 bias, push as synthetic ScoredEntry indexes...
        // Implementation note: mdfind hits aren't in `entries`. We can't use ScoredEntry
        // (which references entries). Strategy: add them as IndexEntry to a *throwaway* clone
        // of self... but that breaks the ArcSwap immutability promise.
        //
        // Cleaner: return a separate Vec<MdfindHit { path, name, kind, score }> and let
        // FileSearchSource merge LauncherItem-level.
        //
        // For now this method is a no-op stub; FileSearchSource will call mdfind directly
        // (see Task 1.8). Document the design choice and move on.
        let _ = new_paths;
        let _ = threshold;
    }
}
```

**Decision (correctness over cleverness):** mdfind merge happens at the `FileSearchSource` layer (Task 1.8), not inside the immutable `InvertedFileIndex`. This task therefore only adds the SkipSet filter helper. Replace the above body with the helper only:

```rust
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
```

Add `tokio-util = { version = "0.7", features = ["rt"] }` to `feature-launcher/Cargo.toml`.

- [ ] **Step 2: Add a unit test for SkipSet filtering (mdfind itself untestable in CI)**

```rust
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
```

- [ ] **Step 3: Run**

```bash
cargo nextest run -p feature-launcher inverted_index
```
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/src/search/inverted_index.rs crates/feature-launcher/Cargo.toml Cargo.lock
git commit -m "feat(feature-launcher): mdfind_paths helper with SkipSet filter and cancellation"
```

### Task 1.7: Add config fields

**Files:**
- Modify: `crates/config/src/schema/launcher.rs`

- [ ] **Step 1: Read existing FileSearchConfig**

```bash
cargo run --quiet --bin nothing 2>&1 || true  # placeholder; engineer reads file directly
```

(Engineer: open `crates/config/src/schema/launcher.rs`, find `FileSearchConfig`.)

- [ ] **Step 2: Add fields with serde defaults**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchConfig {
    pub roots: Vec<String>,
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
    #[serde(default = "default_true")]
    pub mdfind_fallback: bool,
    #[serde(default = "default_mdfind_threshold")]
    pub mdfind_threshold: usize,
    #[serde(default = "default_rebuild_interval_min")]
    pub rebuild_interval_min: u64,
    // ...existing fields
}

fn default_max_entries() -> usize { 1_000_000 }
fn default_true() -> bool { true }
fn default_mdfind_threshold() -> usize { 20 }
fn default_rebuild_interval_min() -> u64 { 30 }
```

If `default_true` already exists in scope, reuse.

- [ ] **Step 3: Build**

```bash
cargo build -p config && cargo build -p feature-launcher
```
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/launcher.rs
git commit -m "feat(config): add file_search index/mdfind/rebuild knobs"
```

### Task 1.8: Rewire `FileSearchSource` to use the index

**Files:**
- Modify: `crates/feature-launcher/src/search/file_search.rs`

- [ ] **Step 1: Replace struct + impls**

Replace the entire `FileSearchSource` implementation:

```rust
use crate::search::inverted_index::{InvertedFileIndex, SkipSet, ScoredEntry};
use crate::types::*;
use arc_swap::ArcSwap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct FileSearchSource {
    index: Arc<ArcSwap<InvertedFileIndex>>,
    roots: Vec<PathBuf>,
    skip: SkipSet,
    max_entries: usize,
    mdfind_fallback: bool,
    mdfind_threshold: usize,
    cancel: Arc<parking_lot::Mutex<CancellationToken>>,
}

impl FileSearchSource {
    pub fn new(
        scan_dirs: Vec<String>,
        max_entries: usize,
        mdfind_fallback: bool,
        mdfind_threshold: usize,
    ) -> Self {
        let roots: Vec<PathBuf> = scan_dirs.iter()
            .map(|s| PathBuf::from(shellexpand::tilde(s).to_string()))
            .collect();
        Self {
            index: Arc::new(ArcSwap::from_pointee(InvertedFileIndex::empty())),
            roots,
            skip: SkipSet::defaults(),
            max_entries,
            mdfind_fallback,
            mdfind_threshold,
            cancel: Arc::new(parking_lot::Mutex::new(CancellationToken::new())),
        }
    }

    pub async fn apply_events(&self, paths: Vec<(PathBuf, FsEventKind)>) {
        let current = self.index.load_full();
        let mut next = (*current).clone_for_patch();
        for (p, kind) in paths {
            match kind {
                FsEventKind::Create => next.apply_event_create(&p),
                FsEventKind::Remove => next.apply_event_remove(&p),
            }
        }
        self.index.store(Arc::new(next));
    }

    pub fn index_len(&self) -> usize {
        self.index.load().len()
    }
}

#[derive(Clone, Copy)]
pub enum FsEventKind { Create, Remove }
```

(`InvertedFileIndex` needs a `clone_for_patch` method — add it: just `Clone` derive on the struct, then `pub fn clone_for_patch(&self) -> Self { self.clone() }`. Add `#[derive(Clone)]` to `IndexEntry` and `InvertedFileIndex`. Note `HashMap<SmolStr, Vec<u32>>` is `Clone`.)

- [ ] **Step 2: Implement `SearchSource` trait**

```rust
#[async_trait::async_trait]
impl super::SearchSource for FileSearchSource {
    fn name(&self) -> &'static str { "files" }
    fn prefix(&self) -> Option<&'static str> { Some("f/") }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() { return vec![]; }
        // Cancel any prior in-flight mdfind for this source
        let cancel = {
            let mut g = self.cancel.lock();
            g.cancel();
            *g = CancellationToken::new();
            g.clone()
        };
        let snap = self.index.load_full();
        let mut scored = snap.search(query, limit);

        let mut items: Vec<LauncherItem> = scored.iter().map(|s| {
            let e = &snap.entries[s.entry_idx as usize];
            let path_str = e.path.display().to_string();
            LauncherItem {
                id: format!("file:{path_str}"),
                title: e.name.clone(),
                subtitle: Some(path_str),
                icon: Some("file".to_string()),
                kind: LauncherItemKind::File { path: e.path.clone(), kind: e.kind },
                score: (s.score as f64 / 200.0) * 0.85,
                no_view: false,
            }
        }).collect();

        // mdfind fallback
        #[cfg(target_os = "macos")]
        if self.mdfind_fallback && items.len() < self.mdfind_threshold {
            let extra = crate::search::inverted_index::mdfind_paths(
                query, &self.roots, &self.skip, cancel,
            ).await;
            let existing: std::collections::HashSet<PathBuf> =
                items.iter().filter_map(|i| match &i.kind {
                    LauncherItemKind::File { path, .. } => Some(path.clone()),
                    _ => None,
                }).collect();
            for p in extra {
                if existing.contains(&p) { continue; }
                let name = p.file_name().and_then(|n| n.to_str())
                    .unwrap_or("").to_string();
                if name.is_empty() { continue; }
                let path_str = p.display().to_string();
                items.push(LauncherItem {
                    id: format!("file:{path_str}"),
                    title: name,
                    subtitle: Some(path_str),
                    icon: Some("file".to_string()),
                    kind: LauncherItemKind::File {
                        path: p.clone(),
                        kind: classify_extension_pub(&p),
                    },
                    score: 0.40, // baseline, biased below local hits
                    no_view: false,
                });
            }
            items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            items.truncate(limit);
        }

        items
    }

    async fn refresh(&self) {
        let roots = self.roots.clone();
        let skip = self.skip.clone();
        let cap = self.max_entries;
        let new = tokio::task::spawn_blocking(move || {
            InvertedFileIndex::build(&roots, &skip, cap)
        }).await.unwrap_or_else(|e| {
            tracing::error!("inverted index build failed: {e}");
            InvertedFileIndex::empty()
        });
        tracing::info!("Indexed {} files (inverted)", new.len());
        self.index.store(Arc::new(new));
    }
}

fn classify_extension_pub(path: &std::path::Path) -> FileKind {
    crate::search::inverted_index::classify_extension(path)
}
```

(Make `classify_extension` `pub(crate)` in `inverted_index.rs`.)

`LauncherItem.no_view` requires PR-2's field. **For PR-1 only**, remove `no_view: false` lines and rely on the spec adding it in PR-2. After PR-2 merges, a follow-up adjusts. Or: add `no_view: bool` to `LauncherItem` here as part of PR-1 (acceptable — additive). Decision for cleanliness: add `no_view` field in PR-1 with default false (one-line change, simplifies subsequent PRs). Update `types.rs` accordingly in this task.

- [ ] **Step 3: Update existing `FileSearchSource::new` callers**

Search for callers:
```bash
grep -rn 'FileSearchSource::new' crates/
```
Update each call site (likely only `crates/app-core/src/init/launcher.rs`) to pass new params:
```rust
FileSearchSource::new(
    file_cfg.roots.clone(),
    file_cfg.max_entries,
    file_cfg.mdfind_fallback,
    file_cfg.mdfind_threshold,
)
```

- [ ] **Step 4: Build + test**

```bash
cargo build -p feature-launcher
cargo nextest run -p feature-launcher
```
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add -A crates/feature-launcher crates/app-core/src/init/launcher.rs crates/feature-launcher/src/types.rs
git commit -m "feat(feature-launcher): rewire FileSearchSource onto InvertedFileIndex + mdfind fallback"
```

### Task 1.9: Hook FSEvents to incremental updates

**Files:**
- Modify: `crates/feature-launcher/src/search/file_watcher.rs`
- Modify: `crates/app-core/src/init/launcher.rs`

- [ ] **Step 1: Inspect file_watcher.rs**

Read the file. Find where it currently calls `source.refresh().await`.

- [ ] **Step 2: Branch on source name "files"**

In the change handler, check the watch entry's `source` reference. If `source.name() == "files"`, downcast (via a new helper) and call `apply_events`.

Cleaner approach: extend `WatchEntry` with an optional `on_change: Option<Arc<dyn Fn(Vec<DebouncedEvent>) -> BoxFuture<()>>>`. Default behavior: call `refresh()`. For files, register a custom handler.

In `app-core/src/init/launcher.rs` where the files watch is registered, attach a handler:
```rust
let fs_clone = file_search.clone();
WatchEntry {
    source: file_search.clone(),
    paths: roots.clone(),
    cooldown_ms: 500,
    on_change: Some(Arc::new(move |events: Vec<DebouncedEvent>| {
        let fs = fs_clone.clone();
        Box::pin(async move {
            let mut changes = Vec::new();
            for ev in events {
                let kind = if ev.path.exists() { FsEventKind::Create } else { FsEventKind::Remove };
                changes.push((ev.path, kind));
            }
            fs.apply_events(changes).await;
        })
    })),
}
```

- [ ] **Step 3: Update `SourceFileWatcher` to use `on_change` if present**

```rust
match &entry.on_change {
    Some(handler) => handler(events).await,
    None => entry.source.refresh().await,
}
```

- [ ] **Step 4: Test**

```bash
cargo nextest run -p feature-launcher
cargo build -p app-core
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(feature-launcher): incremental index updates via FSEvent handler"
```

### Task 1.10: 30-min safety rebuild loop

**Files:**
- Modify: `crates/app-core/src/init/launcher.rs`

- [ ] **Step 1: Spawn a tokio task in init_launcher**

After constructing `file_search`:
```rust
let interval = std::time::Duration::from_secs(file_cfg.rebuild_interval_min * 60);
let fs_for_rebuild = file_search.clone();
tokio::spawn(async move {
    let mut tick = tokio::time::interval(interval);
    tick.tick().await; // skip immediate first tick (initial build already runs)
    loop {
        tick.tick().await;
        tracing::info!("launcher: triggering 30-min safety rebuild");
        fs_for_rebuild.refresh().await;
    }
});
```

- [ ] **Step 2: Verify build**

```bash
cargo build -p app-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/launcher.rs
git commit -m "feat(launcher): 30-min safety rebuild loop for inverted index"
```

### Task 1.11: PR-1 final gates

- [ ] **Step 1: Lint**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: zero warnings.

- [ ] **Step 2: Format**

```bash
cargo fmt --all --check
```
Expected: pass; if not, run `cargo fmt --all` and amend.

- [ ] **Step 3: Full test run**

```bash
cargo nextest run --workspace
```
Expected: green.

- [ ] **Step 4: Manual smoke**

```bash
cargo build --release -p klyntbot
KLYNTBOT_HOME=~/.klyntbot-dev ./target/release/klyntbot &
# In another terminal: type a file query in the launcher, verify result appears within ~50ms
```

- [ ] **Step 5: Open PR**

```bash
git push -u origin <branch>
gh pr create --title "feat(launcher): inverted-prefix file index with mdfind fallback" --body "Implements PR-1 of docs/superpowers/specs/2026-04-19-launcher-core-improvements-design.md"
```

---

# PR-2 — LauncherExecuteResult + no-view + status badge

### Task 2.1: Add result types

**Files:**
- Modify: `crates/feature-launcher/src/types.rs`

- [ ] **Step 1: Append types**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherExecuteResult {
    pub status: ExecStatus,
    pub message: Option<String>,
    pub badge: BadgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum ExecStatus {
    Ok,
    Err(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BadgeKind { Success, Warn, Error, Info }

impl Default for LauncherExecuteResult {
    fn default() -> Self {
        Self { status: ExecStatus::Ok, message: None, badge: BadgeKind::Info }
    }
}

impl From<()> for LauncherExecuteResult {
    fn from(_: ()) -> Self { Self::default() }
}

impl LauncherExecuteResult {
    pub fn ok_msg(msg: impl Into<String>) -> Self {
        Self { status: ExecStatus::Ok, message: Some(msg.into()), badge: BadgeKind::Success }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        let m = msg.into();
        Self { status: ExecStatus::Err(m.clone()), message: Some(m), badge: BadgeKind::Error }
    }
}
```

If PR-1 already added `no_view`, skip adding it here. Otherwise:
```rust
// in LauncherItem struct:
#[serde(default)]
pub no_view: bool,
```

- [ ] **Step 2: Test**

```bash
cargo build -p feature-launcher
cargo nextest run -p feature-launcher
```

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/src/types.rs
git commit -m "feat(feature-launcher): LauncherExecuteResult, ExecStatus, BadgeKind"
```

### Task 2.2: Change `launcher_execute` return type

**Files:**
- Modify: `crates/app-core/src/handlers/launcher/handlers.rs`
- Modify: `crates/desktop/src/commands/launcher.rs`
- Modify: `crates/app-core/src/handlers/launcher/engine.rs` (or wherever `record_execution` lives)

- [ ] **Step 1: Update engine**

`record_execution` returns `Result<(), ApiError>`. Wrap into `LauncherExecuteResult`:
```rust
pub async fn execute(&self, item_id: &str, kind: &str) -> Result<LauncherExecuteResult, ApiError> {
    self.record_execution(item_id, kind).await?;
    Ok(LauncherExecuteResult::default())
}
```
Keep `record_execution` private/internal. Make `execute` the public entry.

- [ ] **Step 2: Update handler**

```rust
pub async fn launcher_execute(
    &self, item_id: String, kind: String,
) -> Result<LauncherExecuteResult, ApiError> {
    let engine = self.launcher_engine()?;
    engine.execute(&item_id, &kind).await
}
```

- [ ] **Step 3: Update Tauri command**

```rust
#[tauri::command]
pub async fn launcher_execute(
    state: State<'_, Arc<AppCore>>,
    item_id: String, kind: String,
) -> Result<LauncherExecuteResult, ApiError> {
    state.launcher_execute(item_id, kind).await
}
```

And update `dispatch_dev`:
```rust
"launcher_execute" => {
    let item_id = dev::get(body, "itemId").unwrap_or_default();
    let kind = dev::get(body, "kind").unwrap_or_default();
    dev::val(core.launcher_execute(item_id, kind).await)
}
```

- [ ] **Step 4: Test + lint**

```bash
cargo nextest run --workspace
cargo clippy -p feature-launcher -p app-core -p desktop --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(launcher): launcher_execute returns LauncherExecuteResult"
```

### Task 2.3: Status badge Tauri command

**Files:**
- Create: `crates/desktop/src/commands/status_badge.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs` (add to coverage)

- [ ] **Step 1: Implement command**

```rust
//! Mini status-badge window for no-view launcher executions.

use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::app_core::AppCore;
use desktop_shared::errors::ApiError;
use feature_launcher::BadgeKind;

const WIN_LABEL: &str = "status_badge";
const WIDTH: f64 = 280.0;
const HEIGHT: f64 = 40.0;

#[tauri::command]
pub async fn show_status_badge(
    app: AppHandle,
    text: String,
    kind: BadgeKind,
    duration_ms: Option<u32>,
) -> Result<(), ApiError> {
    let dur = duration_ms.unwrap_or(2000);

    if let Some(existing) = app.get_webview_window(WIN_LABEL) {
        let _ = existing.emit("badge:update", serde_json::json!({"text": text, "kind": kind, "ms": dur}));
        return Ok(());
    }

    let url = WebviewUrl::App("status-badge.html".into());
    let win = WebviewWindowBuilder::new(&app, WIN_LABEL, url)
        .title("Klynt Status")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .focused(false)
        .skip_taskbar(true)
        .transparent(true)
        .build()
        .map_err(|e| ApiError::new("BADGE_WINDOW", e.to_string()))?;

    // Position top-right of focused screen
    if let Some(monitor) = win.current_monitor().ok().flatten() {
        let size = monitor.size();
        let pos = monitor.position();
        let x = pos.x + size.width as i32 - (WIDTH as i32) - 24;
        let y = pos.y + 60;
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
    }

    let _ = win.emit("badge:show", serde_json::json!({"text": text, "kind": kind, "ms": dur}));

    let app_for_close = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(dur as u64)).await;
        if let Some(w) = app_for_close.get_webview_window(WIN_LABEL) {
            let _ = w.close();
        }
    });

    Ok(())
}

pub(crate) const DEV_COMMANDS: &[&str] = &["show_status_badge"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    _core: &Arc<AppCore>,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;
    if cmd != "show_status_badge" { return None; }
    // Dev server can't open Tauri windows; treat as no-op success.
    let _ = body;
    Some(dev::val(Ok::<(), ApiError>(())))
}
```

- [ ] **Step 2: Register module**

In `crates/desktop/src/commands/mod.rs`:
```rust
pub mod status_badge;
```

In Tauri builder (likely `crates/desktop/src/lib.rs`), add to the `invoke_handler`:
```rust
.invoke_handler(tauri::generate_handler![
    // ...existing
    commands::status_badge::show_status_badge,
])
```

- [ ] **Step 3: Update dev_server coverage**

In `crates/desktop/src/dev_server/mod.rs`, add `status_badge::DEV_COMMANDS` to the coverage merger and `dispatch_dev` chain.

- [ ] **Step 4: Build**

```bash
cargo build -p desktop
cargo nextest run -p desktop
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(desktop): show_status_badge Tauri command + dev coverage"
```

### Task 2.4: Status badge frontend

**Files:**
- Create: `desktop-ui/status-badge.html` (Tauri serves from project root or `dist/`)
- Create: `desktop-ui/src/features/launcher/status-badge/main.tsx`
- Create: `desktop-ui/src/features/launcher/status-badge/StatusBadge.tsx`
- Modify: `desktop-ui/vite.config.ts` (add `rollupOptions.input` for new entry)

- [ ] **Step 1: Add Vite entry**

Edit `desktop-ui/vite.config.ts`:
```ts
build: {
  rollupOptions: {
    input: {
      main: path.resolve(__dirname, 'index.html'),
      statusBadge: path.resolve(__dirname, 'status-badge.html'),
    },
  },
},
```

- [ ] **Step 2: HTML entry**

Create `desktop-ui/status-badge.html`:
```html
<!doctype html>
<html><head>
  <meta charset="UTF-8" />
  <link rel="stylesheet" href="/src/styles/theme.css" />
</head><body><div id="root"></div>
  <script type="module" src="/src/features/launcher/status-badge/main.tsx"></script>
</body></html>
```

- [ ] **Step 3: React entry**

Create `desktop-ui/src/features/launcher/status-badge/main.tsx`:
```tsx
import React from 'react'
import { createRoot } from 'react-dom/client'
import { StatusBadge } from './StatusBadge'

createRoot(document.getElementById('root')!).render(<StatusBadge />)
```

- [ ] **Step 4: Component**

Create `desktop-ui/src/features/launcher/status-badge/StatusBadge.tsx`:
```tsx
import React, { useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'

type Kind = 'success' | 'warn' | 'error' | 'info'
interface Payload { text: string; kind: Kind; ms: number }

const COLORS: Record<Kind, string> = {
  success: 'text-green-400',
  warn: 'text-yellow-400',
  error: 'text-red-400',
  info: 'text-fg',
}

export function StatusBadge() {
  const [payload, setPayload] = useState<Payload | null>(null)

  useEffect(() => {
    const u1 = listen<Payload>('badge:show', (e) => setPayload(e.payload))
    const u2 = listen<Payload>('badge:update', (e) => setPayload(e.payload))
    return () => { u1.then((f) => f()); u2.then((f) => f()) }
  }, [])

  if (!payload) return null
  return (
    <div className="glass-panel h-full w-full rounded-md flex items-center px-3 text-sm">
      <span className={`${COLORS[payload.kind]} truncate`}>{payload.text}</span>
    </div>
  )
}
```

- [ ] **Step 5: Build frontend**

```bash
cd desktop-ui && bun run build
```
Expected: success, two outputs (`index.html` + `status-badge.html`).

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/status-badge.html desktop-ui/src/features/launcher/status-badge/ desktop-ui/vite.config.ts
git commit -m "feat(desktop-ui): status badge mini-window content"
```

### Task 2.5: Frontend hookup — call badge on no_view execute

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useLauncherExecute.ts` (or equivalent)
- Modify: `desktop-ui/src/features/launcher/types.ts` (add `noView`, `LauncherExecuteResult`)

- [ ] **Step 1: Update TS types**

```ts
export interface LauncherExecuteResult {
  status: { kind: 'ok' } | { kind: 'err'; message: string }
  message?: string | null
  badge: 'success' | 'warn' | 'error' | 'info'
}

export interface LauncherItem {
  // ...existing
  noView?: boolean
}
```

- [ ] **Step 2: Update execute hook**

```ts
import { invoke } from '@tauri-apps/api/core'

export function useLauncherExecute() {
  return useMutation('launcher_execute', async (item: LauncherItem) => {
    const result = await invoke<LauncherExecuteResult>('launcher_execute', {
      itemId: item.id, kind: item.kind.type,
    })
    if (item.noView) {
      await closeLauncher()
      const text = result.message ?? item.title
      if (result.message) {
        await invoke('show_status_badge', { text, kind: result.badge, durationMs: 2000 })
      }
    }
    return result
  })
}
```

- [ ] **Step 3: Vitest**

Create `desktop-ui/src/features/launcher/hooks/__tests__/useLauncherExecute.test.ts` (skip if hook isn't easily testable in isolation; instead add a unit test for the badge-payload shaping logic factored into a pure function).

- [ ] **Step 4: Build + lint frontend**

```bash
cd desktop-ui && bun run lint && bun run build && bun run test
```

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/launcher/
git commit -m "feat(desktop-ui): trigger status badge on no_view execute"
```

### Task 2.6: PR-2 gates

- [ ] **Step 1: Run all gates**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd desktop-ui && bun run lint && bun run test && bun run build
```

- [ ] **Step 2: Manual smoke**

Run the desktop app, trigger any launcher item with `no_view: true` (you can temporarily mark a system command for testing), verify the badge window appears top-right and dismisses after 2s.

- [ ] **Step 3: Open PR**

```bash
gh pr create --title "feat(launcher): structured execute result + no-view status badge" --body "Implements PR-2 of the launcher core improvements spec."
```

---

# PR-3 — Inline arguments

### Task 3.1: Add `ArgSpec` types + template substitution

**Files:**
- Modify: `crates/feature-launcher/src/types.rs`
- Create: `crates/feature-launcher/src/template.rs`
- Modify: `crates/feature-launcher/src/lib.rs`

- [ ] **Step 1: Add types**

In `types.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgSpec {
    pub name: String,
    pub placeholder: String,
    pub kind: ArgKind,
    #[serde(default = "default_true")]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "values", rename_all = "camelCase")]
pub enum ArgKind {
    Text,
    Number,
    Choice(Vec<String>),
}

fn default_true() -> bool { true }

// add to LauncherItem:
#[serde(default)]
pub arguments: Vec<ArgSpec>,
```

- [ ] **Step 2: Create template module with tests**

Create `crates/feature-launcher/src/template.rs`:
```rust
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum TemplateErr {
    #[error("missing argument: {0}")]
    MissingArg(String),
}

/// Substitute `{{name}}` placeholders in `s` with values from `args`.
/// Unknown placeholders return MissingArg. `{{{{` escapes to literal `{{`.
pub fn substitute(s: &str, args: &HashMap<String, String>) -> Result<String, TemplateErr> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '{' && chars.peek().map(|(_, c)| *c) == Some('{') {
            chars.next(); // consume second {
            // Escape: {{{{ → {{
            if chars.peek().map(|(_, c)| *c) == Some('{') {
                chars.next();
                if chars.peek().map(|(_, c)| *c) == Some('{') {
                    chars.next();
                    out.push_str("{{");
                    continue;
                }
            }
            // Read until }}
            let mut name = String::new();
            let mut closed = false;
            while let Some(&(_, c)) = chars.peek() {
                chars.next();
                if c == '}' && chars.peek().map(|(_, c)| *c) == Some('}') {
                    chars.next();
                    closed = true;
                    break;
                }
                name.push(c);
            }
            if !closed {
                // Unclosed: emit literally
                out.push_str("{{");
                out.push_str(&name);
                continue;
            }
            let value = args.get(&name).ok_or(TemplateErr::MissingArg(name.clone()))?;
            out.push_str(value);
        } else {
            out.push(c);
            let _ = i;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn simple_substitution() {
        let r = substitute("hello {{name}}", &args(&[("name", "world")])).unwrap();
        assert_eq!(r, "hello world");
    }

    #[test]
    fn missing_arg_errors() {
        let r = substitute("hi {{x}}", &args(&[]));
        assert!(matches!(r, Err(TemplateErr::MissingArg(s)) if s == "x"));
    }

    #[test]
    fn escape_double_brace() {
        let r = substitute("{{{{not}}}}", &args(&[])).unwrap();
        assert_eq!(r, "{{not}}");
    }

    #[test]
    fn no_args_passthrough() {
        let r = substitute("plain text", &args(&[])).unwrap();
        assert_eq!(r, "plain text");
    }
}
```

In `crates/feature-launcher/src/lib.rs`, add `pub mod template;`.

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p feature-launcher template
```
Expected: 4 pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/src/types.rs crates/feature-launcher/src/template.rs crates/feature-launcher/src/lib.rs
git commit -m "feat(feature-launcher): ArgSpec types and {{name}} template substitution"
```

### Task 3.2: Threading args through execute

**Files:**
- Modify: `crates/app-core/src/handlers/launcher/handlers.rs`
- Modify: `crates/app-core/src/handlers/launcher/engine.rs`
- Modify: `crates/desktop/src/commands/launcher.rs`

- [ ] **Step 1: Update handler signature**

```rust
pub async fn launcher_execute(
    &self,
    item_id: String,
    kind: String,
    args: std::collections::HashMap<String, String>,
) -> Result<LauncherExecuteResult, ApiError> {
    let engine = self.launcher_engine()?;
    engine.execute(&item_id, &kind, &args).await
}
```

- [ ] **Step 2: Update engine to substitute templates**

In engine `execute`, before dispatching to kind handler, run `template::substitute` on string fields of the kind payload (e.g. for `Script`, on the script's `args` array; for `SystemCommand` AppleScript text). For PR-3 initial scope, only `ScriptRunner` and DND get substitution; other kinds with no args just bypass.

- [ ] **Step 3: Update Tauri command**

```rust
#[tauri::command]
pub async fn launcher_execute(
    state: State<'_, Arc<AppCore>>,
    item_id: String, kind: String,
    args: Option<std::collections::HashMap<String, String>>,
) -> Result<LauncherExecuteResult, ApiError> {
    state.launcher_execute(item_id, kind, args.unwrap_or_default()).await
}
```

Update `dispatch_dev` similarly.

- [ ] **Step 4: Test + lint**

```bash
cargo nextest run --workspace
cargo clippy -p feature-launcher -p app-core -p desktop --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(launcher): thread args through launcher_execute IPC + template substitution"
```

### Task 3.3: ScriptRunner reads `# arg:` front-matter

**Files:**
- Modify: `crates/feature-launcher/src/search/script_runner.rs`

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn parses_arg_frontmatter() {
    let dir = TempDir::new().unwrap();
    let p = dir.path().join("greet.sh");
    std::fs::write(&p, "#!/bin/sh\n# name: Greet\n# arg: name=Your name\n# arg: greeting=Hello\necho \"{{greeting}}, {{name}}!\"\n").unwrap();
    let runner = ScriptRunner::new(dir.path().to_path_buf());
    let items = runner.search("greet", 10).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].arguments.len(), 2);
    assert_eq!(items[0].arguments[0].name, "name");
    assert_eq!(items[0].arguments[1].placeholder, "Hello");
}
```

- [ ] **Step 2: Verify failure**

```bash
cargo nextest run -p feature-launcher script_runner::tests::parses_arg_frontmatter
```

- [ ] **Step 3: Implement**

Update front-matter parser to recognize `# arg: name=placeholder` lines, push `ArgSpec { name, placeholder, kind: ArgKind::Text, required: true }` into `arguments`.

- [ ] **Step 4: Pass test**

```bash
cargo nextest run -p feature-launcher script_runner
```

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/script_runner.rs
git commit -m "feat(launcher): ScriptRunner parses # arg: front-matter into ArgSpec"
```

### Task 3.4: SystemCommands DND duration arg

**Files:**
- Modify: `crates/feature-launcher/src/search/system_commands.rs`

- [ ] **Step 1: Test**

```rust
#[tokio::test]
async fn dnd_command_has_duration_arg() {
    let cmd = SystemCommands::new();
    let items = cmd.search("dnd", 10).await;
    let dnd = items.iter().find(|i| i.title.to_lowercase().contains("do not disturb")).unwrap();
    assert_eq!(dnd.arguments.len(), 1);
    assert_eq!(dnd.arguments[0].name, "duration");
}
```

- [ ] **Step 2: Implement**

In the DND `LauncherItem`, set:
```rust
arguments: vec![ArgSpec {
    name: "duration".into(),
    placeholder: "30m, 2h, until tomorrow".into(),
    kind: ArgKind::Text,
    required: false,
}],
no_view: true,
```

- [ ] **Step 3: Pass + commit**

```bash
cargo nextest run -p feature-launcher system_commands
git add crates/feature-launcher/src/search/system_commands.rs
git commit -m "feat(launcher): DND command declares duration arg"
```

### Task 3.5: ArgChipBar React component

**Files:**
- Create: `desktop-ui/src/features/launcher/components/ArgChipBar.tsx`
- Create: `desktop-ui/src/features/launcher/components/__tests__/ArgChipBar.test.tsx`

- [ ] **Step 1: Vitest test (failing)**

```tsx
import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { ArgChipBar } from '../ArgChipBar'

const specs = [
  { name: 'duration', placeholder: '30m', kind: { type: 'text' }, required: true },
  { name: 'reason',   placeholder: 'why', kind: { type: 'text' }, required: false },
]

describe('ArgChipBar', () => {
  it('tab moves focus between chips', () => {
    render(<ArgChipBar specs={specs} onSubmit={() => {}} onCancel={() => {}} />)
    const inputs = screen.getAllByRole('textbox')
    inputs[0].focus()
    fireEvent.keyDown(inputs[0], { key: 'Tab' })
    expect(document.activeElement).toBe(inputs[1])
  })

  it('enter on last chip submits', () => {
    const onSubmit = vi.fn()
    render(<ArgChipBar specs={specs} onSubmit={onSubmit} onCancel={() => {}} />)
    const inputs = screen.getAllByRole('textbox')
    fireEvent.change(inputs[0], { target: { value: '30m' } })
    fireEvent.keyDown(inputs[1], { key: 'Enter' })
    expect(onSubmit).toHaveBeenCalledWith({ duration: '30m', reason: '' })
  })

  it('backspace from empty first chip cancels', () => {
    const onCancel = vi.fn()
    render(<ArgChipBar specs={specs} onSubmit={() => {}} onCancel={onCancel} />)
    const inputs = screen.getAllByRole('textbox')
    fireEvent.keyDown(inputs[0], { key: 'Backspace' })
    expect(onCancel).toHaveBeenCalled()
  })
})
```

- [ ] **Step 2: Verify failure**

```bash
cd desktop-ui && bun run test ArgChipBar
```
Expected: file not found.

- [ ] **Step 3: Implement**

```tsx
import React, { useRef, useState } from 'react'

export interface ArgSpec {
  name: string
  placeholder: string
  kind: { type: 'text' } | { type: 'number' } | { type: 'choice'; values: string[] }
  required: boolean
}

interface Props {
  specs: ArgSpec[]
  onSubmit: (args: Record<string, string>) => void
  onCancel: () => void
}

export function ArgChipBar({ specs, onSubmit, onCancel }: Props) {
  const [values, setValues] = useState<Record<string, string>>(() =>
    Object.fromEntries(specs.map((s) => [s.name, ''])),
  )
  const refs = useRef<(HTMLInputElement | null)[]>([])

  const focusAt = (i: number) => refs.current[i]?.focus()

  return (
    <div className="flex items-center gap-2 p-2">
      {specs.map((spec, i) => (
        <input
          key={spec.name}
          ref={(el) => (refs.current[i] = el)}
          type={spec.kind.type === 'number' ? 'number' : 'text'}
          inputMode={spec.kind.type === 'number' ? 'numeric' : undefined}
          placeholder={spec.placeholder}
          value={values[spec.name]}
          onChange={(e) => setValues({ ...values, [spec.name]: e.target.value })}
          onKeyDown={(e) => {
            if (e.key === 'Tab') {
              e.preventDefault()
              focusAt(e.shiftKey ? Math.max(0, i - 1) : Math.min(specs.length - 1, i + 1))
            } else if (e.key === 'Enter') {
              const missing = specs.find((s) => s.required && !values[s.name])
              if (missing) focusAt(specs.indexOf(missing))
              else onSubmit(values)
            } else if (e.key === 'Backspace' && !values[spec.name]) {
              if (i === 0) onCancel()
              else focusAt(i - 1)
            }
          }}
          className="bg-surface-base text-fg px-2 py-1 rounded border border-border min-w-[100px]"
        />
      ))}
    </div>
  )
}
```

- [ ] **Step 4: Pass tests**

```bash
cd desktop-ui && bun run test ArgChipBar
```
Expected: 3 pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/launcher/components/
git commit -m "feat(desktop-ui): ArgChipBar with Tab navigation and submit/cancel"
```

### Task 3.6: Wire ArgChipBar into LauncherPage

**Files:**
- Modify: `desktop-ui/src/features/launcher/pages/LauncherPage.tsx` (or wherever results render)
- Modify: `desktop-ui/src/features/launcher/hooks/useLauncherExecute.ts`

- [ ] **Step 1: Add `argMode` state**

When user selects an item with `arguments.length > 0`, set `argMode = item`. Render `<ArgChipBar specs={item.arguments} onSubmit={(args) => execute(item, args)} onCancel={() => setArgMode(null)} />` instead of results list.

- [ ] **Step 2: Update execute hook to accept args**

```ts
export function useLauncherExecute() {
  return useMutation('launcher_execute', async (input: { item: LauncherItem; args?: Record<string,string> }) => {
    const { item, args } = input
    const result = await invoke<LauncherExecuteResult>('launcher_execute', {
      itemId: item.id, kind: item.kind.type, args: args ?? {},
    })
    // ...badge logic from PR-2
    return result
  })
}
```

- [ ] **Step 3: Build + smoke**

```bash
cd desktop-ui && bun run build
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/launcher/
git commit -m "feat(desktop-ui): switch to ArgChipBar for items with arguments"
```

### Task 3.7: PR-3 gates

- [ ] **Step 1: All gates**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd desktop-ui && bun run lint && bun run test && bun run build
```

- [ ] **Step 2: Manual smoke**

Trigger DND command in launcher, type "2h", press Enter, verify launcher closes and badge shows.

- [ ] **Step 3: PR**

```bash
gh pr create --title "feat(launcher): inline argument UI with ArgChipBar" --body "Implements PR-3."
```

---

# PR-4 — Window-management presets

### Task 4.1: Preset table

**Files:**
- Create: `crates/feature-launcher/src/window_mgmt/presets.rs`
- Modify: `crates/feature-launcher/src/window_mgmt/mod.rs`

- [ ] **Step 1: Create presets table**

```rust
//! Curated window-placement presets. Frame coords are fractions of screen [0..1].

#[derive(Debug, Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    pub display_name: &'static str,
    pub keywords: &'static [&'static str],
    pub frame: PresetFrame,
}

#[derive(Debug, Clone, Copy)]
pub struct PresetFrame { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

pub const PRESETS: &[Preset] = &[
    // Halves
    Preset { name: "left-half",   display_name: "Left Half",   keywords: &["left","half"],     frame: PresetFrame{x:0.0,y:0.0,w:0.5,h:1.0} },
    Preset { name: "right-half",  display_name: "Right Half",  keywords: &["right","half"],    frame: PresetFrame{x:0.5,y:0.0,w:0.5,h:1.0} },
    Preset { name: "top-half",    display_name: "Top Half",    keywords: &["top","half","up"], frame: PresetFrame{x:0.0,y:0.0,w:1.0,h:0.5} },
    Preset { name: "bottom-half", display_name: "Bottom Half", keywords: &["bottom","half","down"], frame: PresetFrame{x:0.0,y:0.5,w:1.0,h:0.5} },
    // Thirds (vertical bands)
    Preset { name: "left-third",   display_name: "Left Third",   keywords: &["left","third","1/3"],  frame: PresetFrame{x:0.0,y:0.0,w:0.333,h:1.0} },
    Preset { name: "center-third", display_name: "Center Third", keywords: &["center","third"],      frame: PresetFrame{x:0.333,y:0.0,w:0.334,h:1.0} },
    Preset { name: "right-third",  display_name: "Right Third",  keywords: &["right","third"],       frame: PresetFrame{x:0.667,y:0.0,w:0.333,h:1.0} },
    Preset { name: "left-two-thirds",  display_name: "Left Two-Thirds",  keywords: &["left","two","thirds","2/3"],  frame: PresetFrame{x:0.0,y:0.0,w:0.667,h:1.0} },
    Preset { name: "right-two-thirds", display_name: "Right Two-Thirds", keywords: &["right","two","thirds","2/3"], frame: PresetFrame{x:0.333,y:0.0,w:0.667,h:1.0} },
    Preset { name: "middle-column",    display_name: "Middle Column (2/3)", keywords: &["middle","column"],       frame: PresetFrame{x:0.166,y:0.0,w:0.667,h:1.0} },
    // Quarters
    Preset { name: "top-left-quarter",     display_name: "Top Left Quarter",     keywords: &["top","left","quarter"],     frame: PresetFrame{x:0.0,y:0.0,w:0.5,h:0.5} },
    Preset { name: "top-right-quarter",    display_name: "Top Right Quarter",    keywords: &["top","right","quarter"],    frame: PresetFrame{x:0.5,y:0.0,w:0.5,h:0.5} },
    Preset { name: "bottom-left-quarter",  display_name: "Bottom Left Quarter",  keywords: &["bottom","left","quarter"],  frame: PresetFrame{x:0.0,y:0.5,w:0.5,h:0.5} },
    Preset { name: "bottom-right-quarter", display_name: "Bottom Right Quarter", keywords: &["bottom","right","quarter"], frame: PresetFrame{x:0.5,y:0.5,w:0.5,h:0.5} },
    // Maximize / center / restore variants
    Preset { name: "maximize",         display_name: "Maximize",         keywords: &["max","maximize","full"],            frame: PresetFrame{x:0.0,y:0.0,w:1.0,h:1.0} },
    Preset { name: "almost-maximize",  display_name: "Almost Maximize",  keywords: &["almost","max"],                     frame: PresetFrame{x:0.025,y:0.025,w:0.95,h:0.95} },
    Preset { name: "center",           display_name: "Center",           keywords: &["center","middle"],                  frame: PresetFrame{x:0.15,y:0.10,w:0.70,h:0.80} },
    Preset { name: "center-large",     display_name: "Center Large",     keywords: &["center","large"],                   frame: PresetFrame{x:0.10,y:0.05,w:0.80,h:0.90} },
    Preset { name: "center-small",     display_name: "Center Small",     keywords: &["center","small"],                   frame: PresetFrame{x:0.25,y:0.20,w:0.50,h:0.60} },
    // Sixths (top/bottom row, 3 columns)
    Preset { name: "top-left-sixth",    display_name: "Top Left Sixth",    keywords: &["top","left","sixth"],    frame: PresetFrame{x:0.0,y:0.0,w:0.333,h:0.5} },
    Preset { name: "top-center-sixth",  display_name: "Top Center Sixth",  keywords: &["top","center","sixth"],  frame: PresetFrame{x:0.333,y:0.0,w:0.334,h:0.5} },
    Preset { name: "top-right-sixth",   display_name: "Top Right Sixth",   keywords: &["top","right","sixth"],   frame: PresetFrame{x:0.667,y:0.0,w:0.333,h:0.5} },
    Preset { name: "bottom-left-sixth", display_name: "Bottom Left Sixth", keywords: &["bottom","left","sixth"], frame: PresetFrame{x:0.0,y:0.5,w:0.333,h:0.5} },
    Preset { name: "bottom-center-sixth", display_name: "Bottom Center Sixth", keywords: &["bottom","center","sixth"], frame: PresetFrame{x:0.333,y:0.5,w:0.334,h:0.5} },
    Preset { name: "bottom-right-sixth", display_name: "Bottom Right Sixth", keywords: &["bottom","right","sixth"], frame: PresetFrame{x:0.667,y:0.5,w:0.333,h:0.5} },
    // Restore = sentinel (handled specially)
    Preset { name: "restore", display_name: "Restore", keywords: &["restore","undo"], frame: PresetFrame{x:0.0,y:0.0,w:0.0,h:0.0} },
];

pub fn lookup(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_frames_in_unit_range() {
        for p in PRESETS {
            if p.name == "restore" { continue; }
            assert!(p.frame.x >= 0.0 && p.frame.x <= 1.0, "{}", p.name);
            assert!(p.frame.w > 0.0 && p.frame.x + p.frame.w <= 1.001, "{}", p.name);
            assert!(p.frame.y >= 0.0 && p.frame.y <= 1.0, "{}", p.name);
            assert!(p.frame.h > 0.0 && p.frame.y + p.frame.h <= 1.001, "{}", p.name);
        }
    }
    #[test]
    fn lookup_known_preset() {
        assert!(lookup("left-half").is_some());
        assert!(lookup("nope").is_none());
    }
    #[test]
    fn count_is_25() {
        assert_eq!(PRESETS.len(), 26); // 25 + restore
    }
}
```

In `crates/feature-launcher/src/window_mgmt/mod.rs`:
```rust
pub mod presets;
pub use presets::{Preset, PresetFrame, PRESETS, lookup as lookup_preset};
```

- [ ] **Step 2: Run tests**

```bash
cargo nextest run -p feature-launcher presets
```
Expected: 3 pass.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/src/window_mgmt/
git commit -m "feat(feature-launcher): 25 window presets (data-driven table)"
```

### Task 4.2: WindowAction::Preset variant + apply_preset

**Files:**
- Modify: `crates/feature-launcher/src/types.rs`
- Modify: `crates/feature-launcher/src/window_mgmt/actions.rs`

- [ ] **Step 1: Add variant**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowAction {
    LeftHalf, RightHalf, TopHalf, BottomHalf,
    LeftThird, CenterThird, RightThird,
    Maximize, Center, Restore,
    Preset(String),  // new
}
```

- [ ] **Step 2: Implement `apply_preset` in actions.rs**

```rust
use crate::window_mgmt::presets::{lookup_preset, PresetFrame};
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct WindowManager {
    pre_preset_cache: Arc<DashMap<String, (i32, i32, i32, i32)>>, // window id → rect
}

impl WindowManager {
    pub fn apply_preset(&self, name: &str) -> Result<(), String> {
        let preset = lookup_preset(name).ok_or_else(|| format!("unknown preset: {name}"))?;
        #[cfg(target_os = "macos")]
        {
            let win = platform_macos::window::get_frontmost_window()
                .map_err(|e| e.to_string())?
                .ok_or("no focused window")?;
            if name == "restore" {
                if let Some(prev) = self.pre_preset_cache.get(&win.id) {
                    return platform_macos::window::set_window_frame(&win, prev.0, prev.1, prev.2, prev.3)
                        .map_err(|e| e.to_string());
                }
                return Ok(());
            }
            let screen = platform_macos::window::get_screen_frame_for_window(&win)
                .map_err(|e| e.to_string())?;
            // Cache current frame for "restore"
            self.pre_preset_cache.insert(
                win.id.clone(),
                (win.x, win.y, win.width, win.height),
            );
            let f = preset.frame;
            let x = screen.x + (f.x * screen.width as f32) as i32;
            let y = screen.y + (f.y * screen.height as f32) as i32;
            let w = (f.w * screen.width as f32) as i32;
            let h = (f.h * screen.height as f32) as i32;
            platform_macos::window::set_window_frame(&win, x, y, w, h)
                .map_err(|e| e.to_string())
        }
        #[cfg(not(target_os = "macos"))]
        { let _ = preset; Err("unsupported on this platform".into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_mgmt::presets::lookup_preset;

    #[test]
    fn preset_frame_math_left_half_on_1920_1080() {
        let p = lookup_preset("left-half").unwrap();
        let screen_w = 1920.0_f32;
        let screen_h = 1080.0_f32;
        let w = (p.frame.w * screen_w) as i32;
        let h = (p.frame.h * screen_h) as i32;
        assert_eq!(w, 960);
        assert_eq!(h, 1080);
    }
}
```

If `platform_macos::window::get_screen_frame_for_window` doesn't exist, add it (or use `get_screen_frame()` which returns the main display, accepting that as a known limitation in v1).

- [ ] **Step 3: Build + test**

```bash
cargo nextest run -p feature-launcher window_mgmt
cargo build -p feature-launcher
```

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/src/types.rs crates/feature-launcher/src/window_mgmt/actions.rs
git commit -m "feat(feature-launcher): WindowAction::Preset variant + apply_preset"
```

### Task 4.3: WindowPresetsSource

**Files:**
- Create: `crates/feature-launcher/src/search/window_presets.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Implement source**

```rust
use crate::search::SearchSource;
use crate::types::*;
use crate::window_mgmt::presets::PRESETS;
use async_trait::async_trait;

#[derive(Default, Clone)]
pub struct WindowPresetsSource;

#[async_trait]
impl SearchSource for WindowPresetsSource {
    fn name(&self) -> &'static str { "window_presets" }
    fn prefix(&self) -> Option<&'static str> { Some("w/") }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            // empty query under prefix → list all
            return PRESETS.iter().take(limit).map(item_for).collect();
        }
        let mut scored: Vec<(i32, &'static crate::window_mgmt::presets::Preset)> =
            PRESETS.iter().filter_map(|p| {
                let dn = p.display_name.to_lowercase();
                let mut score = 0;
                if dn.contains(&q) { score += 50; }
                for kw in p.keywords {
                    if kw.starts_with(&q) { score += 30; }
                    else if kw.contains(&q) { score += 10; }
                }
                if score > 0 { Some((score, p)) } else { None }
            }).collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().take(limit).map(|(_, p)| item_for(p)).collect()
    }
}

fn item_for(p: &'static crate::window_mgmt::presets::Preset) -> LauncherItem {
    LauncherItem {
        id: format!("preset:{}", p.name),
        title: p.display_name.into(),
        subtitle: Some("Window".into()),
        icon: Some("window".into()),
        kind: LauncherItemKind::WindowAction { action: WindowAction::Preset(p.name.into()) },
        score: 1.0,
        no_view: true,
        arguments: vec![],
    }
}
```

(Note: `LauncherItemKind::WindowAction` may not yet be a variant — add it if missing, or wrap presets in a new variant `Preset { name: String }`. Verify against current `types.rs`.)

In `mod.rs`:
```rust
pub mod window_presets;
pub use window_presets::WindowPresetsSource;
```

- [ ] **Step 2: Test**

```rust
#[tokio::test]
async fn search_left_returns_left_presets() {
    let src = WindowPresetsSource;
    let items = src.search("left", 20).await;
    assert!(items.iter().any(|i| i.title == "Left Half"));
    assert!(items.iter().any(|i| i.title == "Left Third"));
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p feature-launcher window_presets
git add -A
git commit -m "feat(feature-launcher): WindowPresetsSource with w/ prefix"
```

### Task 4.4: Wire into config + app-core init

**Files:**
- Modify: `crates/config/src/schema/launcher.rs`
- Modify: `crates/app-core/src/init/launcher.rs`

- [ ] **Step 1: Add config flag**

```rust
#[serde(default = "default_true")]
pub window_presets: bool,
```

- [ ] **Step 2: Wire source**

```rust
if cfg.sources.window_presets {
    sources.push(Arc::new(WindowPresetsSource::default()));
}
```

- [ ] **Step 3: Build + commit**

```bash
cargo build -p app-core
git add -A
git commit -m "feat(launcher): wire WindowPresetsSource into init"
```

### Task 4.5: Engine dispatch for `WindowAction::Preset`

**Files:**
- Modify: `crates/app-core/src/handlers/launcher/engine.rs`

- [ ] **Step 1: Locate execute dispatch**

Find the `match item_kind` in the engine. Add arm:
```rust
LauncherItemKind::WindowAction { action } => {
    match action {
        WindowAction::Preset(name) => {
            self.window_manager.apply_preset(name)
                .map(|_| LauncherExecuteResult::default())
                .unwrap_or_else(|e| LauncherExecuteResult::err(e))
        }
        // existing variants...
    }
}
```

- [ ] **Step 2: Test + lint**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(launcher): dispatch WindowAction::Preset to WindowManager::apply_preset"
```

### Task 4.6: PR-4 gates

- [ ] **Step 1: All gates**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd desktop-ui && bun run lint && bun run test && bun run build
```

- [ ] **Step 2: Manual smoke**

In the launcher type `w/left half`, press Enter, verify focused window snaps to left half and no panel remains open.

- [ ] **Step 3: PR**

```bash
gh pr create --title "feat(launcher): 25 window presets via WindowPresetsSource (w/ prefix)" --body "Implements PR-4."
```

---

## Self-Review Checklist (engineer to confirm before each PR ships)

- [ ] All `# arg:` placeholders replaced with real code (no TODOs).
- [ ] Type names in later tasks match earlier definitions (e.g. `LauncherExecuteResult` vs `ExecuteResult`).
- [ ] Every test step shows the actual test code.
- [ ] `DEV_COMMANDS` updated whenever a new `#[tauri::command]` lands (CLAUDE.md gotcha).
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` is zero.
- [ ] No `unwrap()` in production code paths (use `?` or `unwrap_or_else` with logging).
- [ ] Migrations: none required for any of the 4 PRs (verified per spec).

---

## Acceptance criteria (from spec)

- File search latency on 500K-entry corpus: P95 ≤ 20 ms — check via criterion bench in PR-1.
- mdfind fallback returns at least one privacy-excluded path that local index misses — manual check.
- DND command with `2h` arg via inline chip closes launcher and shows "DND on for 2h" badge — manual check after PR-3.
- Typing `w/left half` and pressing Enter snaps focused window to left half of current screen, no panel left open — manual check after PR-4.
- All 4 PRs ship independently green; main branch never broken.
