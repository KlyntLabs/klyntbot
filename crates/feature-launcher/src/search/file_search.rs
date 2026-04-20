use crate::search::inverted_index::{classify_extension, InvertedFileIndex, SkipSet};
use crate::types::*;
use arc_swap::ArcSwap;
use std::collections::HashSet;
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
        let roots: Vec<PathBuf> = scan_dirs
            .iter()
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
        let mut next = current.clone_for_patch();
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
pub enum FsEventKind {
    Create,
    Remove,
}

#[async_trait::async_trait]
impl super::SearchSource for FileSearchSource {
    fn name(&self) -> &'static str {
        "files"
    }

    fn prefix(&self) -> Option<&'static str> {
        Some("f/")
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }
        // Cancel any prior in-flight mdfind for this source
        let cancel = {
            let mut g = self.cancel.lock();
            g.cancel();
            *g = CancellationToken::new();
            g.clone()
        };
        let snap = self.index.load_full();
        let scored = snap.search(query, limit);

        let mut items: Vec<LauncherItem> = scored
            .iter()
            .map(|s| {
                let e = &snap.entries[s.entry_idx as usize];
                let path_str = e.path.display().to_string();
                LauncherItem {
                    id: format!("file:{path_str}"),
                    title: e.name.clone(),
                    subtitle: Some(path_str),
                    icon: Some("file".to_string()),
                    kind: LauncherItemKind::File {
                        path: e.path.clone(),
                        kind: e.kind,
                    },
                    score: (s.score as f64 / 200.0) * 0.85,
                    no_view: false,
                    arguments: vec![],
                }
            })
            .collect();

        // mdfind fallback
        #[cfg(target_os = "macos")]
        if self.mdfind_fallback && items.len() < self.mdfind_threshold {
            let extra =
                crate::search::inverted_index::mdfind_paths(query, &self.roots, &self.skip, cancel)
                    .await;
            let existing: HashSet<PathBuf> = items
                .iter()
                .filter_map(|i| match &i.kind {
                    LauncherItemKind::File { path, .. } => Some(path.clone()),
                    _ => None,
                })
                .collect();
            for p in extra {
                if existing.contains(&p) {
                    continue;
                }
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                let path_str = p.display().to_string();
                items.push(LauncherItem {
                    id: format!("file:{path_str}"),
                    title: name,
                    subtitle: Some(path_str),
                    icon: Some("file".to_string()),
                    kind: LauncherItemKind::File {
                        path: p.clone(),
                        kind: classify_extension(&p),
                    },
                    score: 0.40,
                    no_view: false,
                    arguments: vec![],
                });
            }
            items.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            items.truncate(limit);
        }

        items
    }

    async fn refresh(&self) {
        let roots = self.roots.clone();
        let skip = self.skip.clone();
        let cap = self.max_entries;
        let new = tokio::task::spawn_blocking(move || InvertedFileIndex::build(&roots, &skip, cap))
            .await
            .unwrap_or_else(|e| {
                tracing::error!("inverted index build failed: {e}");
                InvertedFileIndex::empty()
            });
        tracing::info!("Indexed {} files (inverted)", new.len());
        self.index.store(Arc::new(new));
    }
}
