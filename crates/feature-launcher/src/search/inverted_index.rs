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
}
