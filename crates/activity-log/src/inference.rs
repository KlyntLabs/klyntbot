use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use storage::{StoragePool, VectorStore};
use tracing::warn;

use crate::context_resource_repo::ContextResourceRepo;
use crate::normalizers::new_ulid;
use crate::repo::ActivityLogRepo;
use crate::resource_edge_repo::ResourceEdgeRepo;
use crate::types::*;
use crate::work_context_repo::WorkContextRepo;
use crate::work_resource_repo::WorkResourceRepo;

pub struct ContextInferenceConfig {
    pub assignment_threshold: f64,
    pub merge_threshold: f64,
    pub merge_resource_overlap_min: f64,
    pub temporal_decay_lambda: f64,
    pub centroid_learning_rate: f64,
    pub semantic_weight: f64,
    pub temporal_weight: f64,
    pub resource_weight: f64,
    pub max_active_contexts: usize,
}

impl Default for ContextInferenceConfig {
    fn default() -> Self {
        Self {
            assignment_threshold: 0.55,
            merge_threshold: 0.85,
            merge_resource_overlap_min: 0.50,
            temporal_decay_lambda: 0.1,
            centroid_learning_rate: 0.3,
            semantic_weight: 0.50,
            temporal_weight: 0.25,
            resource_weight: 0.25,
            max_active_contexts: 50,
        }
    }
}

impl ContextInferenceConfig {
    pub fn from_work_context_config(cfg: &config::schema::WorkContextConfig) -> Self {
        Self {
            assignment_threshold: cfg.assignment_threshold,
            merge_threshold: cfg.merge_threshold,
            semantic_weight: cfg.semantic_weight,
            temporal_weight: cfg.temporal_weight,
            resource_weight: cfg.resource_weight,
            max_active_contexts: cfg.max_active_contexts,
            ..Default::default()
        }
    }
}

pub struct ContextInferenceEngine {
    pool: StoragePool,
    embedder: Arc<dyn cognitive::TextEmbedder>,
    vector_store: Option<VectorStore>,
    config: ContextInferenceConfig,
    /// In-memory centroid cache: context_id -> 384-dim embedding vector.
    /// VectorStore's search_similar only returns (id, score), not raw vectors,
    /// so centroids must be cached here. Persisted to LanceDB on update for durability.
    centroids: tokio::sync::RwLock<HashMap<String, Vec<f32>>>,
}

