# Community Intelligence — LLM Naming, Merge, and Split

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add LLM-powered community naming, merging, and splitting to Reforge Phase 6.5 so communities have human-readable labels and the knowledge graph self-organizes into meaningful clusters.

**Architecture:** Extend Phase 6.5 with a community intelligence step that loads all active communities, sends a single LLM batch call for naming + merge/split decisions, then applies structural changes (rename, merge members, split via Louvain sub-run). A stability guard prevents thrashing. CommunityBuilder keeps its fast heuristic naming for real-time; Reforge overwrites nightly with LLM-quality names.

**Tech Stack:** Rust, SQLite (cognitive crate), LLM via `GraphEnrichmentHandler` trait extension (agent crate), existing Louvain detection, CommunityRepo

**Depends on:** Phase B2 (Phase 6.5 exists), Community detection (Louvain + CommunityBuilder)

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/services/community_intelligence.rs` | Types for community intelligence input/output, merge/split execution logic |

### Modified Files
| File | Change |
|------|--------|
| `crates/cognitive/migrations/004_community_graph.sql` | Add `last_restructured_at` column to `communities` |
| `crates/cognitive/src/repos/community.rs` | Add `rename()`, `merge_communities()`, `get_members_with_edges()`, `delete_community()` methods |
| `crates/cognitive/src/repos/mod.rs` | Bump `cognitive_community` migration version |
| `crates/cognitive/src/services/mod.rs` | Export `community_intelligence` |
| `crates/cognitive/src/services/reforge/mod.rs` | Add `CommunityIntelligenceHandler` trait |
| `crates/cognitive/src/services/reforge/types.rs` | Add community intelligence result fields to `ReforgeResult` |
| `crates/cognitive/src/services/reforge/service.rs` | Add community intelligence step in Phase 6.5, thread `community_repo` |
| `crates/agent/src/adapters/reforge_handlers.rs` | Implement `CommunityIntelligenceHandler` LLM call |
| `crates/app-core/src/init/cron.rs` | Pass `community_repo` + handler to `run_reforge` |

---

### Task 1: Add `last_restructured_at` column and repo methods

**Files:**
- Modify: `crates/cognitive/migrations/004_community_graph.sql`
- Modify: `crates/cognitive/src/repos/community.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Add column to migration**

In `crates/cognitive/migrations/004_community_graph.sql`, add to the `communities` table after `updated_at`:

```sql
    last_restructured_at TEXT      -- timestamp of last merge/split by Reforge
```

- [ ] **Step 2: Add `last_restructured_at` to `CommunityRow`**

In `crates/cognitive/src/repos/community.rs`, add to `CommunityRow` after `updated_at`:

```rust
    pub last_restructured_at: Option<String>,
```

- [ ] **Step 3: Update `upsert_community` to include the new column**

In the INSERT and ON CONFLICT clauses of `upsert_community()`, add `last_restructured_at` as a new bind parameter.

- [ ] **Step 4: Add `rename()` method**

```rust
    /// Rename a community (called by Reforge after LLM naming).
    pub async fn rename(&self, id: &str, new_name: &str) -> Result<()> {
        sqlx::query(
            "UPDATE communities SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
        )
        .bind(new_name)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
```

- [ ] **Step 5: Add `merge_communities()` method**

```rust
    /// Merge community `absorb_id` into `into_id`.
    /// Moves all members, updates counts, deletes the absorbed community.
    pub async fn merge_communities(&self, absorb_id: &str, into_id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;

        // Move members from absorbed → target (ignore duplicates)
        sqlx::query(
            "UPDATE OR IGNORE community_members SET community_id = ?1
             WHERE community_id = ?2",
        )
        .bind(into_id)
        .bind(absorb_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        // Delete any remaining members of absorbed (duplicates that couldn't move)
        sqlx::query("DELETE FROM community_members WHERE community_id = ?1")
            .bind(absorb_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;

        // Update target member count
        sqlx::query(
            "UPDATE communities SET
                member_count = (SELECT COUNT(*) FROM community_members WHERE community_id = ?1),
                stability = 0.5,
                last_restructured_at = datetime('now'),
                updated_at = datetime('now')
             WHERE id = ?1",
        )
        .bind(into_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        // Delete absorbed community
        sqlx::query("DELETE FROM communities WHERE id = ?1")
            .bind(absorb_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;

        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }
```

