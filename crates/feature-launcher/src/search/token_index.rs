use fst::{IntoStreamer, Map, MapBuilder, Streamer};
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;

const DELTA_FLUSH_THRESHOLD: usize = 256;

#[derive(Clone)]
pub(crate) struct TokenIndex {
    /// Token bytes → posting list id.
    fst: Map<Vec<u8>>,
    /// One bitmap per terminal node in the FST.
    postings: Vec<RoaringBitmap>,
    /// IDF weight per token id (parallel to `postings`).
    idf: Vec<f32>,
    /// Pending insertions waiting for compaction.
    delta: FxHashMap<SmolStr, RoaringBitmap>,
    /// Removed doc ids that haven't been compacted yet.
    tombstones: RoaringBitmap,
    /// Number of pending delta insertions.
    delta_pending: usize,
    /// Pre-computed short-prefix cache for 1-2 char queries.
    short_prefix_cache: FxHashMap<SmolStr, (RoaringBitmap, f32)>,
}

impl TokenIndex {
    pub fn build(tokens: Vec<(SmolStr, RoaringBitmap, f32)>) -> Self {
        // Collect token strings before moving tokens.
        let all_tokens: Vec<String> = tokens.iter().map(|(t, _, _)| t.to_string()).collect();

        let mut sorted = tokens;
        sorted.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

        let mut builder = MapBuilder::memory();
        let mut postings = Vec::with_capacity(sorted.len());
        let mut idf = Vec::with_capacity(sorted.len());

        for (idx, (token, bitmap, weight)) in sorted.into_iter().enumerate() {
            builder.insert(token.as_bytes(), idx as u64).unwrap();
            postings.push(bitmap);
            idf.push(weight);
        }

        let fst = builder.into_map();

        let mut short_prefix_cache: FxHashMap<SmolStr, (RoaringBitmap, f32)> = FxHashMap::default();
        // Pre-compute 1-2 char prefixes that match at least one token.
        for token in &all_tokens {
            for prefix_len in 1..=2 {
                if token.len() < prefix_len {
                    continue;
                }
                let prefix = SmolStr::new(&token[..prefix_len]);
                if short_prefix_cache.contains_key(&prefix) {
                    continue;
                }
                let mut union = RoaringBitmap::new();
                let mut idf_min = f32::MAX;
                let mut stream = fst.range().ge(prefix.as_bytes()).into_stream();
                while let Some((key, id)) = stream.next() {
                    if !key.starts_with(prefix.as_bytes()) {
                        break;
                    }
                    let i = id as usize;
                    union |= &postings[i];
                    idf_min = idf_min.min(idf[i]);
                }
                if !union.is_empty() {
                    short_prefix_cache.insert(prefix, (union, idf_min));
                }
            }
        }

        Self {
            fst,
            postings,
            idf,
            delta: FxHashMap::default(),
            tombstones: RoaringBitmap::new(),
            delta_pending: 0,
            short_prefix_cache,
        }
    }

    /// Returns a bitmap of doc ids whose tokens match the given query,
    /// plus the minimum IDF weight among matching tokens.
    ///
    /// Short queries (< 3 chars) do a trie range walk + union.
    /// Longer queries first try exact match, falling back to range walk.
    pub fn candidates_for(&self, q: &str) -> Option<(RoaringBitmap, f32)> {
        self.candidates_for_cow(q).map(|(cow, w)| (cow.into_owned(), w))
    }