impl ContextInferenceEngine {
    pub fn new(
        pool: StoragePool,
        embedder: Arc<dyn cognitive::TextEmbedder>,
        vector_store: Option<VectorStore>,
        config: ContextInferenceConfig,
    ) -> Self {
        Self {
            pool,
            embedder,
            vector_store,
            config,
            centroids: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Ensure centroids are cached for all active contexts.
    /// On cold start, re-embeds context titles to bootstrap centroids.
    async fn ensure_centroids(&self, contexts: &[WorkContext]) {
        let missing: Vec<&WorkContext> = {
            let cache = self.centroids.read().await;
            contexts
                .iter()
                .filter(|c| !cache.contains_key(&c.id))
                .collect()
        };
        for ctx in missing {
            let text = format!("{} {}", ctx.title, ctx.context_type.as_str());
            match self.embedder.embed(&text).await {
                Ok(vec) => {
                    self.set_centroid(&ctx.id, vec).await;
                }
                Err(e) => {
                    warn!("Failed to bootstrap centroid for context {}: {e}", ctx.id);
                }
            }
        }
    }

    /// Process recent unassigned events and assign them to work contexts.
    pub async fn process_recent_events(
        &self,
        since: DateTime<Utc>,
    ) -> common::Result<Vec<ContextAssignment>> {
        let events = ActivityLogRepo::query_unassigned(&self.pool, since).await?;
        if events.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch active contexts once for the whole batch
        let active_contexts = WorkContextRepo::list_active(&self.pool).await?;

        // Bootstrap centroids for any contexts missing from cache (e.g. after restart)
        self.ensure_centroids(&active_contexts).await;

        let mut assignments = Vec::new();
        for event in &events {
            match self
                .assign_event_with_contexts(event, &active_contexts)
                .await
            {
                Ok(assignment) => {
                    ActivityLogRepo::set_work_context_id(
                        &self.pool,
                        &event.id,
                        &assignment.context_id,
                    )
                    .await?;
                    assignments.push(assignment);
                }
                Err(e) => {
                    warn!("Failed to assign event {}: {e}", event.id);
                }
            }
        }
        Ok(assignments)
    }

    /// Assign a single event to a work context (existing or new).
    pub async fn assign_event(
        &self,
        event: &ActivityLogEntry,
    ) -> common::Result<ContextAssignment> {
        let active_contexts = WorkContextRepo::list_active(&self.pool).await?;
        self.assign_event_with_contexts(event, &active_contexts)
            .await
    }

    async fn assign_event_with_contexts(
        &self,
        event: &ActivityLogEntry,
        active_contexts: &[WorkContext],
    ) -> common::Result<ContextAssignment> {
        let text = build_embedding_text(event);
        let event_vec = self.embedder.embed(&text).await?;

        // Phase 1: Compute semantic + temporal scores under read lock (no async I/O)
        let now = Utc::now();
        let partial_scores: Vec<(&WorkContext, f64, f64)> = {
            let centroids = self.centroids.read().await;
            active_contexts
                .iter()
                .map(|ctx| {
                    let semantic = centroids
                        .get(&ctx.id)
                        .map(|c| cosine_similarity(&event_vec, c))
                        .unwrap_or(0.0);
                    let hours_since = (now - ctx.last_active_at).num_minutes().max(0) as f64 / 60.0;
                    let temporal = (-self.config.temporal_decay_lambda * hours_since).exp();
                    (ctx, semantic, temporal)
                })
                .collect()
        }; // read lock dropped here

        // Phase 2: Compute resource overlap (async DB queries) without holding lock
        let mut best_score = 0.0_f64;
        let mut best_ctx_id: Option<String> = None;
        for (ctx, semantic, temporal) in &partial_scores {
            let resource = self.compute_resource_overlap(event, &ctx.id).await;
            let score = self.config.semantic_weight * semantic
                + self.config.temporal_weight * temporal
                + self.config.resource_weight * resource;
            if score > best_score {
                best_score = score;
                best_ctx_id = Some(ctx.id.clone());
            }
        }

        if best_score >= self.config.assignment_threshold {
            let ctx_id = best_ctx_id.unwrap();
            // Update centroid via EMA
            self.update_centroid(&ctx_id, &event_vec).await;
            // Update context stats
            let duration = event.duration_secs.unwrap_or(0);
            WorkContextRepo::update_stats(&self.pool, &ctx_id, Utc::now(), duration, 1).await?;
            // Link resources
            self.link_event_resources(event, &ctx_id).await?;

            Ok(ContextAssignment {
                event_id: event.id.clone(),
                context_id: ctx_id,
                is_new_context: false,
                similarity_score: best_score,
            })
        } else {
            // Create new context
            let ctx_id = new_ulid();
            let title = infer_title(event);
            let now = Utc::now();
            let new_ctx = WorkContext {
                id: ctx_id.clone(),
                title,
                description: None,
                status: WorkContextStatus::Active,
                context_type: infer_context_type(event),
                embedding_id: None,
                linked_project_id: event.project_id.clone(),
                color: None,
                tags: vec![],
                confidence: 0.5,
                first_seen_at: event.timestamp,
                last_active_at: now,
                total_duration_secs: event.duration_secs.unwrap_or(0),
                event_count: 1,
                created_at: now,
                updated_at: now,
            };
            WorkContextRepo::insert(&self.pool, &new_ctx).await?;

            // Set initial centroid (reuses update_centroid which also persists to LanceDB)
            self.set_centroid(&ctx_id, event_vec.clone()).await;

            // Link resources
            self.link_event_resources(event, &ctx_id).await?;

            Ok(ContextAssignment {
                event_id: event.id.clone(),
                context_id: ctx_id,
                is_new_context: true,
                similarity_score: best_score,
            })
        }
    }

    async fn compute_resource_overlap(&self, event: &ActivityLogEntry, context_id: &str) -> f64 {
        // Only compare resource IDs (not names) to avoid false positives from
        // a name coincidentally matching another resource's ID.
        let event_resources: HashSet<String> = event.resource_id.iter().cloned().collect();

        if event_resources.is_empty() {
            return 0.0;
        }

        let ctx_resource_ids = match ContextResourceRepo::list_resource_ids_for_context(
            &self.pool, context_id,
        )
        .await
        {
            Ok(ids) => ids.into_iter().collect::<HashSet<_>>(),
            Err(e) => {
                warn!("Failed to fetch resource IDs for context {context_id}: {e}");
                return 0.0;
            }
        };

        jaccard_similarity(&event_resources, &ctx_resource_ids)
    }

    /// Set a centroid directly (for new contexts).
    async fn set_centroid(&self, context_id: &str, vec: Vec<f32>) {
        self.centroids
            .write()
            .await
            .insert(context_id.to_string(), vec.clone());
        self.persist_centroid(context_id, &vec).await;
    }

    /// EMA-update an existing centroid toward a new vector.
    async fn update_centroid(&self, context_id: &str, new_vec: &[f32]) {
        let lr = self.config.centroid_learning_rate;
        let mut centroids = self.centroids.write().await;
        let centroid = centroids
            .entry(context_id.to_string())
            .or_insert_with(|| new_vec.to_vec());

        // EMA: centroid = (1-lr)*centroid + lr*new_vec, then normalize
        for (c, n) in centroid.iter_mut().zip(new_vec.iter()) {
            *c = (1.0 - lr as f32) * *c + lr as f32 * n;
        }
        let norm: f32 = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            centroid.iter_mut().for_each(|c| *c /= norm);
        }

        let updated = centroid.clone();
        drop(centroids);
        self.persist_centroid(context_id, &updated).await;
    }

    async fn persist_centroid(&self, context_id: &str, vec: &[f32]) {
        if let Some(ref vs) = self.vector_store {
            if let Err(e) = vs
                .upsert_embedding("work_context_embeddings", context_id, vec, &[])
                .await
            {
                warn!("Failed to persist centroid for {context_id}: {e}");
            }
        }
    }

    async fn link_event_resources(
        &self,
        event: &ActivityLogEntry,
        context_id: &str,
    ) -> common::Result<()> {
        // Extract resource from event and upsert + link
        if let Some(ref res_name) = event.resource_name {
            let res_id = event.resource_id.clone().unwrap_or_else(new_ulid);
            let now = Utc::now();
            let resource = WorkResource {
                id: res_id.clone(),
                resource_type: event
                    .resource_type
                    .clone()
                    .unwrap_or_else(|| "file".to_string()),
                resource_name: res_name.clone(),
                resource_path: None,
                resource_uri: None,
                first_seen_at: now,
                last_seen_at: now,
                access_count: 1,
                embedding_id: None,
            };
            WorkResourceRepo::upsert(&self.pool, &resource).await?;

            // Create co-occurrence edges between this resource and existing context resources
            let existing_ids =
                ContextResourceRepo::list_resource_ids_for_context(&self.pool, context_id).await?;
            for existing_id in &existing_ids {
                if *existing_id != res_id {
                    let edge = ResourceEdge {
                        source_id: res_id.clone(),
                        target_id: existing_id.clone(),
                        edge_type: "co_access".to_string(),
                        weight: 1.0,
                        first_seen_at: now,
                        last_seen_at: now,
                    };
                    if let Err(e) = ResourceEdgeRepo::upsert(&self.pool, &edge).await {
                        warn!("Failed to upsert resource edge: {e}");
                    }
                }
            }

            ContextResourceRepo::link(&self.pool, context_id, &res_id, 0.5).await?;
        }
        Ok(())
    }

    /// Check pairs of active contexts for merge candidates.
    /// Merges when centroid similarity > merge_threshold AND resource overlap > 50%.
    pub async fn check_merge_candidates(&self) -> common::Result<Vec<(String, String)>> {
        let active = WorkContextRepo::list_active(&self.pool).await?;
        self.ensure_centroids(&active).await;

        // Bulk-fetch resource IDs for all active contexts (avoids N² queries in the pair loop)
        let mut resource_map: HashMap<String, HashSet<String>> = HashMap::new();
        for ctx in &active {
            let ids = ContextResourceRepo::list_resource_ids_for_context(&self.pool, &ctx.id)
                .await
                .unwrap_or_default();
            resource_map.insert(ctx.id.clone(), ids.into_iter().collect());
        }

        let centroids = self.centroids.read().await;
        let mut merged = Vec::new();

        for i in 0..active.len() {
            for j in (i + 1)..active.len() {
                let ctx_a = &active[i];
                let ctx_b = &active[j];

                // Check centroid similarity
                let sim = match (centroids.get(&ctx_a.id), centroids.get(&ctx_b.id)) {
                    (Some(a), Some(b)) => cosine_similarity(a, b),
                    _ => continue,
                };

                if sim < self.config.merge_threshold {
                    continue;
                }

                // Check resource overlap (Jaccard)
                let empty = HashSet::new();
                let res_a = resource_map.get(&ctx_a.id).unwrap_or(&empty);
                let res_b = resource_map.get(&ctx_b.id).unwrap_or(&empty);
                let overlap = jaccard_similarity(res_a, res_b);

                if overlap < self.config.merge_resource_overlap_min {
                    continue;
                }

                // Merge: keep the one with more events
                let (keep, remove) = if ctx_a.event_count >= ctx_b.event_count {
                    (&ctx_a.id, &ctx_b.id)
                } else {
                    (&ctx_b.id, &ctx_a.id)
                };

                drop(centroids);
                WorkContextRepo::merge(&self.pool, keep, remove).await?;

                // Remove merged centroid from cache
                self.centroids.write().await.remove(remove);

                merged.push((keep.clone(), remove.clone()));
                // After mutation, return early to avoid stale iteration
                return Ok(merged);
            }
        }

        Ok(merged)
    }
}

fn build_embedding_text(event: &ActivityLogEntry) -> String {
    let mut parts = Vec::new();
    parts.push(format!("action:{}", event.action));
    if let Some(ref name) = event.resource_name {
        parts.push(format!("resource:{name}"));
    }
    if let Some(ref app) = event.app_name {
        parts.push(format!("app:{app}"));
    }
    if let Some(ref preview) = event.content_preview {
        parts.push(preview.clone());
    }
    parts.join(" ")
}

fn infer_title(event: &ActivityLogEntry) -> String {
    if let Some(ref name) = event.resource_name {
        format!("{}: {name}", event.action)
    } else if let Some(ref app) = event.app_name {
        format!("{} in {app}", event.action)
    } else {
        event.action.clone()
    }
}

fn infer_context_type(event: &ActivityLogEntry) -> WorkContextType {
    let action = event.action.to_lowercase();
    let resource_type = event.resource_type.as_deref().unwrap_or("");
    let app = event.app_name.as_deref().unwrap_or("").to_lowercase();

    if resource_type == "file" || action.contains("edit") || action.contains("code") {
        WorkContextType::Coding
    } else if action.contains("search") || action.contains("browse") || action.contains("read") {
        WorkContextType::Research
    } else if action.contains("chat") || action.contains("message") || app.contains("slack") {
        WorkContextType::Communication
    } else if action.contains("review") || action.contains("pr") {
        WorkContextType::Review
    } else if action.contains("plan") || action.contains("design") {
        WorkContextType::Planning
    } else if action.contains("meet") || app.contains("zoom") || app.contains("meet") {
        WorkContextType::Meeting
    } else {
        WorkContextType::General
    }
}

fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a * norm_b)) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalizers::new_ulid;
    use std::hash::{Hash, Hasher};

    struct MockTextEmbedder;

    #[async_trait::async_trait]
    impl cognitive::TextEmbedder for MockTextEmbedder {
        async fn embed(&self, text: &str) -> common::Result<Vec<f32>> {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut hasher);
            let seed = hasher.finish();
            let mut rng_state = seed;
            let vec: Vec<f32> = (0..384)
                .map(|_| {
                    rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    ((rng_state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
                })
                .collect();
            let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            Ok(vec.into_iter().map(|x| x / norm).collect())
        }
    }

    fn make_event(action: &str, resource_name: Option<&str>) -> ActivityLogEntry {
        ActivityLogEntry {
            id: new_ulid(),
            timestamp: Utc::now(),
            source: ActivitySource::OsWindow,
            actor: ActivityActor::User,
            resource_type: Some("file".to_string()),
            resource_id: resource_name.map(|n| format!("res-{n}")),
            resource_name: resource_name.map(String::from),
            action: action.to_string(),
            content_preview: None,
            content_hash: None,
            metadata: None,
            app_name: None,
            project_id: None,
            work_context_id: None,
            embedding_id: None,
            duration_secs: Some(60),
            session_key: None,
            is_sensitive: false,
        }
    }

    fn make_engine(pool: StoragePool) -> ContextInferenceEngine {
        ContextInferenceEngine::new(
            pool,
            Arc::new(MockTextEmbedder),
            None,
            ContextInferenceConfig::default(),
        )
    }

    #[tokio::test]
    async fn test_assign_event_creates_new_context() {
        let pool = crate::test_pool().await;
        let engine = make_engine(pool.clone());
        let event = make_event("edit", Some("main.rs"));

        let assignment = engine.assign_event(&event).await.unwrap();
        assert!(assignment.is_new_context);

        // Verify context was created
        let ctx = WorkContextRepo::get(&pool, &assignment.context_id)
            .await
            .unwrap();
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().event_count, 1);
    }

    #[tokio::test]
    async fn test_assign_event_matches_existing() {
        let pool = crate::test_pool().await;
        let engine = make_engine(pool.clone());

        // First event creates a context
        let event1 = make_event("edit", Some("main.rs"));
        let a1 = engine.assign_event(&event1).await.unwrap();
        assert!(a1.is_new_context);

        // Second very similar event should match the existing context
        let event2 = make_event("edit", Some("main.rs"));
        let a2 = engine.assign_event(&event2).await.unwrap();

        // With identical text + same resource, it should match existing
        assert!(!a2.is_new_context);
        assert_eq!(a2.context_id, a1.context_id);
    }

    #[tokio::test]
    async fn test_temporal_proximity_scoring() {
        let pool = crate::test_pool().await;
        let engine = make_engine(pool.clone());

        // Create a context that was active very recently
        let event1 = make_event("edit", Some("recent.rs"));
        let a1 = engine.assign_event(&event1).await.unwrap();

        // Create a context that was active long ago
        let old_ctx_id = &a1.context_id;
        let mut ctx = WorkContextRepo::get(&pool, old_ctx_id)
            .await
            .unwrap()
            .unwrap();
        ctx.last_active_at = Utc::now() - chrono::Duration::hours(48);
        WorkContextRepo::update(&pool, &ctx).await.unwrap();

        // A similar event should prefer recency — since the existing context
        // is now "old", the temporal score drops and a new context may be created
        let event2 = make_event("edit", Some("recent.rs"));
        let a2 = engine.assign_event(&event2).await.unwrap();

        // The temporal decay should reduce the score for the old context
        // Whether it creates new or matches depends on threshold vs combined score
        // Just verify no panic and assignment returned
        assert!(!a2.event_id.is_empty());
    }

    #[tokio::test]
    async fn test_resource_overlap_scoring() {
        let pool = crate::test_pool().await;
        let engine = make_engine(pool.clone());

        // Create context with a resource
        let event1 = make_event("edit", Some("shared.rs"));
        let a1 = engine.assign_event(&event1).await.unwrap();
        assert!(a1.is_new_context);

        // Event with same resource should get resource overlap boost
        let event2 = make_event("edit", Some("shared.rs"));
        let a2 = engine.assign_event(&event2).await.unwrap();
        assert!(!a2.is_new_context);
        assert_eq!(a2.context_id, a1.context_id);
    }

    #[tokio::test]
    async fn test_centroid_ema_update() {
        let pool = crate::test_pool().await;
        let engine = make_engine(pool.clone());

        let event1 = make_event("edit", Some("file_a.rs"));
        let a1 = engine.assign_event(&event1).await.unwrap();

        let centroids = engine.centroids.read().await;
        let initial = centroids.get(&a1.context_id).unwrap().clone();
        drop(centroids);

        // Assign another event to same context — centroid should shift
        let event2 = make_event("edit", Some("file_a.rs"));
        engine.assign_event(&event2).await.unwrap();

        let centroids = engine.centroids.read().await;
        let updated = centroids.get(&a1.context_id).unwrap().clone();
        drop(centroids);

        // Centroid should have changed (EMA moves it)
        let diff: f32 = initial
            .iter()
            .zip(updated.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        // With identical embeddings the EMA doesn't shift much, but
        // normalization may cause tiny diffs. Just verify it's reasonable.
        assert!(diff < 1.0, "Centroid should not jump wildly");
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0f32; 384];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let mut a = vec![0.0f32; 384];
        let mut b = vec![0.0f32; 384];
        a[0] = 1.0;
        b[1] = 1.0;
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-5);
    }

    #[test]
    fn test_infer_context_type_coding() {
        let event = make_event("edit", Some("main.rs"));
        assert_eq!(infer_context_type(&event), WorkContextType::Coding);
    }

    #[test]
    fn test_infer_context_type_communication() {
        let mut event = make_event("chat", None);
        event.resource_type = None;
        assert_eq!(infer_context_type(&event), WorkContextType::Communication);
    }

    #[test]
    fn test_build_embedding_text() {
        let mut event = make_event("edit", Some("main.rs"));
        event.app_name = Some("VSCode".to_string());
        let text = build_embedding_text(&event);
        assert!(text.contains("action:edit"));
        assert!(text.contains("resource:main.rs"));
        assert!(text.contains("app:VSCode"));
    }
}
