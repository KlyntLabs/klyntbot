//! Community intelligence types and execution logic.
//!
//! Reforge Phase 6.5 uses an LLM to name communities, decide merges, and
//! decide splits. This module defines the input/output types and the
//! execution functions that apply structural changes.

pub mod co_activation_events;
pub mod events;

use std::collections::HashMap;

use crate::repos::community::{CommunityRepo, CommunityRow};

/// Context for a single community, sent to the LLM.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommunityContext {
    pub id: String,
    pub current_name: String,
    pub entities: Vec<String>,
    pub member_count: usize,
    pub domains: HashMap<String, usize>,
    pub age_days: u32,
}

/// Input for the community intelligence LLM call.
#[derive(Debug, Clone)]
pub struct CommunityIntelligenceInput {
    pub communities: Vec<CommunityContext>,
}

/// Output from the community intelligence LLM call.
#[derive(Debug, Clone, Default)]
pub struct CommunityIntelligenceOutput {
    pub names: Vec<CommunityRename>,
    pub merges: Vec<CommunityMerge>,
    pub splits: Vec<CommunitySplit>,
}

#[derive(Debug, Clone)]
pub struct CommunityRename {
    pub community_id: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct CommunityMerge {
    pub absorb_id: String,
    pub into_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct CommunitySplit {
    pub community_id: String,
    pub reason: String,
}

/// Maximum structural changes per nightly cycle to prevent thrashing.
const MAX_MERGES_PER_CYCLE: usize = 2;
const MAX_SPLITS_PER_CYCLE: usize = 1;
/// Minimum age (days) before a community can be merged or split.
/// Used in `build_intelligence_input` to annotate age for the LLM.
#[allow(dead_code)]
const MIN_AGE_FOR_RESTRUCTURE: u32 = 3;

/// Apply community intelligence output: renames, merges, splits.
///
/// Publishes `CommunityEvent::Updated` on successful renames/merges and
/// `CommunityEvent::Weakened` on splits (the source community is fragmented).
/// Pass `None` for `bus` to skip publishing.
///
/// Returns (renamed, merged, split) counts.
pub async fn apply_intelligence(
    output: &CommunityIntelligenceOutput,
    community_repo: &CommunityRepo,
    co_activation_repo: &crate::repos::CoActivationRepo,
    bus: Option<std::sync::Arc<bus::DomainEventBus>>,
) -> (u32, u32, u32) {
    use events::CommunityEvent;

    let mut renamed = 0u32;
    let mut merged = 0u32;
    let mut split = 0u32;

    // 1. Apply renames
    for rename in &output.names {
        if let Err(e) = community_repo
            .rename(&rename.community_id, &rename.label)
            .await
        {
            tracing::debug!("Community rename failed for {}: {e}", rename.community_id);
        } else {
            if let Some(b) = &bus {
                let event: bus::DomainEvent = CommunityEvent::Updated {
                    community_id: rename.community_id.clone(),
                    name: rename.label.clone(),
                    reason: "renamed".into(),
                }
                .into();
                b.publish(event);
            }
            renamed += 1;
        }
    }

    // 2. Apply merges (capped)
    for merge in output.merges.iter().take(MAX_MERGES_PER_CYCLE) {
        if merge.reason.is_empty() {
            continue;
        }
        if let Err(e) = community_repo
            .merge_communities(&merge.absorb_id, &merge.into_id)
            .await
        {
            tracing::debug!(
                "Community merge failed ({} -> {}): {e}",
                merge.absorb_id,
                merge.into_id
            );
        } else {
            tracing::info!(
                absorb = %merge.absorb_id,
                into = %merge.into_id,
                reason = %merge.reason,
                "Community merged"
            );
            if let Some(b) = &bus {
                let event: bus::DomainEvent = CommunityEvent::Updated {
                    community_id: merge.into_id.clone(),
                    name: String::new(),
                    reason: format!("merged:{}", merge.reason),
                }
                .into();
                b.publish(event);
            }
            merged += 1;
        }
    }

    // 3. Apply splits (capped) — re-run Louvain on sub-graph
    for split_req in output.splits.iter().take(MAX_SPLITS_PER_CYCLE) {
        if split_req.reason.is_empty() {
            continue;
        }
        match execute_split(split_req, community_repo, co_activation_repo).await {
            Ok(new_count) if new_count > 1 => {
                tracing::info!(
                    community = %split_req.community_id,
                    new_communities = new_count,
                    reason = %split_req.reason,
                    "Community split"
                );
                if let Some(b) = &bus {
                    let event: bus::DomainEvent = CommunityEvent::Weakened {
                        community_id: split_req.community_id.clone(),
                        name: String::new(),
                        stability: 0.0,
                    }
                    .into();
                    b.publish(event);
                }
                split += 1;
            }
            Ok(_) => {
                tracing::debug!(
                    "Split aborted for {} — Louvain didn't find sub-clusters",
                    split_req.community_id
                );
            }
            Err(e) => {
                tracing::debug!("Community split failed for {}: {e}", split_req.community_id);
            }
        }
    }

    (renamed, merged, split)
}

/// Execute a community split by re-running Louvain on the sub-graph.
async fn execute_split(
    split: &CommunitySplit,
    community_repo: &CommunityRepo,
    co_activation_repo: &crate::repos::CoActivationRepo,
) -> common::Result<usize> {
    // 1. Get current members
    let members = community_repo.get_members(&split.community_id).await?;
    if members.len() < 4 {
        return Ok(1); // Too small to split
    }

    let member_ids: std::collections::HashSet<String> =
        members.iter().map(|m| m.tree_node_id.clone()).collect();

    // 2. Get co-activation edges between these members only
    let all_edges = co_activation_repo
        .list_all_pairs()
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    let sub_edges: Vec<(String, String, f64)> = all_edges
        .into_iter()
        .filter(|(a, b, _)| member_ids.contains(a) && member_ids.contains(b))
        .collect();

    if sub_edges.is_empty() {
        return Ok(1); // No internal edges
    }

    // 3. Re-run Louvain on sub-graph
    let assignment = crate::services::louvain::detect_communities(&sub_edges);
    if assignment.community_count <= 1 {
        return Ok(1); // Louvain confirms it's one cluster — abort
    }

    // 4. Group members by new community assignment
    let mut new_groups: HashMap<usize, Vec<(String, f64)>> = HashMap::new();
    for member in &members {
        let comm_id = assignment
            .assignments
            .get(&member.tree_node_id)
            .copied()
            .unwrap_or(0);
        new_groups
            .entry(comm_id)
            .or_default()
            .push((member.tree_node_id.clone(), member.membership_score));
    }

    // 5. Delete original community
    community_repo.delete_community(&split.community_id).await?;

    // 6. Create new sub-communities
    let now = jiff::Timestamp::now().to_string();
    for (idx, group_members) in &new_groups {
        let new_id = format!("{}-sub{}", split.community_id, idx);
        let community = CommunityRow {
            id: new_id.clone(),
            name: format!("Sub-cluster {}", idx + 1),
            summary: String::new(),
            member_count: group_members.len() as i64,
            modularity_score: Some(assignment.modularity),
            stability: 0.5,
            top_entities: None,
            representative_paths: None,
            source_note_count: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            last_restructured_at: Some(now.clone()),
        };
        community_repo.upsert_community(&community).await?;
        community_repo.set_members(&new_id, group_members).await?;
    }

    Ok(new_groups.len())
}

/// Build community contexts from active communities for the LLM.
pub async fn build_intelligence_input(
    community_repo: &CommunityRepo,
) -> common::Result<CommunityIntelligenceInput> {
    let communities = community_repo.list_active_communities().await?;
    let now = jiff::Timestamp::now();

    let mut contexts = Vec::new();
    for c in &communities {
        let entities: Vec<String> = c
            .top_entities
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let age_days = c
            .created_at
            .parse::<jiff::Timestamp>()
            .map(|ts| ((now.as_millisecond() - ts.as_millisecond()).max(0) / 86_400_000) as u32)
            .unwrap_or(0);

        contexts.push(CommunityContext {
            id: c.id.clone(),
            current_name: c.name.clone(),
            entities,
            member_count: c.member_count as usize,
            domains: HashMap::new(),
            age_days,
        });
    }

    Ok(CommunityIntelligenceInput {
        communities: contexts,
    })
}
