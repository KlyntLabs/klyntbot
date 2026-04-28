//! `SymbolExtractor` trait + tree-sitter-backed `TreeSitterExtractor`.

use crate::scope::AnchoredSymbol;
use crate::symbols::{Language, SymbolCache};
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

/// Extract symbol anchors from source. Implementations are `Send + Sync`
/// since the Distiller and Reforge symbol-validation phase share one instance.
pub trait SymbolExtractor: Send + Sync + std::fmt::Debug {
    /// Extract anchors for the given file. Returns an empty vec when language
    /// is unsupported or parsing fails — never errors.
    fn extract(&self, path: &Path, source: &str, git_hash: &str) -> Vec<AnchoredSymbol>;
}

/// Tree-sitter–backed extractor with a bounded LRU cache.
#[derive(Debug, Default)]
pub struct TreeSitterExtractor {
    cache: SymbolCache,
}

impl TreeSitterExtractor {
    /// Construct with default cache capacity (256 entries).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct with a custom cache capacity.
    #[must_use]
    pub fn with_cache_capacity(cap: usize) -> Self {
        Self {
            cache: SymbolCache::with_capacity(cap),
        }
    }

    fn content_hash(source: &str) -> [u8; 32] {
        *blake3::hash(source.as_bytes()).as_bytes()
    }
}

impl SymbolExtractor for TreeSitterExtractor {
    fn extract(&self, path: &Path, source: &str, git_hash: &str) -> Vec<AnchoredSymbol> {
        let hash = Self::content_hash(source);
        if let Some(hit) = self.cache.get(path, &hash) {
            return hit;
        }

        let Some(lang) = Language::from_path(path) else {
            return Vec::new();
        };
        let mut parser = Parser::new();
        let ts_lang: tree_sitter::Language = match lang {
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::Go => tree_sitter_go::LANGUAGE.into(),
        };
        if parser.set_language(&ts_lang).is_err() {
            return Vec::new();
        }
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        let query_src = match lang {
            Language::Rust => include_str!("queries/rust.scm"),
            Language::TypeScript => include_str!("queries/typescript.scm"),
            Language::JavaScript => include_str!("queries/javascript.scm"),
            Language::Python => include_str!("queries/python.scm"),
            Language::Go => include_str!("queries/go.scm"),
        };
        let Ok(query) = Query::new(&ts_lang, query_src) else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        let mut out = Vec::new();
        let bytes = source.as_bytes();
        let symbol_idx = match query.capture_index_for_name("symbol") {
            Some(i) => i,
            None => return Vec::new(),
        };
        let mut matches = cursor.matches(&query, tree.root_node(), bytes);
        while let Some(m) = matches.next() {
            let symbol_node = m
                .captures
                .iter()
                .find(|c| c.index == symbol_idx)
                .map(|c| c.node);
            let kind_capture = m.captures.iter().find(|c| c.index != symbol_idx).map(|c| {
                query
                    .capture_names()
                    .get(c.index as usize)
                    .copied()
                    .unwrap_or("symbol")
            });
            let Some(name_node) = symbol_node else {
                continue;
            };
            let name = match name_node.utf8_text(bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };
            let kind = kind_capture.unwrap_or("symbol").to_string();
            let parent_node = name_node.parent().unwrap_or_else(|| tree.root_node());
            let span_start = u32::try_from(parent_node.start_byte()).unwrap_or(u32::MAX);
            let span_end = u32::try_from(parent_node.end_byte()).unwrap_or(u32::MAX);
            out.push(AnchoredSymbol {
                file_path: path.to_path_buf(),
                symbol: name,
                kind,
                git_hash: git_hash.to_string(),
                byte_span: Some((span_start, span_end)),
            });
        }
        self.cache.insert(path.to_path_buf(), hash, out.clone());
        out
    }
}
