//! CommunityBuilder — event subscriber that runs Louvain community detection
//! over shared-entity edges between tree nodes, persists community assignments,
//! embeds community summaries, and pushes context updates.
//!
//! Debounces `NoteContentChanged` events for 5 seconds before processing.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
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

const DEBOUNCE_DURATION: Duration = Duration::from_secs(10);
const MIN_COMMUNITY_SIZE: usize = 2;

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

        let mut pending_source_ids: HashSet<String> = HashSet::new();
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
                            pending_source_ids.insert(note_id);
                            debounce_deadline = Some(Instant::now() + DEBOUNCE_DURATION);
                        }
                        Ok(DomainEvent::TaskHierarchyChanged { project_id }) => {
                            pending_source_ids.insert(format!("task:{project_id}"));
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
                    if !pending_source_ids.is_empty() {
                        let source_ids: Vec<String> = pending_source_ids.drain().collect();
                        debounce_deadline = None;

                        debug!(
                            source_count = source_ids.len(),
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
        for (_comm_idx, members) in &community_members {
            if members.len() < MIN_COMMUNITY_SIZE {
                continue;
            }

            let member_node_ids: Vec<&str> = members.iter().map(|(id, _)| id.as_str()).collect();
            let community_id = stable_community_id(&member_node_ids);
            new_community_ids.insert(community_id.clone());

            // Load tree nodes for top members to build summary
            let top_member_ids: Vec<&str> =
                members.iter().take(5).map(|(id, _)| id.as_str()).collect();

            let mut name_parts = Vec::new();
            let mut summary_parts = Vec::new();
            let mut source_notes: HashSet<String> = HashSet::new();
            let mut representative_paths = Vec::new();

            for node_id in &top_member_ids {
                if let Ok(Some(node)) = self.tree_repo.get_node(node_id).await {
                    // Short title for community name
                    let title = node.title.as_deref().unwrap_or("Untitled").trim();
                    name_parts.push(common::truncate_at_boundary(title, 30).to_string());
                    // Longer text for summary
                    let text = node.title.as_deref().unwrap_or(&node.content);
                    summary_parts.push(common::truncate_at_boundary(text, 100).to_string());
                    source_notes.insert(node.source_id.clone());

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
            let name = if name_parts.len() <= 3 {
                name_parts.join(" & ")
            } else {
                format!(
                    "{} (+{} more)",
                    name_parts[..2].join(" & "),
                    members.len() - 2
                )
            };

            let top_member_node_ids: Vec<&str> =
                members.iter().map(|(id, _)| id.as_str()).collect();
            let top_entities: Vec<String> = self
                .community_repo
                .get_top_entities_for_members(&top_member_node_ids)
                .await;

            let stability = compute_stability(
                self.community_repo
                    .get_community(&community_id)
                    .await
                    .ok()
                    .flatten(),
                members.len(),
            );

            let community = CommunityRow {
                id: community_id.clone(),
                name: common::truncate_at_boundary(&name, 200).to_string(),
                summary: common::truncate_at_boundary(&summary, 500).to_string(),
                member_count: members.len() as i64,
                modularity_score: Some(assignment.modularity),
                stability,
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

/// Generate a stable community ID based on sorted member node IDs.
/// Uses a content hash so the ID is deterministic regardless of Louvain ordering.
fn stable_community_id(member_node_ids: &[&str]) -> String {
    use std::collections::hash_map::DefaultHasher;

    let mut sorted: Vec<&str> = member_node_ids.to_vec();
    sorted.sort();
    let mut hasher = DefaultHasher::new();
    for id in &sorted {
        id.hash(&mut hasher);
    }
    format!("comm-{:016x}", hasher.finish())
}

/// Compute stability for a community based on its previous state.
/// New communities start at 0.7. Growing or stable communities increase toward 1.0.
/// Shrinking communities decay proportionally to member loss.
fn compute_stability(existing: Option<CommunityRow>, new_member_count: usize) -> f64 {
    if let Some(prev) = existing {
        let old_count = prev.member_count as usize;
        if new_member_count >= old_count {
            (prev.stability + 0.05).min(1.0)
        } else {
            let loss_ratio = (old_count - new_member_count) as f64 / old_count.max(1) as f64;
            (prev.stability * (1.0 - loss_ratio * 0.3)).max(0.0)
        }
    } else {
        0.7
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_community_id_same_members_different_order() {
        let a = stable_community_id(&["node-1", "node-2", "node-3"]);
        let b = stable_community_id(&["node-3", "node-1", "node-2"]);
        assert_eq!(a, b, "Same members in different order should produce the same ID");
    }

    #[test]
    fn stable_community_id_different_members() {
        let a = stable_community_id(&["node-1", "node-2"]);
        let b = stable_community_id(&["node-1", "node-3"]);
        assert_ne!(a, b, "Different members should produce different IDs");
    }

    #[test]
    fn stable_community_id_format() {
        let id = stable_community_id(&["a", "b"]);
        assert!(id.starts_with("comm-"), "ID should start with 'comm-'");
        assert_eq!(id.len(), 5 + 16, "ID should be 'comm-' + 16 hex chars");
    }

    #[test]
    fn stability_new_community() {
        assert!((compute_stability(None, 5) - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn stability_growing_community() {
        let prev = CommunityRow {
            id: "c1".into(),
            name: "n".into(),
            summary: "s".into(),
            member_count: 5,
            modularity_score: None,
            stability: 0.8,
            top_entities: None,
            representative_paths: None,
            source_note_count: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let s = compute_stability(Some(prev), 7);
        assert!((s - 0.85).abs() < f64::EPSILON, "Growing community should increase stability");
    }

    #[test]
    fn stability_same_size_community() {
        let prev = CommunityRow {
            id: "c1".into(),
            name: "n".into(),
            summary: "s".into(),
            member_count: 5,
            modularity_score: None,
            stability: 0.9,
            top_entities: None,
            representative_paths: None,
            source_note_count: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let s = compute_stability(Some(prev), 5);
        assert!((s - 0.95).abs() < f64::EPSILON, "Same-size community should increase stability");
    }

    #[test]
    fn stability_shrinking_community() {
        let prev = CommunityRow {
            id: "c1".into(),
            name: "n".into(),
            summary: "s".into(),
            member_count: 10,
            modularity_score: None,
            stability: 0.8,
            top_entities: None,
            representative_paths: None,
            source_note_count: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        // Lost 5 out of 10 → loss_ratio = 0.5, decay = 0.8 * (1 - 0.5 * 0.3) = 0.8 * 0.85 = 0.68
        let s = compute_stability(Some(prev), 5);
        assert!((s - 0.68).abs() < 0.001, "Shrinking community should decay: got {s}");
    }

    #[test]
    fn stability_capped_at_1() {
        let prev = CommunityRow {
            id: "c1".into(),
            name: "n".into(),
            summary: "s".into(),
            member_count: 5,
            modularity_score: None,
            stability: 0.98,
            top_entities: None,
            representative_paths: None,
            source_note_count: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let s = compute_stability(Some(prev), 10);
        assert!((s - 1.0).abs() < f64::EPSILON, "Stability should cap at 1.0");
    }

    #[test]
    fn stability_floored_at_0() {
        let prev = CommunityRow {
            id: "c1".into(),
            name: "n".into(),
            summary: "s".into(),
            member_count: 100,
            modularity_score: None,
            stability: 0.01,
            top_entities: None,
            representative_paths: None,
            source_note_count: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let s = compute_stability(Some(prev), 0);
        assert!(s >= 0.0, "Stability should not go below 0.0");
    }
}
