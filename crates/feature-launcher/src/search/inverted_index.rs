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
