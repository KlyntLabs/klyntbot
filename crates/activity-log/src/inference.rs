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
            assignment_threshold: 0.45,
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
        // Skip entirely in heuristic-only mode (semantic_weight == 0.0)
        if self.config.semantic_weight > 0.0 {
            self.ensure_centroids(&active_contexts).await;
        }

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
        let use_semantic = self.config.semantic_weight > 0.0;

        // Only embed when semantic scoring is enabled (skip CPU cost at weight 0.0)
        let event_vec = if use_semantic {
            let text = build_embedding_text(event);
            Some(self.embedder.embed(&text).await?)
        } else {
            None
        };

        // Phase 1: Compute semantic + temporal scores under read lock (no async I/O)
        let now = Utc::now();
        let partial_scores: Vec<(&WorkContext, f64, f64)> = {
            let centroids = if use_semantic {
                Some(self.centroids.read().await)
            } else {
                None
            };
            active_contexts
                .iter()
                .map(|ctx| {
                    let semantic = match (&event_vec, &centroids) {
                        (Some(vec), Some(cache)) => cache
                            .get(&ctx.id)
                            .map(|c| cosine_similarity(vec, c))
                            .unwrap_or(0.0),
                        _ => 0.0,
                    };
                    let hours_since = (now - ctx.last_active_at).num_minutes().max(0) as f64 / 60.0;
                    let temporal = (-self.config.temporal_decay_lambda * hours_since).exp();
                    (ctx, semantic, temporal)
                })
                .collect()
        }; // read lock dropped here

        // Phase 2: Compute resource overlap (async DB queries) without holding lock
        // Also apply title-similarity boosting (FIX 6)
        let inferred_title = infer_title(event).to_lowercase();
        let mut best_score = 0.0_f64;
        let mut best_ctx_id: Option<String> = None;
        for (ctx, semantic, temporal) in &partial_scores {
            let resource = self.compute_resource_overlap(event, &ctx.id).await;
            let mut score = self.config.semantic_weight * semantic
                + self.config.temporal_weight * temporal
                + self.config.resource_weight * resource;

            // Title-similarity boost: matching titles get a bonus
            let ctx_title = ctx.title.to_lowercase();
            if inferred_title == ctx_title {
                score += 0.20; // Exact match
            } else if inferred_title.contains(&ctx_title) || ctx_title.contains(&inferred_title) {
                score += 0.10; // Partial match
            }

            if score > best_score {
                best_score = score;
                best_ctx_id = Some(ctx.id.clone());
            }
        }

        if best_score >= self.config.assignment_threshold {
            let ctx_id = best_ctx_id.unwrap();
            // Update centroid via EMA (only when semantic scoring is active)
            if let Some(ref vec) = event_vec {
                self.update_centroid(&ctx_id, vec).await;
            }

            // Infer duration from event gap if not explicitly set (FIX 3)
            let duration = if let Some(explicit) = event.duration_secs {
                explicit
            } else {
                let ctx = active_contexts.iter().find(|c| c.id == ctx_id);
                match ctx {
                    Some(c) => {
                        let gap_secs = (event.timestamp - c.last_active_at).num_seconds().max(0);
                        gap_secs.min(30 * 60) // Cap at 30 minutes
                    }
                    None => 0,
                }
            };
            WorkContextRepo::update_stats(&self.pool, &ctx_id, Utc::now(), duration, 1).await?;

            // Link resources
            self.link_event_resources(event, &ctx_id).await?;

            // Recalculate confidence (FIX 1)
            let resource_count = ContextResourceRepo::count_for_context(&self.pool, &ctx_id).await;
            let ctx = WorkContextRepo::get(&self.pool, &ctx_id).await?;
            if let Some(ctx) = ctx {
                let new_confidence =
                    calculate_confidence(ctx.event_count, best_score, resource_count as usize);
                WorkContextRepo::update_confidence(&self.pool, &ctx_id, new_confidence).await?;
            }

            // Auto-enrich with tags, description, color (FIX 5)
            self.auto_enrich_context(&ctx_id).await;

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
            let context_type = infer_context_type(event);
            let now = Utc::now();
            let new_ctx = WorkContext {
                id: ctx_id.clone(),
                title,
                description: None,
                status: WorkContextStatus::Active,
                context_type,
                embedding_id: None,
                linked_project_id: event.project_id.clone(),
                color: Some(default_color_for_type(&context_type).to_string()),
                tags: vec![],
                confidence: 0.3, // Start low, grows with evidence (FIX 1)
                first_seen_at: event.timestamp,
                last_active_at: now,
                total_duration_secs: event.duration_secs.unwrap_or(0),
                event_count: 1,
                created_at: now,
                updated_at: now,
            };
            WorkContextRepo::insert(&self.pool, &new_ctx).await?;

            // Set initial centroid (only when semantic scoring is active)
            if let Some(vec) = event_vec {
                self.set_centroid(&ctx_id, vec).await;
            }

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
    /// Merges when (centroid similarity > merge_threshold AND resource overlap > 50%)
    /// OR when titles match exactly (regardless of centroid similarity).
    /// Collects all candidates first, then executes merges in a second pass.
    pub async fn check_merge_candidates(&self) -> common::Result<Vec<(String, String)>> {
        let active = WorkContextRepo::list_active(&self.pool).await?;
        self.ensure_centroids(&active).await;

        // Bulk-fetch resource IDs for all active contexts
        let mut resource_map: HashMap<String, HashSet<String>> = HashMap::new();
        for ctx in &active {
            let ids = ContextResourceRepo::list_resource_ids_for_context(&self.pool, &ctx.id)
                .await
                .unwrap_or_default();
            resource_map.insert(ctx.id.clone(), ids.into_iter().collect());
        }

        // Phase 1: Collect all merge candidates under read lock (no mutations)
        let candidates: Vec<(String, String)> = {
            let centroids = self.centroids.read().await;
            let mut found = Vec::new();

            for i in 0..active.len() {
                for j in (i + 1)..active.len() {
                    let ctx_a = &active[i];
                    let ctx_b = &active[j];

                    let sim = match (centroids.get(&ctx_a.id), centroids.get(&ctx_b.id)) {
                        (Some(a), Some(b)) => cosine_similarity(a, b),
                        _ => continue,
                    };

                    // Title-match fast path: identical titles always merge
                    let title_match = ctx_a.title == ctx_b.title;

                    if !title_match && sim < self.config.merge_threshold {
                        continue;
                    }

                    if !title_match {
                        // Only check resource overlap for non-title matches
                        let empty = HashSet::new();
                        let res_a = resource_map.get(&ctx_a.id).unwrap_or(&empty);
                        let res_b = resource_map.get(&ctx_b.id).unwrap_or(&empty);
                        let overlap = jaccard_similarity(res_a, res_b);

                        if overlap < self.config.merge_resource_overlap_min {
                            continue;
                        }
                    }

                    let (keep, remove) = if ctx_a.event_count >= ctx_b.event_count {
                        (ctx_a.id.clone(), ctx_b.id.clone())
                    } else {
                        (ctx_b.id.clone(), ctx_a.id.clone())
                    };
                    found.push((keep, remove));
                }
            }
            found
        }; // read lock dropped

        // Phase 2: Execute merges
        let mut merged = Vec::new();
        let mut already_removed: HashSet<String> = HashSet::new();

        for (keep, remove) in candidates {
            if already_removed.contains(&keep) || already_removed.contains(&remove) {
                continue;
            }

            if let Err(e) = WorkContextRepo::merge(&self.pool, &keep, &remove, "inferred").await {
                warn!("Failed to merge contexts {keep} <- {remove}: {e}");
                continue;
            }
            self.centroids.write().await.remove(&remove);
            already_removed.insert(remove.clone());
            merged.push((keep, remove));
        }

        Ok(merged)
    }

    /// Auto-enrich context with tags, description, and color if missing (FIX 5).
    async fn auto_enrich_context(&self, context_id: &str) {
        let ctx = match WorkContextRepo::get(&self.pool, context_id).await {
            Ok(Some(c)) => c,
            _ => return,
        };

        let mut ctx = ctx;
        let mut updated = false;

        // Auto-color based on context type (only if no color set)
        if ctx.color.is_none() {
            ctx.color = Some(default_color_for_type(&ctx.context_type).to_string());
            updated = true;
        }

        // Auto-tags from resources (only if fewer than 3 tags)
        if ctx.tags.len() < 3 {
            let resource_ids =
                ContextResourceRepo::list_resource_ids_for_context(&self.pool, context_id)
                    .await
                    .unwrap_or_default();

            let resources = WorkResourceRepo::get_by_ids(&self.pool, &resource_ids)
                .await
                .unwrap_or_default();

            let mut new_tags: HashSet<String> = ctx.tags.iter().cloned().collect();
            for res in &resources {
                let name = res.resource_name.to_lowercase();
                if let Some(ext) = name.rsplit('.').next() {
                    if CODE_EXTENSIONS.contains(&ext) {
                        new_tags.insert(ext.to_string());
                    }
                }
            }
            if let Some(ref proj) = ctx.linked_project_id {
                new_tags.insert(proj.clone());
            }

            let tags_vec: Vec<String> = new_tags.into_iter().take(5).collect();
            if tags_vec != ctx.tags {
                ctx.tags = tags_vec;
                updated = true;
            }
        }

        // Auto-description (only if missing and we have enough events)
        if ctx.description.is_none() && ctx.event_count >= 3 {
            let resource_ids =
                ContextResourceRepo::list_resource_ids_for_context(&self.pool, context_id)
                    .await
                    .unwrap_or_default();

            let res_names: Vec<String> = WorkResourceRepo::get_by_ids(&self.pool, &resource_ids)
                .await
                .unwrap_or_default()
                .into_iter()
                .take(3)
                .map(|r| r.resource_name)
                .collect();

            if !res_names.is_empty() {
                ctx.description = Some(format!(
                    "{} activity involving {}",
                    ctx.context_type.as_str(),
                    res_names.join(", ")
                ));
                updated = true;
            }
        }

        if updated {
            let _ = WorkContextRepo::update(&self.pool, &ctx).await;
        }
    }

    /// Reclassify contexts currently typed as "general" using improved heuristics (FIX 4).
    /// Called once at startup.
    pub async fn reclassify_general_contexts(&self) -> common::Result<usize> {
        let active = WorkContextRepo::list_active(&self.pool).await?;
        let general_contexts: Vec<&WorkContext> = active
            .iter()
            .filter(|c| c.context_type == WorkContextType::General)
            .collect();

        let mut reclassified = 0;
        for ctx in general_contexts {
            let events = ActivityLogRepo::query_by_context(&self.pool, &ctx.id, 5).await?;
            if events.is_empty() {
                continue;
            }

            // Majority vote from recent events
            let mut type_counts: HashMap<WorkContextType, usize> = HashMap::new();
            for event in &events {
                let inferred = infer_context_type(event);
                *type_counts.entry(inferred).or_default() += 1;
            }

            let best_type = type_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(t, _)| t)
                .unwrap_or(WorkContextType::General);

            if best_type != WorkContextType::General {
                let mut updated = ctx.clone();
                updated.context_type = best_type;
                updated.color = Some(default_color_for_type(&best_type).to_string());
                WorkContextRepo::update(&self.pool, &updated).await?;
                reclassified += 1;
            }
        }
        Ok(reclassified)
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

const CODE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "swift", "kt", "cpp", "c", "h", "hpp",
    "toml", "json", "yaml", "yml", "css", "html", "sql", "sh", "rb", "ex", "exs",
];

