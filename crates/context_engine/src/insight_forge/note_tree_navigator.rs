use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::DomainSearcher;
use crate::book_index::tree::BookTreeRepo;
use crate::memory_retriever::{MemoryEntry, MemorySource};

// ---------------------------------------------------------------------------
// Embedding search trait (dependency inversion: L3 can't import L5)
// ---------------------------------------------------------------------------

/// A single vector-search hit against the tree_node_embeddings table.
#[derive(Debug, Clone)]
pub struct TreeNodeHit {
    pub node_id: String,
    pub note_id: String,
    pub similarity: f64,
}

/// Trait abstracting embedding operations for tree nodes.
///
/// Implemented in higher layers (cognitive / agent) and injected at startup.
#[async_trait]
pub trait TreeNodeEmbeddingSearch: Send + Sync {
    /// Search the `tree_node_embeddings` LanceDB table.
    async fn search_tree_nodes(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f64,
        note_id_filter: Option<&str>,
    ) -> common::Result<Vec<TreeNodeHit>>;

    /// Embed a text query into a vector.
    async fn embed_query(&self, query: &str) -> common::Result<Vec<f32>>;
}

// ---------------------------------------------------------------------------
// Community search traits (dependency inversion: L3 can't import L5)
// ---------------------------------------------------------------------------

/// A single vector-search hit against the community_embeddings table.
#[derive(Debug, Clone)]
pub struct CommunityHit {
    pub community_id: String,
    pub member_count: usize,
    pub source_note_count: usize,
    pub score: f64,
}

/// Trait abstracting embedding operations for communities.
#[async_trait]
pub trait CommunityEmbeddingSearch: Send + Sync {
    async fn search_communities(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f64,
    ) -> common::Result<Vec<CommunityHit>>;
}

/// Metadata about a community, loaded from SQLite.
#[derive(Debug, Clone)]
pub struct CommunityInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub top_entities: Vec<String>,
    pub representative_paths: Vec<String>,
    pub source_note_count: usize,
    pub stability: f64,
}

/// A community member (tree node + membership score).
#[derive(Debug, Clone)]
pub struct CommunityMember {
    pub tree_node_id: String,
    pub membership_score: f64,
}

/// Trait abstracting community data loading from SQLite.
#[async_trait]
pub trait CommunityMemberLoader: Send + Sync {
    async fn get_community_members(
        &self,
        community_id: &str,
    ) -> common::Result<Vec<CommunityMember>>;

    async fn get_community_info(&self, community_id: &str)
        -> common::Result<Option<CommunityInfo>>;
}

// ---------------------------------------------------------------------------
// Query classification
// ---------------------------------------------------------------------------

const COMMUNITY_KEYWORDS: &[&str] = &[
    "across my notes",
    "connect",
    "related to",
    "how does",
    "relate to",
    "community",
    "cluster",
    "summarize my thoughts on",
    "everything about",
    "all my notes about",
    "cross-note",
];