    /// Like `candidates_for` but returns a `Cow` to avoid cloning when
    /// no mutation (delta/tombstone application) is required.
    pub fn candidates_for_cow<'a>(&'a self, q: &str) -> Option<(std::borrow::Cow<'a, RoaringBitmap>, f32)> {
        use std::borrow::Cow;

        // Fast path: pre-computed short prefixes (1-2 chars) when no delta/tombstones.
        if q.len() <= 2 && self.delta.is_empty() && self.tombstones.is_empty() {
            if let Some(cached) = self.short_prefix_cache.get(&SmolStr::new(q)) {
                return Some((Cow::Borrowed(&cached.0), cached.1));
            }
        }

        if q.len() >= 3 {
            if let Some(id) = self.fst.get(q.as_bytes()) {
                let i = id as usize;
                if self.tombstones.is_empty() && self.delta.is_empty() {
                    return Some((Cow::Borrowed(&self.postings[i]), self.idf[i]));
                }
                let mut bitmap = self.postings[i].clone();
                bitmap -= &self.tombstones;
                if bitmap.is_empty() {
                    return None;
                }
                return Some((Cow::Owned(bitmap), self.idf[i]));
            }
        }

        // Range walk: union all postings under `q..` until first key
        // that no longer starts with q.
        let mut union = RoaringBitmap::new();
        let mut idf_min = f32::MAX;
        let mut stream = self.fst.range().ge(q.as_bytes()).into_stream();
        while let Some((key, id)) = stream.next() {
            if !key.starts_with(q.as_bytes()) {
                break;
            }
            let i = id as usize;
            union |= &self.postings[i];
            idf_min = idf_min.min(self.idf[i]);
        }

        // Merge in pending delta entries.
        for (token, bitmap) in &self.delta {
            if token.starts_with(q) {
                union |= bitmap;
            }
        }

        union -= &self.tombstones;

        if union.is_empty() {
            None
        } else {
            Some((Cow::Owned(union), idf_min))
        }
    }

    pub fn insert_pending(&mut self, token: SmolStr, doc: u32) {
        self.delta.entry(token).or_default().insert(doc);
        self.delta_pending += 1;
    }

    pub fn remove_doc(&mut self, doc: u32) {
        self.tombstones.insert(doc);
    }

    pub fn should_compact(&self) -> bool {
        self.delta_pending >= DELTA_FLUSH_THRESHOLD || !self.tombstones.is_empty()
    }

    pub fn compact(&mut self, n_entries: usize) {
        if self.delta.is_empty() && self.tombstones.is_empty() {
            return;
        }

        // Merge delta into existing tokens.
        let mut merged: FxHashMap<SmolStr, RoaringBitmap> = FxHashMap::default();

        // Start with existing FST tokens.
        let mut stream = self.fst.stream();
        while let Some((key, id)) = stream.next() {
            let token = SmolStr::new(std::str::from_utf8(key).unwrap_or(""));
            let i = id as usize;
            let mut bitmap = self.postings[i].clone();
            bitmap -= &self.tombstones;
            if let Some(delta_bitmap) = self.delta.remove(&token) {
                bitmap |= delta_bitmap;
            }
            if !bitmap.is_empty() {
                merged.insert(token, bitmap);
            }
        }

        // Add remaining delta tokens that weren't in the FST.
        for (token, bitmap) in self.delta.drain() {
            let mut bitmap = bitmap;
            bitmap -= &self.tombstones;
            if !bitmap.is_empty() {
                *merged.entry(token).or_default() |= bitmap;
            }
        }

        // Rebuild FST and postings.
        let n = n_entries as f32;
        let mut tokens: Vec<(SmolStr, RoaringBitmap, f32)> = merged
            .into_iter()
            .map(|(token, bitmap)| {
                let df = bitmap.len() as f32;
                let w = ((n - df + 0.5) / (df + 0.5) + 1.0).ln().max(0.05);
                (token, bitmap, w)
            })
            .collect();
        tokens.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

        // Save token strings before consuming tokens.
        let token_strings: Vec<String> = tokens.iter().map(|(t, _, _)| t.to_string()).collect();

        let mut builder = MapBuilder::memory();
        let mut postings = Vec::with_capacity(tokens.len());
        let mut idf = Vec::with_capacity(tokens.len());

        for (idx, (token, bitmap, weight)) in tokens.into_iter().enumerate() {
            builder.insert(token.as_bytes(), idx as u64).unwrap();
            postings.push(bitmap);
            idf.push(weight);
        }

        self.fst = builder.into_map();
        self.postings = postings;
        self.idf = idf;
        self.delta.clear();
        self.delta_pending = 0;
        self.tombstones.clear();

        // Rebuild short-prefix cache with new postings.
        let mut short_prefix_cache: FxHashMap<SmolStr, (RoaringBitmap, f32)> = FxHashMap::default();
        for token in &token_strings {
            for prefix_len in 1..=2 {
                if token.len() < prefix_len {
                    continue;
                }
                let prefix = SmolStr::new(&token[..prefix_len]);
                if short_prefix_cache.contains_key(&prefix) {
                    continue;
                }
                let mut union = RoaringBitmap::new();
                let mut idf_min = f32::MAX;
                let mut stream = self.fst.range().ge(prefix.as_bytes()).into_stream();
                while let Some((key, id)) = stream.next() {
                    if !key.starts_with(prefix.as_bytes()) {
                        break;
                    }
                    let i = id as usize;
                    union |= &self.postings[i];
                    idf_min = idf_min.min(self.idf[i]);
                }
                if !union.is_empty() {
                    short_prefix_cache.insert(prefix, (union, idf_min));
                }
            }
        }
        self.short_prefix_cache = short_prefix_cache;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_lookup_exact() {
        let tokens = vec![
            (SmolStr::new("hello"), RoaringBitmap::from_iter([1, 2]), 1.0),
            (SmolStr::new("world"), RoaringBitmap::from_iter([2, 3]), 1.0),
        ];
        let idx = TokenIndex::build(tokens);
        let (bitmap, _) = idx.candidates_for("hello").unwrap();
        assert_eq!(bitmap.iter().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn range_walk_unions_prefixes() {
        let tokens = vec![
            (SmolStr::new("report"), RoaringBitmap::from_iter([1]), 2.0),
            (SmolStr::new("readme"), RoaringBitmap::from_iter([2]), 1.5),
            (SmolStr::new("world"), RoaringBitmap::from_iter([3]), 1.0),
        ];
        let idx = TokenIndex::build(tokens);
        let (bitmap, idf_min) = idx.candidates_for("re").unwrap();
        assert_eq!(bitmap.iter().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(idf_min, 1.5);
    }

    #[test]
    fn delta_merged_in_search() {
        let tokens = vec![
            (SmolStr::new("report"), RoaringBitmap::from_iter([1]), 1.0),
        ];
        let mut idx = TokenIndex::build(tokens);
        idx.insert_pending(SmolStr::new("report"), 2);
        idx.insert_pending(SmolStr::new("readme"), 3);
        let (bitmap, _) = idx.candidates_for("re").unwrap();
        assert_eq!(bitmap.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn compaction_roundtrip() {
        let tokens = vec![
            (SmolStr::new("report"), RoaringBitmap::from_iter([1, 2]), 1.0),
            (SmolStr::new("readme"), RoaringBitmap::from_iter([2, 3]), 1.0),
        ];
        let mut idx = TokenIndex::build(tokens);
        idx.insert_pending(SmolStr::new("report"), 4);
        idx.remove_doc(2);
        idx.compact(5);

        let (bitmap, _) = idx.candidates_for("re").unwrap();
        assert_eq!(bitmap.iter().collect::<Vec<_>>(), vec![1, 3, 4]);
    }

    #[test]
    fn tombstones_filter_exact_match() {
        let tokens = vec![
            (SmolStr::new("report"), RoaringBitmap::from_iter([1, 2]), 1.0),
        ];
        let mut idx = TokenIndex::build(tokens);
        idx.remove_doc(2);
        let (bitmap, _) = idx.candidates_for("report").unwrap();
        assert_eq!(bitmap.iter().collect::<Vec<_>>(), vec![1]);
    }
}