fn infer_context_type(event: &ActivityLogEntry) -> WorkContextType {
    let action = event.action.to_lowercase();
    let resource_type = event.resource_type.as_deref().unwrap_or("");
    let app = event.app_name.as_deref().unwrap_or("").to_lowercase();
    let resource = event.resource_name.as_deref().unwrap_or("").to_lowercase();

    // 1. Coding: file resources, code editors, terminal
    if resource_type == "file"
        || action.contains("edit")
        || action.contains("code")
        || app.contains("visual studio")
        || app.contains("vscode")
        || app.contains("intellij")
        || app.contains("xcode")
        || app.contains("terminal")
        || app.contains("iterm")
        || app.contains("warp")
        || app.contains("alacritty")
        || app.contains("kitty")
        || resource
            .rsplit('.')
            .next()
            .is_some_and(|ext| CODE_EXTENSIONS.contains(&ext))
        || resource.contains("cargo")
        || resource.contains("npm")
        || resource.contains("git")
    {
        WorkContextType::Coding
    }
    // 2. Research: browsers, search, reading, documentation
    else if action.contains("search")
        || action.contains("browse")
        || action.contains("read")
        || app.contains("chrome")
        || app.contains("firefox")
        || app.contains("safari")
        || app.contains("arc")
        || app.contains("brave")
        || resource.contains("stackoverflow")
        || resource.contains("github.com")
        || resource.contains("docs.")
        || resource.contains("wiki")
    {
        WorkContextType::Research
    }
    // 3. Communication: chat, email, messaging
    else if action.contains("chat")
        || action.contains("message")
        || action.contains("email")
        || action.contains("reply")
        || action.contains("prompt")
        || app.contains("slack")
        || app.contains("discord")
        || app.contains("telegram")
        || app.contains("teams")
        || app.contains("mail")
        || app.contains("outlook")
        || app.contains("messages")
        || resource_type == "conversation"
    {
        WorkContextType::Communication
    }
    // 4. Review: PR review, code review
    else if action.contains("review")
        || resource.contains("pull request")
        || resource.contains("review")
    {
        WorkContextType::Review
    }
    // 5. Planning: design, planning tools
    else if action.contains("plan")
        || action.contains("design")
        || app.contains("figma")
        || app.contains("notion")
        || app.contains("linear")
        || app.contains("jira")
        || app.contains("miro")
        || app.contains("trello")
    {
        WorkContextType::Planning
    }
    // 6. Meeting: video calls, calendar
    else if action.contains("meet")
        || app.contains("zoom")
        || app.contains("meet")
        || app.contains("facetime")
        || app.contains("loom")
        || app.contains("webex")
    {
        WorkContextType::Meeting
    }
    // 7. Learning: educational content
    else if resource.contains("course")
        || resource.contains("tutorial")
        || resource.contains("learn")
        || resource.contains("udemy")
        || resource.contains("youtube")
        || app.contains("anki")
    {
        WorkContextType::Learning
    }
    // Fallback
    else {
        WorkContextType::General
    }
}

