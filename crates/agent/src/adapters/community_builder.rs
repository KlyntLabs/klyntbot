//! CommunityBuilder — event subscriber that runs Louvain community detection
//! over shared-entity edges between tree nodes, persists community assignments,
//! embeds community summaries, and pushes context updates.
//!
//! Debounces `NoteContentChanged` events for 5 seconds before processing.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::{ContextUpdate, ContextUpdateQueue, ContextUpdateReason, DomainEvent, UpdatePriority};
use cognitive::repos::{CommunityRepo, CommunityRow};
use cognitive::services::louvain;
use cognitive::TextEmbedder;
use context_engine::book_index::BookTreeRepo;

const DEBOUNCE_DURATION: Duration = Duration::from_secs(5);
const MIN_COMMUNITY_SIZE: usize = 3;

pub struct CommunityBuilder {
    community_repo: CommunityRepo,
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
    tree_repo: Arc<dyn BookTreeRepo>,
    context_update_queue: Option<Arc<ContextUpdateQueue>>,
}

impl CommunityBuilder {
    pub fn new(
        community_repo: CommunityRepo,
        vector_store: Arc<storage::VectorStore>,
        embedder: Arc<dyn TextEmbedder>,
        tree_repo: Arc<dyn BookTreeRepo>,
        context_update_queue: Option<Arc<ContextUpdateQueue>>,
    ) -> Self {
        Self {
            community_repo,
            vector_store,
            embedder,
            tree_repo,
            context_update_queue,
        }
    }

    /// Event-driven subscriber loop with debouncing.
    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        info!("CommunityBuilder: subscriber started");

        let mut pending_note_ids: HashSet<String> = HashSet::new();
        let mut debounce_deadline: Option<Instant> = None;