const HIERARCHICAL_KEYWORDS: &[&str] = &[
    "section",
    "part",
    "chapter",
    "heading",
    "paragraph",
    "in my note",
    "in the note",
    "from the note",
    "note about",
    "notebook",
    "under the",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryType {
    /// No hierarchical keywords detected.
    Simple,
    /// Hierarchical keywords present, no active task context.
    Hierarchical,
    /// Hierarchical keywords present AND an active task is set.
    Hybrid,
    /// Cross-note synthesis / community traversal keywords.
    Community,
}

/// Classify a query into one of the four retrieval paths.
///
/// `active_task` is an optional description / title of the user's current task
/// (provided via session context).  When present and keywords match, the
/// classifier returns `Hybrid` so both simple and hierarchical paths run.
pub fn classify_query(query: &str, active_task: Option<&str>) -> QueryType {
    let lower = query.to_lowercase();

    // Community keywords take priority (cross-note synthesis intent).
    let has_community = COMMUNITY_KEYWORDS.iter().any(|kw| lower.contains(kw));
    if has_community {
        return QueryType::Community;
    }

    let has_keyword = HIERARCHICAL_KEYWORDS.iter().any(|kw| lower.contains(kw));

    if !has_keyword {
        return QueryType::Simple;
    }

    match active_task {
        Some(task) if !task.is_empty() => QueryType::Hybrid,
        _ => QueryType::Hierarchical,
    }
}

// ---------------------------------------------------------------------------
// NoteTreeNavigator
// ---------------------------------------------------------------------------

/// `DomainSearcher` implementation that provides 3-path retrieval over the
/// hierarchical note tree.
///
/// - **Simple** (~65%): embed query -> vector search on tree_node_embeddings
///   -> load ancestor chain -> return with path context.
/// - **Hierarchical** (~15%): coarse vector search (top_k=10) -> identify
///   candidate notes -> load subtrees -> score nodes by vector similarity +
///   FTS keyword boost -> select best paths -> RRF merge.
/// - **Hybrid** (~20%): run both Simple and Hierarchical concurrently, merge
///   via RRF, dedup by node_id.
pub struct NoteTreeNavigator {
    tree_repo: Arc<dyn BookTreeRepo>,
    embedding_search: Arc<dyn TreeNodeEmbeddingSearch>,
    /// Optional active task description used for query classification.
    active_task: Option<String>,
    /// When None, community queries fall back to hybrid.
    community_search: Option<Arc<dyn CommunityEmbeddingSearch>>,
    community_loader: Option<Arc<dyn CommunityMemberLoader>>,
}

impl NoteTreeNavigator {
    pub fn new(
        tree_repo: Arc<dyn BookTreeRepo>,
        embedding_search: Arc<dyn TreeNodeEmbeddingSearch>,
        active_task: Option<String>,
    ) -> Self {
        Self {
            tree_repo,
            embedding_search,
            active_task,
            community_search: None,
            community_loader: None,
        }
    }

    /// Inject community search capabilities (Phase 2).
    pub fn with_community_search(
        mut self,
        community_search: Arc<dyn CommunityEmbeddingSearch>,
        community_loader: Arc<dyn CommunityMemberLoader>,
    ) -> Self {
        self.community_search = Some(community_search);
        self.community_loader = Some(community_loader);
        self
    }

    /// Update the active task context (e.g. when the user switches tasks).
    pub fn set_active_task(&mut self, task: Option<String>) {
        self.active_task = task;
    }

    // -----------------------------------------------------------------------
    // Path 1: Simple retrieval
    // -----------------------------------------------------------------------

    async fn simple_search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let embedding = match self.embedding_search.embed_query(query).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("NoteTreeNavigator: embed_query failed: {e}");
                return vec![];
            }
        };

        let hits = match self
            .embedding_search
            .search_tree_nodes(&embedding, limit, 0.3, None)
            .await
        {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("NoteTreeNavigator: vector search failed: {e}");
                return vec![];
            }
        };

        let mut entries = Vec::with_capacity(hits.len());
        for hit in hits {
            let entry = self.hit_to_entry(&hit).await;
            if let Some(e) = entry {
                entries.push(e);
            }
        }
        entries
    }

    // -----------------------------------------------------------------------
    // Path 2: Hierarchical retrieval
    // -----------------------------------------------------------------------

    async fn hierarchical_search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        // Step 1: Coarse vector search to identify candidate notes.
        let embedding = match self.embedding_search.embed_query(query).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("NoteTreeNavigator: embed_query failed: {e}");
                return vec![];
            }
        };

        let coarse_hits = match self
            .embedding_search
            .search_tree_nodes(&embedding, 10, 0.2, None)
            .await
        {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("NoteTreeNavigator: coarse vector search failed: {e}");
                return vec![];
            }
        };

        if coarse_hits.is_empty() {
            return vec![];
        }

        // Build a map of note_id -> best similarity from the coarse pass.
        let mut note_similarities: HashMap<String, f64> = HashMap::new();
        for hit in &coarse_hits {
            note_similarities
                .entry(hit.note_id.clone())
                .and_modify(|s| {
                    if hit.similarity > *s {
                        *s = hit.similarity;
                    }
                })
                .or_insert(hit.similarity);
        }

        // Also build a set of node_id -> similarity for vector score lookup.
        let vector_scores: HashMap<String, f64> = coarse_hits
            .iter()
            .map(|h| (h.node_id.clone(), h.similarity))
            .collect();

        // Step 2: FTS keyword boost — search the tree FTS index.
        let fts_results = match self.tree_repo.search_fts(query, 20).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("NoteTreeNavigator: FTS search failed: {e}");
                vec![]
            }
        };

        let fts_node_ids: HashSet<String> = fts_results.iter().map(|n| n.id.clone()).collect();

        // Step 3: For each candidate note, load the subtree from the
        // top-scoring node and score nodes by vector similarity + FTS boost.
        let mut scored_entries: Vec<MemoryEntry> = Vec::new();

        for hit in &coarse_hits {
            // Load the subtree rooted at this node (includes children).
            let subtree = match self.tree_repo.get_subtree(&hit.node_id).await {
                Ok(t) => t,
                Err(_) => continue,
            };

            for node in subtree {
                let vector_score = vector_scores.get(&node.id).copied().unwrap_or(0.0);
                let fts_boost = if fts_node_ids.contains(&node.id) {
                    0.15
                } else {
                    0.0
                };
                let combined = vector_score + fts_boost;

                if combined < 0.2 {
                    continue;
                }

                let path = self.build_path(&node.id).await;
                let path_str = path.join(" > ");

                let content = format!("[{}] {}", path_str, node.content);
                let metadata = json!({
                    "node_id": node.id,
                    "note_id": node.source_id,
                    "level": node.level,
                    "path": path,
                });

                scored_entries.push(MemoryEntry {
                    id: node.id.clone(),
                    content,
                    score: combined,
                    source: MemorySource::Domain {
                        name: "note_tree".into(),
                    },
                    raw_score: combined,
                });

                // Attach metadata via the id (we'll set it properly below).
                // Since MemoryEntry doesn't have a metadata field, we encode
                // the path information directly in the content string.
                let _ = metadata; // metadata is encoded in content above
            }
        }

        // Step 4: Dedup by node_id, keeping highest score.
        scored_entries = dedup_by_node_id(scored_entries);

        // Sort by score descending.
        scored_entries.sort_by(|a, b| b.score.total_cmp(&a.score));
        scored_entries.truncate(limit);

        scored_entries
    }

    // -----------------------------------------------------------------------
    // Path 3: Hybrid retrieval
    // -----------------------------------------------------------------------

    async fn hybrid_search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        // Run both paths concurrently.
        let (simple, hierarchical) = tokio::join!(
            self.simple_search(query, limit),
            self.hierarchical_search(query, limit),
        );

        // RRF merge.
        let merged = super::rrf_merge(&[simple, hierarchical], limit);

        // Dedup by node_id.
        let deduped = dedup_by_node_id(merged);
        deduped.into_iter().take(limit).collect()
    }

    // -----------------------------------------------------------------------
    // Path 4: Community traversal
    // -----------------------------------------------------------------------

    async fn community_search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let (Some(community_search), Some(community_loader)) =
            (&self.community_search, &self.community_loader)
        else {
            // Fall back to hybrid if community search not wired.
            return self.hybrid_search(query, limit).await;
        };

        // 1. Embed query
        let embedding = match self.embedding_search.embed_query(query).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("NoteTreeNavigator: embed_query failed: {e}");
                return vec![];
            }
        };

        // 2. Search community embeddings
        let community_hits = match community_search
            .search_communities(&embedding, 5, 0.3)
            .await
        {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("NoteTreeNavigator: community search failed: {e}");
                return vec![];
            }
        };

        if community_hits.is_empty() {
            // Fall back to hybrid if no communities match.
            return self.hybrid_search(query, limit).await;
        }

        // 3. For each community, load members and build entries
        let mut entries = Vec::new();
        for hit in &community_hits {
            let info = match community_loader.get_community_info(&hit.community_id).await {
                Ok(Some(info)) => info,
                _ => continue,
            };

            let members = match community_loader
                .get_community_members(&hit.community_id)
                .await
            {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Add community summary as an entry
            let paths_str = info.representative_paths.join("; ");
            let community_content = format!(
                "[Community: {}] {} — {} notes, paths: {}",
                info.name, info.summary, info.source_note_count, paths_str
            );

            entries.push(MemoryEntry {
                id: format!("community-{}", hit.community_id),
                content: community_content,
                score: hit.score,
                source: MemorySource::Domain {
                    name: "note_tree".into(),
                },
                raw_score: hit.score,
            });

            // Add top member nodes
            for member in members.iter().take(3) {
                if let Some(entry) = self
                    .hit_to_entry(&TreeNodeHit {
                        node_id: member.tree_node_id.clone(),
                        note_id: String::new(), // unused by hit_to_entry
                        similarity: member.membership_score * hit.score,
                    })
                    .await
                {
                    entries.push(entry);
                }
            }
        }

        // Dedup and sort
        entries = dedup_by_node_id(entries);
        entries.sort_by(|a, b| b.score.total_cmp(&a.score));
        entries.truncate(limit);
        entries
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Convert a `TreeNodeHit` into a `MemoryEntry` with ancestor path context.
    async fn hit_to_entry(&self, hit: &TreeNodeHit) -> Option<MemoryEntry> {
        let node = match self.tree_repo.get_node(&hit.node_id).await {
            Ok(Some(n)) => n,
            Ok(None) => {
                tracing::debug!("NoteTreeNavigator: node {} not found in tree", hit.node_id);
                return None;
            }
            Err(e) => {
                tracing::warn!("NoteTreeNavigator: get_node failed: {e}");
                return None;
            }
        };

        let path = self.build_path(&hit.node_id).await;
        let path_str = path.join(" > ");

        let content = format!("[{}] {}", path_str, node.content);
        let metadata = json!({
            "node_id": node.id,
            "note_id": node.source_id,
            "level": node.level,
            "path": path,
        });
        let _ = metadata; // metadata is encoded in the content prefix

        Some(MemoryEntry {
            id: node.id.clone(),
            content,
            score: hit.similarity,
            source: MemorySource::Domain {
                name: "note_tree".into(),
            },
            raw_score: hit.similarity,
        })
    }

    /// Build the display path from root to a given node (inclusive).
    ///
    /// Returns a list like `["Notebook Title", "Section", "Subsection"]`.
    async fn build_path(&self, node_id: &str) -> Vec<String> {
        match self.tree_repo.get_path_to_root(node_id).await {
            Ok(ancestors) => {
                // `get_path_to_root` returns the path from the node up to root,
                // so we reverse it for display (root first).
                ancestors
                    .into_iter()
                    .rev()
                    .map(|n| n.title.unwrap_or_else(|| truncate_content(&n.content, 40)))
                    .collect()
            }
            Err(e) => {
                tracing::debug!("NoteTreeNavigator: get_path_to_root failed: {e}");
                vec![]
            }
        }
    }
}