- [ ] **Step 6: Add `delete_community()` method**

```rust
    /// Delete a community and its member links (for split teardown).
    pub async fn delete_community(&self, id: &str) -> Result<()> {
        // CASCADE handles community_members
        sqlx::query("DELETE FROM communities WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }
```

- [ ] **Step 7: Add `pool()` accessor if missing**

```rust
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
```

- [ ] **Step 8: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, find the `cognitive_community` `FeatureMigration` and bump its version from `1` to `2`.

- [ ] **Step 9: Fix all `CommunityRow` construction sites**

Search for `CommunityRow {` across the codebase. Add `last_restructured_at: None` to every construction. This includes:
- `crates/agent/src/adapters/community_builder.rs` (~line 224)
- Any test files in `crates/cognitive/src/repos/community.rs`

- [ ] **Step 10: Verify**

Run: `cargo build --workspace`
Expected: Compiles.

- [ ] **Step 11: Commit**

```bash
git add crates/cognitive/ crates/agent/
git commit -m "feat(cognitive): add community merge/rename/delete repo methods and last_restructured_at"
```

---

### Task 2: Community intelligence types and service

**Files:**
- Create: `crates/cognitive/src/services/community_intelligence.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/cognitive/src/services/reforge/mod.rs`
- Modify: `crates/cognitive/src/services/reforge/types.rs`

- [ ] **Step 1: Create the community intelligence module**

Create `crates/cognitive/src/services/community_intelligence.rs`:

```rust
//! Community intelligence types and execution logic.
//!
//! Reforge Phase 6.5 uses an LLM to name communities, decide merges, and
//! decide splits. This module defines the input/output types and the
//! execution functions that apply structural changes.

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
const MIN_AGE_FOR_RESTRUCTURE: u32 = 3;

/// Apply community intelligence output: renames, merges, splits.
///
/// Returns (renamed, merged, split) counts.
pub async fn apply_intelligence(
    output: &CommunityIntelligenceOutput,
    community_repo: &CommunityRepo,
    co_activation_repo: &crate::repos::CoActivationRepo,
) -> (u32, u32, u32) {
    let mut renamed = 0u32;
    let mut merged = 0u32;
    let mut split = 0u32;

    // 1. Apply renames
    for rename in &output.names {
        if let Err(e) = community_repo.rename(&rename.community_id, &rename.label).await {
            tracing::debug!("Community rename failed for {}: {e}", rename.community_id);
        } else {
            renamed += 1;
        }
    }

    // 2. Apply merges (capped)
    for merge in output.merges.iter().take(MAX_MERGES_PER_CYCLE) {
        if merge.reason.is_empty() {
            continue; // Skip merges without reasoning
        }
        if let Err(e) = community_repo
            .merge_communities(&merge.absorb_id, &merge.into_id)
            .await
        {
            tracing::debug!(
                "Community merge failed ({} → {}): {e}",
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
    let all_edges = co_activation_repo.list_all().await?;
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
    let now = chrono::Utc::now().to_rfc3339();
    for (idx, group_members) in &new_groups {
        let new_id = format!("{}-sub{}", split.community_id, idx);
        let community = CommunityRow {
            id: new_id.clone(),
            name: format!("Sub-cluster {}", idx + 1), // Heuristic placeholder — next Reforge cycle will LLM-name it
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
    let now = chrono::Utc::now();

    let mut contexts = Vec::new();
    for c in &communities {
        let entities: Vec<String> = c
            .top_entities
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let age_days = chrono::DateTime::parse_from_rfc3339(&c.created_at)
            .map(|dt| (now - dt.with_timezone(&chrono::Utc)).num_days().max(0) as u32)
            .unwrap_or(0);

        // Skip recently restructured communities
        if age_days < MIN_AGE_FOR_RESTRUCTURE {
            // Still include for naming, but won't be merge/split candidate
        }

        contexts.push(CommunityContext {
            id: c.id.clone(),
            current_name: c.name.clone(),
            entities,
            member_count: c.member_count as usize,
            domains: HashMap::new(), // Domain distribution not stored on CommunityRow — LLM infers from entities
            age_days,
        });
    }

    Ok(CommunityIntelligenceInput {
        communities: contexts,
    })
}
```

