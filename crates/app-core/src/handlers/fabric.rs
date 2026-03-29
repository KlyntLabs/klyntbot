use std::collections::{HashMap, HashSet};

use chrono::Utc;
use cognitive::repos::{CommunityRepo, EntityRow, SqliteBookTreeRepo, SqliteGTLinkRepo};
use context_engine::book_index::{BookTreeRepo, GTLinkRepo, SourceType};
use desktop_shared::commands::fabric::{
    FabricActionParams, FabricActionResponse, FabricCommunity, FabricCommunityDetail,
    FabricEntitiesResponse, FabricEntity, FabricEntityEdge, FabricExpandParams,
    FabricExpandResponse, FabricGraphBase, FabricLink, FabricMember, FabricNote, FabricTreeNode,
    FabricTreeNodesResponse,
};
use desktop_shared::errors::ApiError;
use feature_notes::models::NoteRow;

use crate::errors::{map_cognitive_err, map_storage_err};
use crate::state::AppCore;

const COMMUNITY_COLORS: &[&str] = &[
    "#6366f1", "#f59e0b", "#10b981", "#ef4444", "#8b5cf6", "#ec4899", "#14b8a6", "#f97316",
];

impl AppCore {
    pub async fn fabric_graph_base(&self) -> Result<FabricGraphBase, ApiError> {
        let pool = self.storage_pool.inner().clone();

        let notes = self
            .note_repo
            .list_notes(None)
            .await
            .map_err(map_storage_err)?;

        let link_rows = self
            .note_repo
            .get_all_links()
            .await
            .map_err(map_storage_err)?;

        let links: Vec<FabricLink> = link_rows
            .into_iter()
            .map(|r| FabricLink {
                source_id: r.source_id,
                target_id: r.target_id,
                link_type: "wiki".to_string(),
            })
            .collect();

        let note_ids: Vec<String> = notes.iter().map(|n| n.id.clone()).collect();
        let tags_map = self
            .note_repo
            .get_tags_batch(&note_ids)
            .await
            .map_err(map_storage_err)?;

        // Fetch all note root sections once, then count per source_id
        let tree_repo = SqliteBookTreeRepo::new(pool.clone());
        let all_root_nodes = tree_repo
            .get_root_sections(&SourceType::Note)
            .await
            .map_err(map_cognitive_err)?;

        let mut tree_section_counts: HashMap<String, u32> = HashMap::new();
        for node in &all_root_nodes {
            *tree_section_counts
                .entry(node.source_id.clone())
                .or_default() += 1;
        }

        // Count entities per note in a single query
        let entity_counts: HashMap<String, u32> = sqlx::query_as::<_, (String, i64)>(
            "SELECT source_id, COUNT(*) FROM entities WHERE source_id IS NOT NULL GROUP BY source_id",
        )
        .fetch_all(&pool)
        .await
        .map_err(map_cognitive_err)?
        .into_iter()
        .map(|(id, count)| (id, count as u32))
        .collect();

        let mut fabric_notes: Vec<FabricNote> = notes
            .iter()
            .map(|n| FabricNote {
                id: n.id.clone(),
                title: n.title.clone(),
                notebook_id: n.notebook_id.clone(),
                tags: tags_map.get(&n.id).cloned().unwrap_or_default(),
                body_preview: common::truncate_at_boundary(&n.body, 200).to_string(),
                tree_section_count: tree_section_counts.get(&n.id).copied().unwrap_or(0),
                entity_count: entity_counts.get(&n.id).copied().unwrap_or(0),
            })
            .collect();

        let community_repo = CommunityRepo::new(pool.clone());
        let active_communities = community_repo
            .list_active_communities()
            .await
            .map_err(map_cognitive_err)?;

        let mut fabric_communities: Vec<FabricCommunity> = Vec::new();
        for (idx, c) in active_communities.iter().enumerate() {
            let members = community_repo
                .get_members(&c.id)
                .await
                .map_err(map_cognitive_err)?;

            let mut member_note_ids: Vec<String> = Vec::new();
            for m in &members {
                if let Ok(Some(node)) = tree_repo.get_node(&m.tree_node_id).await {
                    if matches!(
                        node.source_type.as_str(),
                        "note" | "task" | "finance" | "productivity" | "okr" | "learning"
                    ) {
                        member_note_ids.push(node.source_id.clone());
                    }
                }
            }
            member_note_ids.sort();
            member_note_ids.dedup();

            fabric_communities.push(FabricCommunity {
                id: c.id.clone(),
                name: c.name.clone(),
                color: COMMUNITY_COLORS[idx % COMMUNITY_COLORS.len()].to_string(),
                stability: c.stability,
                member_count: member_note_ids.len() as u32,
                member_note_ids,
            });
        }

        // Collect project IDs referenced by community members so we can add them as graph nodes
        let mut project_ids_in_communities: HashSet<String> = HashSet::new();
        for c in &fabric_communities {
            for mid in &c.member_note_ids {
                if !notes.iter().any(|n| n.id == *mid) {
                    project_ids_in_communities.insert(mid.clone());
                }
            }
        }

        // Add projects as FabricNote entries so they appear in the graph
        if !project_ids_in_communities.is_empty() {
            let project_rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT id, name FROM projects WHERE id IN (SELECT value FROM json_each(?1))",
            )
            .bind(
                serde_json::to_string(&project_ids_in_communities.iter().collect::<Vec<_>>())
                    .unwrap_or_default(),
            )
            .fetch_all(&pool)
            .await
            .map_err(map_cognitive_err)?;

            for (id, name) in project_rows {
                fabric_notes.push(FabricNote {
                    id,
                    title: name,
                    notebook_id: None,
                    tags: vec!["project".to_string()],
                    body_preview: String::new(),
                    tree_section_count: 0,
                    entity_count: 0,
                });
            }
        }