        loop {
            let timeout = debounce_deadline.map(|d| tokio::time::sleep_until(d));

            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("CommunityBuilder: shutdown received");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::NoteContentChanged { note_id, .. }) => {
                            pending_note_ids.insert(note_id);
                            debounce_deadline = Some(Instant::now() + DEBOUNCE_DURATION);
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("CommunityBuilder: lagged, skipped {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("CommunityBuilder: event channel closed");
                            break;
                        }
                    }
                }
                _ = async { timeout.unwrap().await }, if debounce_deadline.is_some() => {
                    if !pending_note_ids.is_empty() {
                        let note_ids: Vec<String> = pending_note_ids.drain().collect();
                        debounce_deadline = None;

                        debug!(
                            note_count = note_ids.len(),
                            "CommunityBuilder: debounce window elapsed, rebuilding communities"
                        );

                        if let Err(e) = self.rebuild_communities().await {
                            warn!("CommunityBuilder: failed to rebuild communities: {e}");
                        }
                    }
                }
            }
        }
    }

    /// Full community rebuild: load edges, run Louvain, persist, embed, push context updates.
    ///
    /// Public so it can be called from backfill jobs.
    pub async fn rebuild_communities(&self) -> common::Result<()> {
        // 1. Load shared-entity edges
        let edges = self.community_repo.load_shared_entity_edges().await?;

        if edges.is_empty() {
            debug!("CommunityBuilder: no shared-entity edges found, skipping");
            return Ok(());
        }

        debug!(
            edge_count = edges.len(),
            "CommunityBuilder: loaded shared-entity edges"
        );

        // 2. Run Louvain community detection
        let assignment = louvain::detect_communities(&edges);

        if assignment.community_count == 0 {
            debug!("CommunityBuilder: Louvain found no communities");
            return Ok(());
        }

        info!(
            community_count = assignment.community_count,
            modularity = format!("{:.3}", assignment.modularity),
            "CommunityBuilder: Louvain detection complete"
        );

        // 3. Group assignments into communities
        let mut community_members: HashMap<usize, Vec<(String, f64)>> = HashMap::new();
        for (node_id, comm_id) in &assignment.assignments {
            community_members
                .entry(*comm_id)
                .or_default()
                .push((node_id.clone(), 1.0));
        }

        // 4. Get previously existing community IDs for diffing
        let existing_communities = self.community_repo.list_active_communities().await?;
        let existing_ids: HashSet<String> =
            existing_communities.iter().map(|c| c.id.clone()).collect();

        let now = chrono::Utc::now().to_rfc3339();
        let mut new_community_ids: HashSet<String> = HashSet::new();

        // 5. Process each community
        for (comm_idx, members) in &community_members {
            if members.len() < MIN_COMMUNITY_SIZE {
                continue;
            }

            let community_id = format!("comm-{comm_idx}");
            new_community_ids.insert(community_id.clone());

            // Load tree nodes for top members to build summary
            let top_member_ids: Vec<&str> =
                members.iter().take(5).map(|(id, _)| id.as_str()).collect();

            let mut summary_parts = Vec::new();
            let mut source_notes: HashSet<String> = HashSet::new();
            let mut representative_paths = Vec::new();

            for node_id in &top_member_ids {
                if let Ok(Some(node)) = self.tree_repo.get_node(node_id).await {
                    let text = node.title.as_deref().unwrap_or(&node.content);
                    summary_parts.push(common::truncate_at_boundary(text, 100).to_string());
                    source_notes.insert(node.source_id.clone());

                    // Build representative path
                    if representative_paths.len() < 3 {
                        if let Ok(ancestors) = self.tree_repo.get_path_to_root(node_id).await {
                            let path: Vec<String> = ancestors
                                .iter()
                                .rev()
                                .filter_map(|a| a.title.clone())
                                .collect();
                            if !path.is_empty() {
                                representative_paths.push(path.join(" > "));
                            }
                        }
                    }
                }
            }

            let summary = summary_parts.join("; ");
            let name = if summary_parts.len() <= 3 {
                summary_parts.join(" & ")
            } else {
                format!(
                    "{} (+{} more)",
                    summary_parts[..2].join(" & "),
                    members.len() - 2
                )
            };

            let top_entities: Vec<String> = Vec::new();

            let community = CommunityRow {
                id: community_id.clone(),
                name: common::truncate_at_boundary(&name, 200).to_string(),
                summary: common::truncate_at_boundary(&summary, 500).to_string(),
                member_count: members.len() as i64,
                modularity_score: Some(assignment.modularity),
                stability: 1.0,
                top_entities: Some(serde_json::to_string(&top_entities).unwrap_or_default()),
                representative_paths: Some(
                    serde_json::to_string(&representative_paths).unwrap_or_default(),
                ),
                source_note_count: Some(source_notes.len() as i64),
                created_at: now.clone(),
                updated_at: now.clone(),
            };

            // 6. Persist community and members
            self.community_repo.upsert_community(&community).await?;
            self.community_repo
                .set_members(&community_id, members)
                .await?;

            // 7. Embed community summary
            if !summary.is_empty() {
                match self.embedder.embed(&summary).await {
                    Ok(embedding) => {
                        if let Err(e) = self
                            .vector_store
                            .upsert_community_embedding(
                                &community_id,
                                &embedding,
                                &members.len().to_string(),
                                &source_notes.len().to_string(),
                            )
                            .await
                        {
                            warn!(
                                community_id = %community_id,
                                "CommunityBuilder: failed to upsert embedding: {e}"
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            community_id = %community_id,
                            "CommunityBuilder: failed to embed summary: {e}"
                        );
                    }
                }
            }

            // 8. Push context updates for new/changed communities
            if let Some(queue) = &self.context_update_queue {
                let reason = if existing_ids.contains(&community_id) {
                    ContextUpdateReason::CommunityUpdated
                } else {
                    ContextUpdateReason::CommunityDiscovered
                };

                queue.push(ContextUpdate {
                    reason,
                    content: Some(format!(
                        "Community \"{}\": {} nodes across {} notes — {}",
                        name,
                        members.len(),
                        source_notes.len(),
                        summary_parts.first().unwrap_or(&String::new()),
                    )),
                    metadata: Some(serde_json::json!({
                        "community_id": community_id,
                        "member_count": members.len(),
                        "source_note_count": source_notes.len(),
                        "modularity": assignment.modularity,
                    })),
                    priority: UpdatePriority::Normal,
                    timestamp: chrono::Utc::now(),
                });
            }

            tokio::task::yield_now().await;
        }

        // 9. Prune communities that no longer have enough members
        let pruned = self.community_repo.prune_weak_communities(0.3).await?;
        for pruned_id in &pruned {
            if let Err(e) = self
                .vector_store
                .delete_community_embedding(pruned_id)
                .await
            {
                warn!("CommunityBuilder: failed to delete embedding for pruned community {pruned_id}: {e}");
            }
            if let Some(queue) = &self.context_update_queue {
                queue.push(ContextUpdate {
                    reason: ContextUpdateReason::CommunityWeakened,
                    content: Some(format!(
                        "Community {pruned_id} dissolved (stability too low)"
                    )),
                    metadata: None,
                    priority: UpdatePriority::Low,
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        info!(
            communities = new_community_ids.len(),
            pruned = pruned.len(),
            "CommunityBuilder: rebuild complete"
        );

        Ok(())
    }
}
