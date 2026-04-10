# Community Intelligence — LLM Naming, Merge, and Split in Reforge Phase 6.5

**Date:** 2026-04-10
**Status:** Draft
**Depends on:** Phase B2 (Phase 6.5 graph consolidation), Community detection (Louvain + CommunityBuilder)

## Problem Statement

Communities are formed by Louvain over co-activation edges. The algorithm produces structurally valid clusters, but:

1. **Names are meaningless** — built by concatenating truncated tree node titles ("User & I'm Jayden, a softw...")
2. **No structural refinement** — Louvain runs mechanically; communities that should merge (sparse edges between related topics) or split (one broad cluster covering unrelated domains) persist indefinitely
3. **No feedback loop** — Reforge sees community health metrics but can't act on them

Communities are the 9th factor in 12-factor retrieval scoring (weight 0.15 — joint highest). Bad communities = bad retrieval. This is not cosmetic.

## Design

### Overview

Reforge Phase 6.5 gains three new capabilities, all LLM-driven in a single batch call:

1. **Name** — generate 2-4 word noun-phrase labels for each community
2. **Merge** — identify community pairs that should be combined (fragmented topics)
3. **Split** — identify communities that should be divided (overly broad groupings)

CommunityBuilder continues using the heuristic for immediate naming at rebuild time. Reforge Phase 6.5 runs nightly and overwrites with LLM-quality names + applies structural changes.

### Token Budget

Single batch call per nightly cycle:
- Input: ~50 tokens per community (entities, member count, domains, current name)
- For 15 communities: ~750 tokens input + ~200 tokens output = ~1K tokens total
- This is less than a single chat message. Negligible cost.

### Phase 6.5 Extension

Current Phase 6.5 flow (from B2):
1. Load medium-density turns
2. Find duplicate entity candidates
3. Run LLM enrichment (entity resolution)
4. Apply merges + relationships
5. Record knowledge snapshot

**New step inserted after step 4, before snapshot:**

```
5. Community intelligence (NEW)
   a. Load all active communities with members, entities, domains
   b. Single LLM call: name + merge + split decisions
   c. Apply: rename communities, execute merges, execute splits
   d. Log decisions in knowledge snapshot metrics
```

### LLM Prompt Design

```
You are a knowledge graph curator. Review these communities and:
1. Generate a short (2-4 word) noun-phrase label for each
2. Identify pairs that should MERGE (same topic, fragmented by structure)
3. Identify any that should SPLIT (multiple unrelated topics in one group)

Rules:
- Labels must be unique, human-readable noun phrases
- Only merge when communities clearly overlap (>50% entity similarity)
- Only split when a community spans 2+ clearly distinct domains
- Prefer stability: don't merge/split unless the evidence is strong

Communities:
[
  {"id": "comm-abc", "current_name": "Klynt & Rust", "entities": ["Klynt", "Rust", "SQLite"], "members": 12, "domains": {"work": 8, "skill": 4}},
  {"id": "comm-def", "current_name": "Buildkite & Sarah", "entities": ["Buildkite", "Sarah", "GitHub Actions"], "members": 6, "domains": {"work": 4, "task": 2}},
  ...
]

Respond with JSON:
{
  "names": [{"id": "comm-abc", "label": "Klynt Development"}, ...],
  "merges": [{"absorb": "comm-def", "into": "comm-abc", "reason": "CI migration is part of Klynt project"}],
  "splits": [{"id": "comm-xyz", "into": ["Domain A facts", "Domain B facts"], "reason": "Spans finance and identity with no connection"}]
}
```

### Merge Execution

When Reforge decides to merge community A into community B:

1. Move all members of A to B (update `community_members.community_id`)
2. Recompute B's summary, top_entities, member_count
3. Delete community A from `communities` table
4. Delete A's vector embedding
5. B gets the LLM-assigned name (not a concatenation of A+B names)

### Split Execution

When Reforge decides to split community C:

1. The LLM provides guidance on which domain/entity cluster forms each new sub-community
2. Re-run Louvain on C's members only (sub-graph detection)
3. If Louvain produces 2+ clusters: create new communities from them, name via the batch response
4. If Louvain still produces 1 cluster: abort the split (structure doesn't support it)
5. Delete original community C

The key insight: **Reforge suggests, Louvain validates.** Reforge says "this should split" but Louvain's structural detection must confirm the split is structurally sound. This prevents hallucinated splits.

### Stability Guard

To prevent thrashing (merge Monday, split Tuesday, merge Wednesday):

- **Cooldown**: Communities merged or created by split get `stability = 0.5` and a `last_restructured_at` timestamp
- **Minimum age**: Communities younger than 3 days are excluded from merge/split candidates
- **Confidence threshold**: LLM must provide a reason; empty reasons are ignored
- **Max changes per cycle**: At most 2 merges + 1 split per nightly run

### Data Flow

```
CommunityBuilder (real-time)          Reforge Phase 6.5 (nightly)
  Louvain detection                     Load active communities
  Heuristic naming (fast)         -->   LLM batch: name + merge + split
  Persist to communities table          Apply renames
  Embed summaries                       Execute merges (move members)
                                        Execute splits (Louvain sub-run)
                                        Update snapshot with decisions
                                        Feed community quality to Phase 1
```

### Handler Trait

Extend the existing `GraphEnrichmentHandler` (or create a new trait — TBD during planning):

```rust
pub struct CommunityIntelligenceInput {
    pub communities: Vec<CommunityContext>,
}

pub struct CommunityContext {
    pub id: String,
    pub current_name: String,
    pub entities: Vec<String>,
    pub member_count: usize,
    pub domains: HashMap<String, usize>,
    pub age_days: u32,
}

pub struct CommunityIntelligenceOutput {
    pub names: Vec<(String, String)>,       // (community_id, new_label)
    pub merges: Vec<CommunityMerge>,
    pub splits: Vec<CommunitySplit>,
}

pub struct CommunityMerge {
    pub absorb_id: String,
    pub into_id: String,
    pub reason: String,
}

pub struct CommunitySplit {
    pub community_id: String,
    pub reason: String,
}
```

### ReforgeResult Extension

Add to `ReforgeResult`:
```rust
pub communities_renamed: u32,
pub communities_merged: u32,
pub communities_split: u32,
```

### Success Criteria

| # | Criterion | Metric |
|---|-----------|--------|
| 1 | Community names are human-readable | No raw content or UUIDs in community names after first Reforge run |
| 2 | Names are unique | No two communities share the same label |
| 3 | Merges reduce fragmentation | Communities with >50% entity overlap get merged within 2 cycles |
| 4 | Splits improve precision | Communities spanning 3+ unrelated domains get split |
| 5 | Stability | No community is merged and re-split within 7 days |
| 6 | Token cost | <2K tokens per nightly cycle for community intelligence |
| 7 | Graceful degradation | LLM failure → heuristic names preserved, no structural changes |

### Affected Files

| File | Change |
|------|--------|
| `crates/cognitive/src/services/reforge/service.rs` | Add community intelligence step in Phase 6.5 |
| `crates/cognitive/src/services/reforge/mod.rs` | Extend `GraphEnrichmentHandler` or add new trait |
| `crates/cognitive/src/services/reforge/types.rs` | Add `CommunityIntelligenceInput/Output`, extend `ReforgeResult` |
| `crates/cognitive/src/repos/community.rs` | Add `merge_communities()`, `split_community()`, `rename()`, `list_merge_candidates()` |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add `last_restructured_at` to `communities` table |
| `crates/agent/src/adapters/reforge_handlers.rs` | Implement community intelligence LLM call |
| `crates/agent/src/adapters/community_builder.rs` | Keep heuristic naming (already done) |
| `crates/app-core/src/init/cron.rs` | Pass community_repo to Phase 6.5 |

### Non-Goals

- Real-time LLM naming in CommunityBuilder (too expensive per rebuild)
- User-facing UI for manual community management (separate spec)
- Hierarchical community nesting (future: communities within communities)