- [ ] **Step 2: Export the module**

In `crates/cognitive/src/services/mod.rs`, add:

```rust
pub mod community_intelligence;
```

- [ ] **Step 3: Add `CommunityIntelligenceHandler` trait**

In `crates/cognitive/src/services/reforge/mod.rs`, add after `GraphEnrichmentHandler`:

```rust
// ---------------------------------------------------------------------------
// CommunityIntelligenceHandler — Phase 6.5 community naming/merge/split
// ---------------------------------------------------------------------------

/// Bridge trait for LLM-based community intelligence in Phase 6.5.
/// Implemented in the agent crate.
#[async_trait]
pub trait CommunityIntelligenceHandler: Send + Sync {
    /// Name communities, suggest merges and splits in a single LLM call.
    async fn analyze_communities(
        &self,
        input: &crate::services::community_intelligence::CommunityIntelligenceInput,
    ) -> common::Result<crate::services::community_intelligence::CommunityIntelligenceOutput>;
}
```

- [ ] **Step 4: Add result fields to `ReforgeResult`**

In `crates/cognitive/src/services/reforge/types.rs`, add after `snapshot_recorded`:

```rust
    // Community intelligence
    pub communities_renamed: u32,
    pub communities_merged: u32,
    pub communities_split: u32,
```

- [ ] **Step 5: Verify**

Run: `cargo build -p cognitive`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add community intelligence types, execution logic, and handler trait"
```

---

### Task 3: Implement `CommunityIntelligenceHandler` in agent crate

**Files:**
- Modify: `crates/agent/src/adapters/reforge_handlers.rs`

- [ ] **Step 1: Add the prompt**

After `GRAPH_ENRICHMENT_PROMPT`, add:

```rust
const COMMUNITY_INTELLIGENCE_PROMPT: &str = "\
You are a knowledge graph curator. Review these communities and:\n\
1. Generate a short (2-4 word) noun-phrase label for each\n\
2. Identify pairs that should MERGE (same topic, fragmented)\n\
3. Identify any that should SPLIT (multiple unrelated topics)\n\n\
Rules:\n\
- Labels must be unique, human-readable noun phrases\n\
- Only merge when communities clearly overlap (>50% entity similarity)\n\
- Only split when a community has 2+ clearly distinct domains\n\
- Prefer stability: don't merge/split unless evidence is strong\n\
- Skip merge/split for communities younger than 3 days\n\n\
Respond with JSON:\n\
{\"names\": [{\"id\": \"...\", \"label\": \"...\"}], \
\"merges\": [{\"absorb\": \"...\", \"into\": \"...\", \"reason\": \"...\"}], \
\"splits\": [{\"id\": \"...\", \"reason\": \"...\"}]}";
```

- [ ] **Step 2: Add JSON parsing types**

```rust
#[derive(serde::Deserialize)]
struct CommunityIntelligenceResponse {
    #[serde(default)]
    names: Vec<CommunityNameJson>,
    #[serde(default)]
    merges: Vec<CommunityMergeJson>,
    #[serde(default)]
    splits: Vec<CommunitySplitJson>,
}

#[derive(serde::Deserialize)]
struct CommunityNameJson {
    id: String,
    label: String,
}

#[derive(serde::Deserialize)]
struct CommunityMergeJson {
    absorb: String,
    into: String,
    reason: String,
}

