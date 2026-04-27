//! Bounded LRU of parsed symbol vectors. Keyed by `(path, content_hash)`.
//!
//! Threading: `parking_lot::Mutex` wraps an `lru::LruCache`. The Distiller
//! shares one cache across all turns; cache cap defaults to 256 entries.

use crate::scope::AnchoredSymbol;
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;
use std::path::PathBuf;

type Key = (PathBuf, [u8; 32]);

/// LRU cache of extracted symbol vectors.
#[derive(Debug)]
pub struct SymbolCache {
    inner: Mutex<LruCache<Key, Vec<AnchoredSymbol>>>,
}

impl SymbolCache {
    /// Construct with a max-entry cap (clamped to ≥ 1).
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        let cap = NonZeroUsize::new(cap.max(1)).expect("cap ≥ 1");
        Self {
            inner: Mutex::new(LruCache::new(cap)),
        }
    }

    /// Cache miss → `None`. Cache hit promotes the entry.
    pub fn get(&self, path: &PathBuf, content_hash: &[u8; 32]) -> Option<Vec<AnchoredSymbol>> {
        let key = (path.clone(), *content_hash);
        self.inner.lock().get(&key).cloned()
    }

    /// Insert. May evict the oldest entry.
    pub fn insert(&self, path: PathBuf, content_hash: [u8; 32], symbols: Vec<AnchoredSymbol>) {
        self.inner.lock().put((path, content_hash), symbols);
    }
}

impl Default for SymbolCache {
    fn default() -> Self {
        Self::with_capacity(256)
    }
}