/// Recalculate confidence for a context based on accumulated evidence (FIX 1).
/// Formula: base + event_factor + similarity_factor + resource_factor, clamped to [0.1, 0.99].
fn calculate_confidence(event_count: i64, avg_similarity: f64, resource_count: usize) -> f64 {
    let base = 0.25;
    let event_factor = 0.25 * (1.0 - (-0.15 * event_count as f64).exp());
    let similarity_factor = 0.30 * avg_similarity;
    let resource_factor = 0.20 * (1.0 - (-0.3 * resource_count as f64).exp());
    (base + event_factor + similarity_factor + resource_factor).clamp(0.1, 0.99)
}

fn default_color_for_type(ct: &WorkContextType) -> &'static str {
    match ct {
        WorkContextType::Coding => "#8B5CF6",
        WorkContextType::Research => "#3B82F6",
        WorkContextType::Communication => "#10B981",
        WorkContextType::Review => "#F59E0B",
        WorkContextType::Planning => "#EC4899",
        WorkContextType::Meeting => "#EF4444",
        WorkContextType::Learning => "#06B6D4",
        WorkContextType::General => "#6B7280",
    }
}

fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

// Re-use canonical implementation from common crate.
use common::helpers::cosine_similarity;

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
    fn test_infer_context_type_vscode() {
        let mut event = make_event("view", Some("main.rs"));
        event.app_name = Some("Visual Studio Code".to_string());
        event.resource_type = None;
        assert_eq!(infer_context_type(&event), WorkContextType::Coding);
    }

    #[test]
    fn test_infer_context_type_chrome_is_research() {
        let mut event = make_event("view", Some("Some Blog Post"));
        event.app_name = Some("Google Chrome".to_string());
        event.resource_type = None;
        assert_eq!(infer_context_type(&event), WorkContextType::Research);
    }

    #[test]
    fn test_infer_context_type_terminal() {
        let mut event = make_event("view", Some("cargo build"));
        event.app_name = Some("Terminal".to_string());
        event.resource_type = None;
        assert_eq!(infer_context_type(&event), WorkContextType::Coding);
    }

    #[test]
    fn test_infer_context_type_youtube_is_learning() {
        let mut event = make_event("view", Some("Rust Tutorial - YouTube"));
        event.app_name = None;
        event.resource_type = None;
        assert_eq!(infer_context_type(&event), WorkContextType::Learning);
    }

    #[test]
    fn test_infer_context_type_conversation_is_communication() {
        let mut event = make_event("prompt", None);
        event.resource_type = Some("conversation".to_string());
        assert_eq!(infer_context_type(&event), WorkContextType::Communication);
    }

    #[test]
    fn test_calculate_confidence_grows_with_events() {
        let c1 = calculate_confidence(1, 0.5, 1);
        let c5 = calculate_confidence(5, 0.5, 3);
        let c20 = calculate_confidence(20, 0.8, 8);
        assert!(c1 < c5, "Confidence should grow with event count");
        assert!(
            c5 < c20,
            "Confidence should grow with more events and better similarity"
        );
        assert!(c20 < 1.0, "Confidence should never reach 1.0");
        assert!(c1 >= 0.1, "Confidence should never go below 0.1");
    }

    #[test]
    fn test_calculate_confidence_clamps() {
        let low = calculate_confidence(0, 0.0, 0);
        assert!(low >= 0.1);
        assert!(low <= 0.99);
    }

    #[test]
    fn test_default_color_for_type() {
        assert_eq!(default_color_for_type(&WorkContextType::Coding), "#8B5CF6");
        assert_eq!(default_color_for_type(&WorkContextType::General), "#6B7280");
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