#[derive(serde::Deserialize)]
struct CommunitySplitJson {
    id: String,
    reason: String,
}
```

- [ ] **Step 3: Implement the handler**

On the same struct that implements `GraphEnrichmentHandler` (`LlmGraphEnrichmentHandler`), add the `CommunityIntelligenceHandler` impl:

```rust
#[async_trait]
impl cognitive::services::reforge::CommunityIntelligenceHandler for LlmGraphEnrichmentHandler {
    async fn analyze_communities(
        &self,
        input: &cognitive::services::community_intelligence::CommunityIntelligenceInput,
    ) -> common::Result<cognitive::services::community_intelligence::CommunityIntelligenceOutput> {
        use cognitive::services::community_intelligence::*;

        if input.communities.is_empty() {
            return Ok(CommunityIntelligenceOutput::default());
        }

        // Build compact community list for the prompt
        let mut user_msg = String::from("Communities:\n");
        for c in &input.communities {
            user_msg.push_str(&format!(
                "- id={}, name=\"{}\", entities=[{}], members={}, age={}d\n",
                c.id,
                c.current_name,
                c.entities.join(", "),
                c.member_count,
                c.age_days,
            ));
        }

        let messages = vec![
            Message::system(COMMUNITY_INTELLIGENCE_PROMPT),
            Message::user(user_msg),
        ];

        let response = self.provider.chat(&messages, None, &self.params).await?;
        let content = response.content.unwrap_or_default();
        let text = content.trim();

        let parsed: CommunityIntelligenceResponse =
            serde_json::from_str(text).unwrap_or(CommunityIntelligenceResponse {
                names: Vec::new(),
                merges: Vec::new(),
                splits: Vec::new(),
            });

        Ok(CommunityIntelligenceOutput {
            names: parsed
                .names
                .into_iter()
                .map(|n| CommunityRename {
                    community_id: n.id,
                    label: n.label,
                })
                .collect(),
            merges: parsed
                .merges
                .into_iter()
                .map(|m| CommunityMerge {
                    absorb_id: m.absorb,
                    into_id: m.into,
                    reason: m.reason,
                })
                .collect(),
            splits: parsed
                .splits
                .into_iter()
                .map(|s| CommunitySplit {
                    community_id: s.id,
                    reason: s.reason,
                })
                .collect(),
        })
    }
}
```

- [ ] **Step 4: Verify**

Run: `cargo build -p agent`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): implement CommunityIntelligenceHandler for LLM naming/merge/split"
```

---

### Task 4: Wire community intelligence into Reforge Phase 6.5

**Files:**
- Modify: `crates/cognitive/src/services/reforge/service.rs`
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 1: Add new parameters to `run_reforge`**

In `crates/cognitive/src/services/reforge/service.rs`, add to the `run_reforge` signature after `snapshot_repo`:

```rust
    community_intelligence_handler: Option<&dyn super::CommunityIntelligenceHandler>,
    community_repo: Option<&crate::repos::CommunityRepo>,
    co_activation_repo_for_split: Option<&crate::repos::CoActivationRepo>,
```

- [ ] **Step 2: Add community intelligence step in Phase 6.5**

In `service.rs`, inside the Phase 6.5 block, after the knowledge snapshot step (after line ~431 `result.snapshot_recorded = true;`) and BEFORE the closing `} else { debug!("Reforge Phase 6.5: skipped..."); }`, add:

```rust
        // Step 5: Community intelligence — LLM naming, merge, split
        if let (Some(ci_handler), Some(community_repo)) =
            (community_intelligence_handler, community_repo)
        {
            match crate::services::community_intelligence::build_intelligence_input(community_repo)
                .await
            {
                Ok(input) if !input.communities.is_empty() => {
                    match ci_handler.analyze_communities(&input).await {
                        Ok(output) => {
                            let co_act = co_activation_repo_for_split
                                .unwrap_or_else(|| {
                                    // This shouldn't happen but handle gracefully
                                    panic!("co_activation_repo required for community split")
                                });
                            let (renamed, merged, split_count) =
                                crate::services::community_intelligence::apply_intelligence(
                                    &output,
                                    community_repo,
                                    co_act,
                                )
                                .await;
                            result.communities_renamed = renamed;
                            result.communities_merged = merged;
                            result.communities_split = split_count;
                            info!(
                                renamed,
                                merged,
                                split = split_count,
                                "Phase 6.5: community intelligence complete"
                            );
                        }
                        Err(e) => {
                            warn!("Phase 6.5 community intelligence failed: {e}");
                            result
                                .phase_errors
                                .push(format!("community_intelligence: {e}"));
                        }
                    }
                }
                Ok(_) => {
                    debug!("Phase 6.5: no active communities for intelligence");
                }
                Err(e) => {
                    debug!("Phase 6.5: failed to build community input: {e}");
                }
            }
        }
```