#[async_trait]
impl DomainSearcher for NoteTreeNavigator {
    fn domain_name(&self) -> &str {
        "note_tree"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let query_type = classify_query(query, self.active_task.as_deref());

        tracing::debug!(
            ?query_type,
            query = query,
            "NoteTreeNavigator: classified query"
        );

        match query_type {
            QueryType::Simple => self.simple_search(query, limit).await,
            QueryType::Hierarchical => self.hierarchical_search(query, limit).await,
            QueryType::Hybrid => self.hybrid_search(query, limit).await,
            QueryType::Community => self.community_search(query, limit).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// Deduplicate entries by their `id` (node_id), keeping the highest-scored entry.
fn dedup_by_node_id(entries: Vec<MemoryEntry>) -> Vec<MemoryEntry> {
    let mut seen: HashMap<String, MemoryEntry> = HashMap::new();
    for entry in entries {
        seen.entry(entry.id.clone())
            .and_modify(|existing| {
                if entry.score > existing.score {
                    *existing = entry.clone();
                }
            })
            .or_insert(entry);
    }
    let mut result: Vec<MemoryEntry> = seen.into_values().collect();
    result.sort_by(|a, b| b.score.total_cmp(&a.score));
    result
}

/// Truncate content to `max_chars`, appending "..." if truncated.
fn truncate_content(s: &str, max_chars: usize) -> String {
    common::truncate_chars(s, max_chars, "...")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book_index::types::{SourceType, TreeNode, TreeNodeType};

    // -- Query classification tests --

    #[test]
    fn test_classify_simple_query() {
        assert_eq!(
            classify_query("what is the weather today", None),
            QueryType::Simple
        );
        assert_eq!(
            classify_query("tell me about Rust async", None),
            QueryType::Simple
        );
    }

    #[test]
    fn test_classify_hierarchical_query() {
        assert_eq!(
            classify_query("find the section about error handling", None),
            QueryType::Hierarchical
        );
        assert_eq!(
            classify_query("what is in my note about databases", None),
            QueryType::Hierarchical
        );
        assert_eq!(
            classify_query("look under the API heading", None),
            QueryType::Hierarchical
        );
    }

    #[test]
    fn test_classify_hybrid_query() {
        assert_eq!(
            classify_query("find the section on auth", Some("implement login flow")),
            QueryType::Hybrid
        );
        assert_eq!(
            classify_query("what's in my note about caching", Some("optimise API")),
            QueryType::Hybrid
        );
    }

    #[test]
    fn test_classify_notebook_keyword() {
        assert_eq!(
            classify_query("check my notebook for design notes", None),
            QueryType::Hierarchical
        );
    }

    #[test]
    fn test_classify_empty_active_task_stays_hierarchical() {
        // An empty string active task should not upgrade to Hybrid.
        assert_eq!(
            classify_query("find the section on auth", Some("")),
            QueryType::Hierarchical
        );
    }

    #[test]
    fn test_classify_community_query() {
        assert_eq!(
            classify_query("across my notes about AI safety", None),
            QueryType::Community
        );
        assert_eq!(
            classify_query("how does sleep relate to productivity", None),
            QueryType::Community
        );
        assert_eq!(
            classify_query("summarize my thoughts on leadership", None),
            QueryType::Community
        );
        assert_eq!(
            classify_query("everything about Rust patterns", None),
            QueryType::Community
        );
    }

    #[test]
    fn test_classify_case_insensitive() {
        assert_eq!(
            classify_query("Find the SECTION about tests", None),
            QueryType::Hierarchical
        );
        assert_eq!(
            classify_query("IN MY NOTE about errors", None),
            QueryType::Hierarchical
        );
    }

    // -- Utility tests --

    #[test]
    fn test_truncate_content_short() {
        assert_eq!(truncate_content("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_content_long() {
        assert_eq!(
            truncate_content("hello world this is long", 10),
            "hello worl..."
        );
    }

    #[test]
    fn test_dedup_by_node_id_keeps_highest() {
        let entries = vec![
            MemoryEntry {
                id: "a".into(),
                content: "first".into(),
                score: 0.5,
                source: MemorySource::Domain {
                    name: "note_tree".into(),
                },
                raw_score: 0.5,
            },
            MemoryEntry {
                id: "a".into(),
                content: "second".into(),
                score: 0.9,
                source: MemorySource::Domain {
                    name: "note_tree".into(),
                },
                raw_score: 0.9,
            },
            MemoryEntry {
                id: "b".into(),
                content: "other".into(),
                score: 0.7,
                source: MemorySource::Domain {
                    name: "note_tree".into(),
                },
                raw_score: 0.7,
            },
        ];
        let deduped = dedup_by_node_id(entries);
        assert_eq!(deduped.len(), 2);
        // Highest score first after sort.
        assert_eq!(deduped[0].id, "a");
        assert_eq!(deduped[0].score, 0.9);
        assert_eq!(deduped[1].id, "b");
    }

    #[test]
    fn test_rrf_merge_boosts_shared_entries() {
        let list1 = vec![
            MemoryEntry {
                id: "x".into(),
                content: "shared".into(),
                score: 0.8,
                source: MemorySource::Domain {
                    name: "note_tree".into(),
                },
                raw_score: 0.8,
            },
            MemoryEntry {
                id: "y".into(),
                content: "only-list1".into(),
                score: 0.6,
                source: MemorySource::Domain {
                    name: "note_tree".into(),
                },
                raw_score: 0.6,
            },
        ];
        let list2 = vec![
            MemoryEntry {
                id: "x".into(),
                content: "shared".into(),
                score: 0.7,
                source: MemorySource::Domain {
                    name: "note_tree".into(),
                },
                raw_score: 0.7,
            },
            MemoryEntry {
                id: "z".into(),
                content: "only-list2".into(),
                score: 0.5,
                source: MemorySource::Domain {
                    name: "note_tree".into(),
                },
                raw_score: 0.5,
            },
        ];

        let merged = crate::insight_forge::rrf_merge(&[list1, list2], 10);

        // "x" appears in both lists → should rank first.
        assert_eq!(merged[0].id, "x");
        // Top score normalised to 1.0.
        assert!((merged[0].score - 1.0).abs() < f64::EPSILON);
        // 3 unique entries.
        assert_eq!(merged.len(), 3);
    }

    // -- Integration-style test with mocks --

    struct MockEmbeddingSearch {
        hits: Vec<TreeNodeHit>,
    }

    #[async_trait]
    impl TreeNodeEmbeddingSearch for MockEmbeddingSearch {
        async fn search_tree_nodes(
            &self,
            _query_embedding: &[f32],
            limit: usize,
            _min_similarity: f64,
            _note_id_filter: Option<&str>,
        ) -> common::Result<Vec<TreeNodeHit>> {
            Ok(self.hits.iter().take(limit).cloned().collect())
        }

        async fn embed_query(&self, _query: &str) -> common::Result<Vec<f32>> {
            Ok(vec![0.1, 0.2, 0.3])
        }
    }

    struct MockTreeRepo {
        nodes: HashMap<String, TreeNode>,
        fts_results: Vec<TreeNode>,
    }

    impl MockTreeRepo {
        fn new(nodes: Vec<TreeNode>, fts_results: Vec<TreeNode>) -> Self {
            let map = nodes.into_iter().map(|n| (n.id.clone(), n)).collect();
            Self {
                nodes: map,
                fts_results,
            }
        }
    }

    #[async_trait]
    impl BookTreeRepo for MockTreeRepo {
        async fn insert_node(&self, _node: &TreeNode) -> common::Result<()> {
            Ok(())
        }
        async fn insert_nodes(&self, _nodes: &[TreeNode]) -> common::Result<()> {
            Ok(())
        }
        async fn get_node(&self, id: &str) -> common::Result<Option<TreeNode>> {
            Ok(self.nodes.get(id).cloned())
        }
        async fn get_children(&self, _parent_id: &str) -> common::Result<Vec<TreeNode>> {
            Ok(vec![])
        }
        async fn get_subtree(&self, node_id: &str) -> common::Result<Vec<TreeNode>> {
            // Return the node itself as the subtree for testing.
            Ok(self.nodes.get(node_id).cloned().into_iter().collect())
        }
        async fn get_root_sections(
            &self,
            _source_type: &SourceType,
        ) -> common::Result<Vec<TreeNode>> {
            Ok(vec![])
        }
        async fn get_path_to_root(&self, node_id: &str) -> common::Result<Vec<TreeNode>> {
            // Return just the node itself (no real ancestors in test).
            Ok(self.nodes.get(node_id).cloned().into_iter().collect())
        }
        async fn delete_by_source(
            &self,
            _source_type: &SourceType,
            _source_id: &str,
        ) -> common::Result<u64> {
            Ok(0)
        }
        async fn search_fts(&self, _query: &str, _limit: usize) -> common::Result<Vec<TreeNode>> {
            Ok(self.fts_results.clone())
        }
        async fn has_any_nodes(&self) -> common::Result<bool> {
            Ok(!self.nodes.is_empty())
        }
    }

    fn make_node(id: &str, title: &str, content: &str, level: u32) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: content.to_string(),
            title: Some(title.to_string()),
            level,
            source_type: SourceType::Note,
            source_id: "note-1".to_string(),
            position: 0,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_simple_search_returns_entries_with_path() {
        let node = make_node("n1", "My Section", "Some content here", 1);
        let tree_repo = Arc::new(MockTreeRepo::new(vec![node], vec![]));
        let embedding_search = Arc::new(MockEmbeddingSearch {
            hits: vec![TreeNodeHit {
                node_id: "n1".into(),
                note_id: "note-1".into(),
                similarity: 0.85,
            }],
        });

        let nav = NoteTreeNavigator::new(tree_repo, embedding_search, None);
        let results = nav.search("what is some content", 5).await;

        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("My Section"));
        assert!(results[0].content.contains("Some content here"));
        assert_eq!(results[0].score, 0.85);
        assert_eq!(
            results[0].source,
            MemorySource::Domain {
                name: "note_tree".into()
            }
        );
    }

    #[tokio::test]
    async fn test_hierarchical_search_with_fts_boost() {
        let node = make_node("n1", "Auth Section", "Authentication flow details", 1);
        let fts_node = node.clone();
        let tree_repo = Arc::new(MockTreeRepo::new(vec![node], vec![fts_node]));
        let embedding_search = Arc::new(MockEmbeddingSearch {
            hits: vec![TreeNodeHit {
                node_id: "n1".into(),
                note_id: "note-1".into(),
                similarity: 0.7,
            }],
        });

        let nav = NoteTreeNavigator::new(tree_repo, embedding_search, None);
        let results = nav.search("find the section on authentication", 5).await;

        assert_eq!(results.len(), 1);
        // Score should be vector (0.7) + FTS boost (0.15) = 0.85.
        assert!((results[0].score - 0.85).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_domain_name() {
        let tree_repo = Arc::new(MockTreeRepo::new(vec![], vec![]));
        let embedding_search = Arc::new(MockEmbeddingSearch { hits: vec![] });
        let nav = NoteTreeNavigator::new(tree_repo, embedding_search, None);
        assert_eq!(nav.domain_name(), "note_tree");
    }

    #[tokio::test]
    async fn test_hybrid_uses_both_paths() {
        let node = make_node("n1", "Caching", "Cache invalidation strategies", 1);
        let tree_repo = Arc::new(MockTreeRepo::new(vec![node], vec![]));
        let embedding_search = Arc::new(MockEmbeddingSearch {
            hits: vec![TreeNodeHit {
                node_id: "n1".into(),
                note_id: "note-1".into(),
                similarity: 0.75,
            }],
        });

        let nav = NoteTreeNavigator::new(
            tree_repo,
            embedding_search,
            Some("optimise API".to_string()),
        );
        // "in my note" triggers hierarchical keyword; active_task makes it Hybrid.
        let results = nav.search("what's in my note about caching", 5).await;

        // Should get results (merged from both paths).
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_empty_results_on_no_hits() {
        let tree_repo = Arc::new(MockTreeRepo::new(vec![], vec![]));
        let embedding_search = Arc::new(MockEmbeddingSearch { hits: vec![] });
        let nav = NoteTreeNavigator::new(tree_repo, embedding_search, None);

        let results = nav.search("anything", 5).await;
        assert!(results.is_empty());
    }
}