        let last_activity_timestamp = active_communities
            .iter()
            .map(|c| c.updated_at.as_str())
            .max()
            .unwrap_or("")
            .to_string();

        let five_mins_ago = Utc::now() - chrono::Duration::minutes(5);
        let live_pulse_active = active_communities.iter().any(|c| {
            chrono::DateTime::parse_from_rfc3339(&c.updated_at)
                .map(|dt| dt > five_mins_ago)
                .unwrap_or(false)
        });

        Ok(FabricGraphBase {
            notes: fabric_notes,
            links,
            communities: fabric_communities,
            suggested_preset: None,
            last_activity_timestamp,
            live_pulse_active,
        })
    }

    pub async fn fabric_graph_expand(
        &self,
        params: FabricExpandParams,
    ) -> Result<FabricExpandResponse, ApiError> {
        let pool = self.storage_pool.inner().clone();

        match params.layer.as_str() {
            "entities" => {
                let entity_rows: Vec<EntityRow> = sqlx::query_as(
                    "SELECT DISTINCT e.* FROM entities e
                     INNER JOIN entity_tree_links l ON l.entity_id = e.id",
                )
                .fetch_all(&pool)
                .await
                .map_err(map_cognitive_err)?;

                let gt_link_repo = SqliteGTLinkRepo::new(pool.clone());

                let mut entities: Vec<FabricEntity> = Vec::new();
                let mut edges: Vec<FabricEntityEdge> = Vec::new();
                let mut seen_edges: HashSet<(String, String)> = HashSet::new();

                for entity in &entity_rows {
                    entities.push(FabricEntity {
                        id: entity.id.clone(),
                        name: entity.name.clone(),
                        entity_type: entity.entity_type.clone(),
                        mention_count: entity.mention_count,
                    });

                    let linked_nodes = gt_link_repo
                        .get_linked_nodes(&entity.id)
                        .await
                        .map_err(map_cognitive_err)?;

                    for node in &linked_nodes {
                        if matches!(
                            node.source_type.as_str(),
                            "note" | "task" | "finance" | "productivity" | "okr" | "learning"
                        ) {
                            let key = (entity.id.clone(), node.source_id.clone());
                            if seen_edges.insert(key) {
                                edges.push(FabricEntityEdge {
                                    entity_id: entity.id.clone(),
                                    note_id: node.source_id.clone(),
                                    weight: 1.0,
                                });
                            }
                        }
                    }
                }

                Ok(FabricExpandResponse::Entities(FabricEntitiesResponse {
                    entities,
                    edges,
                }))
            }

            "tree" => {
                let tree_repo = SqliteBookTreeRepo::new(pool.clone());
                let all_root_nodes = tree_repo
                    .get_root_sections(&SourceType::Note)
                    .await
                    .map_err(map_cognitive_err)?;

                let mut results: Vec<FabricTreeNodesResponse> = Vec::new();
                for note_id in &params.scopes {
                    let mut nodes: Vec<FabricTreeNode> = Vec::new();
                    for root in &all_root_nodes {
                        if root.source_id == *note_id {
                            let subtree = tree_repo
                                .get_subtree(&root.id)
                                .await
                                .map_err(|e| map_cognitive_err(e))?;
                            for node in subtree {
                                nodes.push(FabricTreeNode {
                                    id: node.id,
                                    parent_id: node.parent_id,
                                    node_type: node.node_type.as_str().to_string(),
                                    title: node.title,
                                    content_preview: common::truncate_at_boundary(
                                        &node.content,
                                        100,
                                    )
                                    .to_string(),
                                    level: node.level,
                                });
                            }
                        }
                    }
                    results.push(FabricTreeNodesResponse {
                        note_id: note_id.clone(),
                        nodes,
                    });
                }

                Ok(FabricExpandResponse::Tree(results))
            }

            "community_detail" => {
                let community_repo = CommunityRepo::new(pool.clone());
                let tree_repo = SqliteBookTreeRepo::new(pool.clone());
                let mut details: Vec<FabricCommunityDetail> = Vec::new();

                for community_id in &params.scopes {
                    let Some(community) = community_repo
                        .get_community(community_id)
                        .await
                        .map_err(map_cognitive_err)?
                    else {
                        continue;
                    };

                    let member_rows = community_repo
                        .get_members(community_id)
                        .await
                        .map_err(map_cognitive_err)?;

                    let representative_paths: Vec<String> = community
                        .representative_paths
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_default();

                    let top_entities: Vec<String> = community
                        .top_entities
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_default();

                    let mut members: Vec<FabricMember> = Vec::new();
                    for m in &member_rows {
                        let Ok(Some(node)) = tree_repo.get_node(&m.tree_node_id).await else {
                            continue;
                        };
                        if !matches!(
                            node.source_type.as_str(),
                            "note" | "task" | "finance" | "productivity" | "okr" | "learning"
                        ) {
                            continue;
                        }
                        members.push(FabricMember {
                            note_id: node.source_id,
                            tree_node_id: m.tree_node_id.clone(),
                            membership_score: m.membership_score,
                        });
                    }

                    details.push(FabricCommunityDetail {
                        community_id: community_id.clone(),
                        representative_paths,
                        top_entities,
                        stability_history: vec![community.stability],
                        members,
                    });
                }

                Ok(FabricExpandResponse::CommunityDetail(details))
            }

            other => Err(ApiError::new(
                "INVALID_PARAM",
                format!("unknown layer: {other}"),
            )),
        }
    }

    pub async fn fabric_graph_action(
        &self,
        params: FabricActionParams,
    ) -> Result<FabricActionResponse, ApiError> {
        match params.action.as_str() {
            "create_bridge_note" => {
                let source_community_id = params
                    .payload
                    .get("sourceCommunityId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ApiError::new("INVALID_PARAM", "missing sourceCommunityId"))?
                    .to_string();

                let target_community_id = params
                    .payload
                    .get("targetCommunityId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ApiError::new("INVALID_PARAM", "missing targetCommunityId"))?
                    .to_string();

                let content = params
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let now = Utc::now().to_rfc3339();
                let note_id = uuid::Uuid::new_v4().to_string();

                let row = NoteRow {
                    id: note_id.clone(),
                    notebook_id: None,
                    title: format!(
                        "Bridge: {} \u{2194} {}",
                        source_community_id, target_community_id
                    ),
                    body: content.clone(),
                    body_html: None,
                    body_json: None,
                    pinned: 0,
                    archived: 0,
                    icon: None,
                    color: None,
                    embedding_updated_at: None,
                    split_content: None,
                    split_mode: None,
                    perspective_config: None,
                    last_visited_at: None,
                    created_at: now.clone(),
                    updated_at: now,
                };

                self.note_repo
                    .create_note(&row)
                    .await
                    .map_err(map_storage_err)?;

                // Publish domain event so the cognitive pipeline picks up the new note
                if let Some(bus) = &self.domain_event_bus {
                    bus.publish(bus::DomainEvent::NoteContentChanged {
                        note_id: note_id.clone(),
                        content,
                    });
                }

                Ok(FabricActionResponse {
                    success: true,
                    message: Some(note_id),
                })
            }

            "link_entity" => {
                let entity_id = params
                    .payload
                    .get("entityId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ApiError::new("INVALID_PARAM", "missing entityId"))?
                    .to_string();

                let tree_node_id = params
                    .payload
                    .get("treeNodeId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ApiError::new("INVALID_PARAM", "missing treeNodeId"))?
                    .to_string();

                let pool = self.storage_pool.inner().clone();
                let gt_link_repo = SqliteGTLinkRepo::new(pool);
                gt_link_repo
                    .link(&entity_id, &tree_node_id)
                    .await
                    .map_err(map_cognitive_err)?;

                Ok(FabricActionResponse {
                    success: true,
                    message: Some(format!("Linked entity {entity_id} to node {tree_node_id}")),
                })
            }

            "suggest_merge" | "pin_to_focus" | "highlight_gap" => Ok(FabricActionResponse {
                success: true,
                message: Some("Action acknowledged (hook)".to_string()),
            }),

            other => Err(ApiError::new(
                "INVALID_PARAM",
                format!("unknown action: {other}"),
            )),
        }
    }
}