Actually, the `co_activation_repo_for_split` shouldn't panic. Fix it to skip the split gracefully:

Replace the `unwrap_or_else` with:
```rust
                            let (renamed, merged, split_count) =
                                if let Some(co_act) = co_activation_repo_for_split {
                                    crate::services::community_intelligence::apply_intelligence(
                                        &output,
                                        community_repo,
                                        co_act,
                                    )
                                    .await
                                } else {
                                    // No co-activation repo — can rename but not split
                                    let no_splits = crate::services::community_intelligence::CommunityIntelligenceOutput {
                                        names: output.names.clone(),
                                        merges: output.merges.clone(),
                                        splits: Vec::new(),
                                    };
                                    crate::services::community_intelligence::apply_intelligence(
                                        &no_splits,
                                        community_repo,
                                        // Create a temporary one from entity_repo pool
                                        &crate::repos::CoActivationRepo::new(entity_repo.pool().clone()),
                                    )
                                    .await
                                };
```

The implementer should pick the cleaner approach. The key is: don't panic, degrade gracefully.

- [ ] **Step 3: Wire in cron handler**

In `crates/app-core/src/init/cron.rs`, where `run_reforge` is called, add the 3 new parameters:

```rust
                            // Community intelligence
                            crate::handlers::cognitive::build_graph_enrichment_handler(
                                &cog_provider,
                                &cog_config,
                            )
                            .as_deref()
                            .map(|h| h as &dyn cognitive::services::reforge::CommunityIntelligenceHandler),
                            Some(&cognitive::CommunityRepo::new(pool.clone())),
                            Some(&co_activation_repo),
```

Wait — `LlmGraphEnrichmentHandler` implements both `GraphEnrichmentHandler` AND `CommunityIntelligenceHandler`, but they're different trait objects. The implementer needs to either:
1. Build a separate handler box for community intelligence, or
2. Use the same handler via trait object coercion

The simplest approach: build a second handler in cron using `build_graph_enrichment_handler` and cast. Or better: add a `build_community_intelligence_handler` function alongside `build_graph_enrichment_handler` in `handlers/cognitive/mod.rs`:

```rust
pub(crate) fn build_community_intelligence_handler(
    cognitive_provider: &Option<providers::DynProvider>,
    config: &config::Config,
) -> Option<Box<dyn cognitive::services::reforge::CommunityIntelligenceHandler>> {
    cognitive_provider.as_ref().map(|cp| {
        let params = providers::cognitive_chat_params(config, 4096);
        Box::new(
            agent::adapters::reforge_handlers::LlmGraphEnrichmentHandler::new(cp.clone(), params),
        ) as Box<dyn cognitive::services::reforge::CommunityIntelligenceHandler>
    })
}
```

Then in cron:
```rust
                            crate::handlers::cognitive::build_community_intelligence_handler(
                                &cog_provider,
                                &cog_config,
                            )
                            .as_deref(),
                            Some(&cognitive::CommunityRepo::new(pool.clone())),
                            Some(&co_activation_repo),
```

- [ ] **Step 4: Fix all `run_reforge` call sites**

Search for all calls to `run_reforge` (cron.rs + integration tests). Add 3 `None` parameters for the new args:

```rust
    None, // community_intelligence_handler
    None, // community_repo
    None, // co_activation_repo_for_split
```

- [ ] **Step 5: Verify**

Run: `cargo build --workspace`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/ crates/agent/ crates/app-core/ tests/
git commit -m "feat(cognitive): wire community intelligence into Reforge Phase 6.5"
```

---

### Task 5: Add `list_all` to CoActivationRepo (if missing)

**Files:**
- Modify: `crates/cognitive/src/repos/co_activation.rs`

- [ ] **Step 1: Check if `list_all` exists**

The `execute_split` function calls `co_activation_repo.list_all()` to get edges for sub-graph Louvain. Check if this method exists. If not, add it:

```rust
    /// List all co-activation edges as (node_a, node_b, weight) tuples.
    pub async fn list_all(&self) -> Result<Vec<(String, String, f64)>> {
        let rows: Vec<(String, String, f64)> = sqlx::query_as(
            "SELECT fact_id_a, fact_id_b, weight FROM co_activation ORDER BY weight DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows)
    }
```

If it already exists with a different signature, adapt `execute_split` to match.

- [ ] **Step 2: Verify**

Run: `cargo build -p cognitive`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): add list_all to CoActivationRepo for community split sub-graph"
```

---

### Task 6: Integration tests and verification

**Files:**
- Modify: `tests/integration/cognitive.rs`

- [ ] **Step 1: Add community naming test**

```rust
#[test]
fn test_community_name_heuristic_filters_junk() {
    use klyntbot::agent::adapters::community_builder::build_community_name;
    use std::collections::HashMap;

    let entities = vec!["Klynt".into(), "Rust".into()];
    let mut domains = HashMap::new();
    domains.insert("work".to_string(), 5);
    let name = build_community_name(&entities, &domains, 8);
    assert!(name.contains("Klynt"), "Name should contain top entity");
    assert!(!name.contains("[user]"), "Name should not contain raw content");
}
```

Note: `build_community_name` may need to be made `pub` for this test. If it's private, test indirectly or make it `pub(crate)`.

- [ ] **Step 2: Add community merge repo test**

```rust
#[tokio::test]
async fn test_community_merge() {
    let pool = klyntbot::cognitive::repos::cognitive_test_pool().await;
    let repo = klyntbot::cognitive::CommunityRepo::new(pool);

    // Create two communities
    let now = chrono::Utc::now().to_rfc3339();
    let c1 = klyntbot::cognitive::repos::CommunityRow {
        id: "c1".into(), name: "Alpha".into(), summary: "s1".into(),
        member_count: 3, modularity_score: None, stability: 0.8,
        top_entities: None, representative_paths: None, source_note_count: None,
        created_at: now.clone(), updated_at: now.clone(), last_restructured_at: None,
    };
    let c2 = klyntbot::cognitive::repos::CommunityRow {
        id: "c2".into(), name: "Beta".into(), summary: "s2".into(),
        member_count: 2, modularity_score: None, stability: 0.7,
        top_entities: None, representative_paths: None, source_note_count: None,
        created_at: now.clone(), updated_at: now.clone(), last_restructured_at: None,
    };
    repo.upsert_community(&c1).await.unwrap();
    repo.upsert_community(&c2).await.unwrap();

    // Merge c2 into c1
    repo.merge_communities("c2", "c1").await.unwrap();

    // c2 should be gone
    assert!(repo.get_community("c2").await.unwrap().is_none());
    // c1 should have stability reset
    let merged = repo.get_community("c1").await.unwrap().unwrap();
    assert!((merged.stability - 0.5).abs() < 0.01);
}
```

- [ ] **Step 3: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero new warnings.

- [ ] **Step 5: Commit**

```bash
git add tests/ crates/
git commit -m "test: add community intelligence integration tests"
```

---

## Summary

| Task | Component | Files | Key Change |
|------|-----------|-------|------------|
| 1 | Repo methods + migration | 3 | merge, rename, delete, last_restructured_at |
| 2 | Types + execution logic | 4 | CommunityIntelligenceInput/Output, apply_intelligence, execute_split |
| 3 | LLM handler | 1 | CommunityIntelligenceHandler impl with batch prompt |
| 4 | Phase 6.5 wiring | 3 | Thread handler+repo into run_reforge, cron wiring |
| 5 | CoActivationRepo.list_all | 1 | Edge listing for split sub-graph |
| 6 | Integration tests | 1 | Merge test, naming test, workspace verification |

**Total: ~13 files modified/created, 6 commits**
