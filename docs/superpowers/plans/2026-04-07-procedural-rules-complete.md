# Procedural Rules: Complete Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make procedural rules a fully functional learning system — rules deduplicate on creation, gain evidence over time via signal reinforcement, auto-expire when stale, can be promoted from MetaRules, and are manageable by users in the UI.

**Architecture:** Five independent improvements to the existing `procedural_rules` infrastructure. Task 1 adds semantic deduplication before upsert in reflection. Task 2 wires `increment_signal_count` into the background consolidation service when extracted facts match existing rules. Task 3 adds rule compaction (90-day stale deactivation) alongside existing fact/episodic compaction. Task 4 bridges `MirrorFacade::approve_meta_rule` to create a ProceduralRule. Task 5 adds deactivate/delete buttons to the Memory tab UI.

**Tech Stack:** Rust, SQLite FTS5, TypeScript/React, Tailwind, tokio, cargo-nextest, bun/vitest

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/cognitive/src/services/reflection.rs` | Modify | Add dedup check before rule upsert |
| `crates/cognitive/src/repos/procedural_rule.rs` | Modify | Add `find_similar(rule_text, domain)` method |
| `crates/cognitive/src/services/background.rs` | Modify | Wire signal reinforcement after fact extraction |
| `crates/cognitive/src/services/compaction.rs` | Modify | Add rule compaction (deactivate stale rules) |
| `crates/cognitive/src/mirror/facade.rs` | Modify | Promote MetaRule → ProceduralRule on approval |
| `desktop-ui/src/features/debug/components/tabs/MemoryTab.tsx` | Modify | Add deactivate button per rule row |

---

### Task 1: Deduplicate rules before upsert in reflection

**Files:**
- Modify: `crates/cognitive/src/repos/procedural_rule.rs`
- Modify: `crates/cognitive/src/services/reflection.rs:176-181`

- [ ] **Step 1: Add `find_similar` method to `ProceduralRuleRepo`**

In `crates/cognitive/src/repos/procedural_rule.rs`, add after `deactivate()` (before the `#[cfg(test)]` block):

```rust
    /// Find an active rule with similar text in the same domain via FTS5.
    /// Returns the best match if its BM25 score indicates high similarity.
    pub async fn find_similar(
        &self,
        rule_text: &str,
        domain: &str,
    ) -> Result<Option<ProceduralRule>, sqlx::Error> {
        // Use FTS5 to find candidates, then check text overlap
        let candidates = self.search_fts(rule_text, Some(domain), 3).await?;
        for candidate in candidates {
            // Simple word-overlap ratio as similarity check
            let overlap = word_overlap_ratio(&candidate.rule_text, rule_text);
            if overlap > 0.6 {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }
```

Add this helper function above the `impl` block (or at the bottom of the file before tests):

```rust
/// Compute word overlap ratio between two strings (Jaccard-like).
fn word_overlap_ratio(a: &str, b: &str) -> f64 {
    let words_a: std::collections::HashSet<&str> = a.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 2)
        .collect();
    let words_b: std::collections::HashSet<&str> = b.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 2)
        .collect();
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let intersection = words_a.intersection(&words_b).count() as f64;
    let union = words_a.union(&words_b).count() as f64;
    intersection / union
}
```

- [ ] **Step 2: Add dedup logic in reflection rule upsert**

In `crates/cognitive/src/services/reflection.rs`, replace lines 176-181:

```rust
    // Apply rule updates
    for rule in &output.rule_updates {
        if let Err(e) = rule_repo.upsert(rule).await {
            warn!("Reflection: failed to upsert rule '{}': {e}", rule.id);
        }
    }
```

with:

```rust
    // Apply rule updates with deduplication
    for rule in &output.rule_updates {
        // Check if a similar rule already exists in the same domain
        match rule_repo.find_similar(&rule.rule_text, &rule.domain).await {
            Ok(Some(existing)) => {
                // Reinforce the existing rule instead of creating a duplicate
                if let Err(e) = rule_repo.increment_signal_count(&existing.id).await {
                    warn!("Reflection: failed to reinforce rule '{}': {e}", existing.id);
                }
                // Update confidence if the new one is higher
                if rule.confidence > existing.confidence {
                    let mut updated = existing.clone();
                    updated.confidence = rule.confidence;
                    updated.updated_at = chrono::Utc::now().to_rfc3339();
                    if let Err(e) = rule_repo.upsert(&updated).await {
                        warn!("Reflection: failed to update confidence for '{}': {e}", updated.id);
                    }
                }
                debug!(
                    "Reflection: reinforced existing rule '{}' (signal_count += 1)",
                    existing.id
                );
            }
            Ok(None) => {
                // No duplicate — insert new rule
                if let Err(e) = rule_repo.upsert(rule).await {
                    warn!("Reflection: failed to upsert rule '{}': {e}", rule.id);
                }
            }
            Err(e) => {
                // FTS search failed — fall back to unconditional upsert
                warn!("Reflection: dedup search failed: {e}, upserting anyway");
                if let Err(e) = rule_repo.upsert(rule).await {
                    warn!("Reflection: failed to upsert rule '{}': {e}", rule.id);
                }
            }
        }
    }
```

Add `use tracing::debug;` if not already imported.

- [ ] **Step 3: Add tests**

In `crates/cognitive/src/repos/procedural_rule.rs` test module:

```rust
    #[test]
    fn test_word_overlap_ratio() {
        assert!(word_overlap_ratio("User works best in mornings", "User works best in the mornings") > 0.6);
        assert!(word_overlap_ratio("User works best in mornings", "Track daily expenses") < 0.3);
        assert!((word_overlap_ratio("", "anything") - 0.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_find_similar_match() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool);

        let r = test_rule("r1", "productivity", "Suggest break after 90 minutes of focused work");
        repo.upsert(&r).await.unwrap();

        let found = repo
            .find_similar("Take a break after 90 minutes of focus work", "productivity")
            .await
            .unwrap();
        assert!(found.is_some(), "should find similar rule");
        assert_eq!(found.unwrap().id, "r1");
    }

    #[tokio::test]
    async fn test_find_similar_no_match() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool);

        let r = test_rule("r1", "productivity", "Morning is peak time");
        repo.upsert(&r).await.unwrap();

        let found = repo
            .find_similar("Track daily expenses carefully", "productivity")
            .await
            .unwrap();
        assert!(found.is_none(), "should not find unrelated rule");
    }
```

- [ ] **Step 4: Build and test**

```bash
cargo build -p cognitive
cargo nextest run -p cognitive -E 'test(procedural) | test(word_overlap) | test(find_similar)' --no-capture
```

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/procedural_rule.rs crates/cognitive/src/services/reflection.rs
git commit -m "feat(cognitive): deduplicate procedural rules during reflection

When the weekly reflection LLM proposes a rule similar to an
existing one (>60% word overlap via Jaccard), the existing rule
is reinforced (signal_count += 1) and its confidence is raised
if the new proposal has higher confidence. Prevents duplicate
rules from accumulating over weeks."
```

---

### Task 2: Wire signal_count reinforcement from fact extraction

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`

- [ ] **Step 1: Add `rule_repo` to `BackgroundServiceConfig`**

In `crates/cognitive/src/services/background.rs`, add to `BackgroundServiceConfig`:

```rust
    pub rule_repo: Option<ProceduralRuleRepo>,
```

Add the import: `use crate::repos::ProceduralRuleRepo;`

Destructure it in `start()` alongside other fields.

- [ ] **Step 2: Add signal reinforcement after fact extraction**

In the main loop, after the episodic memory creation block (around line 452) and before the consolidation prefetch, add:

```rust
                    // Reinforce procedural rules when extracted facts match rule patterns
                    if let Some(ref rule_repo) = rule_repo {
                        for obs in &to_extract {
                            if let Ok(Some(matching_rule)) =
                                rule_repo.find_similar(&obs.content, &obs.domain).await
                            {
                                let _ = rule_repo
                                    .increment_signal_count(&matching_rule.id)
                                    .await;
                            }
                        }
                    }
```

This means: every time the system extracts facts from an observation, it checks if any active procedural rule in the same domain is textually related to that observation. If so, it increments the signal count — the rule is being reinforced by real user behavior.

- [ ] **Step 3: Wire in builder**

In `crates/agent/src/agent_loop/builder.rs`, find where `BackgroundServiceConfig` is constructed. Add:

```rust
                rule_repo: self.pool.as_ref().map(|p| cognitive::repos::ProceduralRuleRepo::new(p.clone())),
```

- [ ] **Step 4: Build and test**

```bash
cargo build -p cognitive -p agent
cargo nextest run -p cognitive -E 'test(background)' --no-capture
```

Fix any compile errors (add `rule_repo: None` to test `BackgroundServiceConfig` constructions).

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/services/background.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(cognitive): wire signal_count reinforcement from fact extraction

When the background consolidation service extracts facts from
observations, it checks if any active procedural rule matches
the observation content (via FTS similarity). Matching rules get
their signal_count incremented, providing real evidence that the
learned pattern is still being observed."
```

---

### Task 3: Add rule compaction (stale rule deactivation)

**Files:**
- Modify: `crates/cognitive/src/repos/procedural_rule.rs`
- Modify: `crates/cognitive/src/services/compaction.rs`

- [ ] **Step 1: Add `deactivate_stale` method to repo**

In `crates/cognitive/src/repos/procedural_rule.rs`, add:

```rust
    /// Deactivate rules that haven't been updated in `days` days and have
    /// fewer than `min_signals` signal count. Returns count deactivated.
    pub async fn deactivate_stale(
        &self,
        days: i64,
        min_signals: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE procedural_rules SET active = 0, updated_at = datetime('now')
             WHERE active = 1
             AND julianday('now') - julianday(updated_at) > ?1
             AND signal_count < ?2",
        )
        .bind(days)
        .bind(min_signals)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
```

- [ ] **Step 2: Add rule compaction to compaction.rs**

In `crates/cognitive/src/services/compaction.rs`:

Add import: `use crate::repos::ProceduralRuleRepo;`

Add constants:

```rust
/// Default: deactivate rules not updated in this many days.
const DEFAULT_RULE_STALE_DAYS: i64 = 90;
/// Minimum signal count for a rule to survive compaction.
const RULE_MIN_SIGNALS: i64 = 2;
```

Add field to `CompactionResult`:

```rust
    pub rules_deactivated: u64,
```

Update `run_compaction` signature to accept `rule_repo`:

```rust
pub async fn run_compaction(
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: Option<&ProceduralRuleRepo>,
) -> Result<CompactionResult, sqlx::Error> {
    run_compaction_with_params(
        fact_repo,
        episodic_repo,
        rule_repo,
        DEFAULT_ARCHIVE_DAYS,
        DEFAULT_EPISODIC_ARCHIVE_DAYS,
        EPISODIC_MIN_ACCESS_COUNT,
    )
    .await
}
```

Update `run_compaction_with_params` to also accept and use `rule_repo`:

```rust
pub async fn run_compaction_with_params(
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: Option<&ProceduralRuleRepo>,
    archive_days: i64,
    episodic_archive_days: i64,
    min_access_count: i64,
) -> Result<CompactionResult, sqlx::Error> {
```

After the existing step 3 (low-stability archive), add step 4:

```rust
    // 4. Deactivate stale procedural rules
    if let Some(rule_repo) = rule_repo {
        let deactivated = rule_repo
            .deactivate_stale(DEFAULT_RULE_STALE_DAYS, RULE_MIN_SIGNALS)
            .await?;
        result.rules_deactivated = deactivated;
        if deactivated > 0 {
            info!("Compaction: deactivated {deactivated} stale procedural rules");
        }
    }
```

- [ ] **Step 3: Fix all callers of `run_compaction`**

Search for all callers: `grep -rn "run_compaction\b" crates/`. Update each to pass the rule_repo parameter. For callers that don't have a rule_repo, pass `None`.

The main caller is in `crates/app-core/src/handlers/cognitive/operations.rs` or `crates/app-core/src/init/cron.rs`. Check with grep and update.

- [ ] **Step 4: Fix tests**

Update the existing compaction tests to pass `None` for rule_repo (or `Some(&rule_repo)` where testing the new behavior):

```rust
    #[tokio::test]
    async fn test_compaction_deactivates_stale_rules() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        // Create an old rule with low signals
        let mut r = ProceduralRule {
            id: "old-rule".into(),
            domain: "productivity".into(),
            rule_text: "Outdated pattern".into(),
            confidence: 0.5,
            source: "reflected".into(),
            signal_count: 0,
            created_at: "2025-01-01".into(),
            updated_at: "2025-01-01".into(),
            active: true,
            project_id: None,
            scope_type: "system".into(),
            scope_id: None,
        };
        rule_repo.upsert(&r).await.unwrap();

        let result = run_compaction(&fact_repo, &episodic_repo, Some(&rule_repo))
            .await
            .unwrap();
        assert_eq!(result.rules_deactivated, 1);

        let active = rule_repo.list_active("productivity").await.unwrap();
        assert!(active.is_empty());
    }
```

- [ ] **Step 5: Build and test**

```bash
cargo build --workspace
cargo nextest run -p cognitive -E 'test(compaction)' --no-capture
```

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/repos/procedural_rule.rs crates/cognitive/src/services/compaction.rs crates/app-core/
git commit -m "feat(cognitive): add stale rule deactivation to compaction

Rules not updated in 90 days with signal_count < 2 are
automatically deactivated during compaction. This prevents
outdated behavioral patterns from persisting indefinitely
in the system prompt."
```

---

### Task 4: Promote MetaRules to ProceduralRules on approval

**Files:**
- Modify: `crates/cognitive/src/mirror/facade.rs`

- [ ] **Step 1: Add `ProceduralRuleRepo` to `MirrorFacade`**

In `crates/cognitive/src/mirror/facade.rs`, find the struct fields. Add:

```rust
    rule_repo: Option<ProceduralRuleRepo>,
```

Add import: `use crate::repos::ProceduralRuleRepo;` and `use crate::types::ProceduralRule;`

Add builder method:

```rust
    pub fn with_rule_repo(mut self, repo: ProceduralRuleRepo) -> Self {
        self.rule_repo = Some(repo);
        self
    }
```

Initialize as `None` in the constructor.

- [ ] **Step 2: Promote on approval**

In the `approve_meta_rule` method (around line 204), after the existing `update_meta_rule_status` call, add:

```rust
        // Promote to procedural rule if we have a rule repo
        if let Some(ref rule_repo) = self.rule_repo {
            // Load the meta-rule to get its content
            if let Ok((active, _)) = self.repo.get_meta_rules_by_status(MetaRuleStatus::Active).await {
                if let Some(meta) = active.iter().find(|r| r.id == rule_id) {
                    let procedural = ProceduralRule {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain: "general".into(),
                        rule_text: format!("{}", meta.action),
                        confidence: meta.effectiveness_score.max(0.6),
                        source: "mirror".into(),
                        signal_count: meta.signal_count as i64,
                        created_at: chrono::Utc::now().to_rfc3339(),
                        updated_at: chrono::Utc::now().to_rfc3339(),
                        active: true,
                        project_id: None,
                        scope_type: "system".into(),
                        scope_id: None,
                    };
                    // Dedup check before inserting
                    match rule_repo.find_similar(&procedural.rule_text, &procedural.domain).await {
                        Ok(Some(existing)) => {
                            let _ = rule_repo.increment_signal_count(&existing.id).await;
                        }
                        _ => {
                            let _ = rule_repo.upsert(&procedural).await;
                        }
                    }
                }
            }
        }
```

Note: check how `MetaRuleAction` displays — it may implement `Display` or you may need to use `meta.trigger_condition` instead of `meta.action`. Read the `MetaRule` struct and `MetaRuleAction` enum to determine the right text to use. Use `grep -n "enum MetaRuleAction" crates/cognitive/src/mirror/types.rs` to check.

- [ ] **Step 3: Wire rule_repo into MirrorFacade at init**

In `crates/app-core/src/init/mod.rs` (or wherever `MirrorFacade` is constructed), find where `.with_episodic_repo()` or similar builder calls are made. Add:

```rust
        .with_rule_repo(cognitive::repos::ProceduralRuleRepo::new(pool.clone()))
```

- [ ] **Step 4: Build and test**

```bash
cargo build --workspace
cargo nextest run -p cognitive -E 'test(mirror) | test(facade)' --no-capture
```

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/mirror/facade.rs crates/app-core/
git commit -m "feat(mirror): promote MetaRules to ProceduralRules on approval

When a user approves a pending MetaRule, it is now also created
as a ProceduralRule (source: 'mirror') with dedup check. This
bridges the mirror self-reflection system with the behavioral
guidelines that appear in the LLM system prompt."
```

---

### Task 5: Add deactivate button to Memory tab UI

**Files:**
- Modify: `desktop-ui/src/features/debug/components/tabs/MemoryTab.tsx`

- [ ] **Step 1: Add deactivate mutation**

In `MemoryTab.tsx`, find the existing mutations (search for `useMutation`). Add:

```typescript
const deactivateRule = useMutation("cognitive_rule_deactivate");
```

- [ ] **Step 2: Add deactivate button to rule rows**

In the Procedural Rules table, find the existing columns (Domain, Rule, Conf, Signals, Active). Add a new `<th>` header and a `<td>` with a button.

Change the table header row to add a 6th column:

```tsx
                <th className="text-left p-2 text-muted-foreground font-normal">Domain</th>
                <th className="text-left p-2 text-muted-foreground font-normal">Rule</th>
                <th className="text-left p-2 text-muted-foreground font-normal">Conf</th>
                <th className="text-left p-2 text-muted-foreground font-normal">Signals</th>
                <th className="text-left p-2 text-muted-foreground font-normal">Active</th>
                <th className="p-2" />
```

In the rule row, after the Active `<td>`, add:

```tsx
                  <td className="p-2">
                    {r.active && (
                      <button
                        type="button"
                        className="text-2xs text-destructive/60 hover:text-destructive"
                        onClick={async () => {
                          await deactivateRule.mutateAsync({ id: r.id });
                          invalidateQueries("cognitive_rules_list");
                          invalidateQueries("cognitive_memory_stats");
                        }}
                      >
                        <Trash2 className="size-3" />
                      </button>
                    )}
                  </td>
```

Update the "No rules" colSpan from 5 to 6:

```tsx
                    <td colSpan={6} className="p-4 text-center text-muted-foreground">
```

- [ ] **Step 3: Verify Tauri command exists**

The backend `cognitive_rule_deactivate` command should already exist. Verify:

```bash
grep -rn "cognitive_rule_deactivate" crates/desktop/src/commands/cognitive.rs
```

If the command takes `{ id: String }` as params, the frontend mutation call matches. If it takes a different shape, adjust the `mutateAsync` call.

- [ ] **Step 4: Build frontend and test**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/debug/components/tabs/MemoryTab.tsx
git commit -m "feat(ui): add deactivate button for procedural rules

Users can now deactivate rules directly from the Memory tab by
clicking the trash icon. The button only appears on active rules.
Invalidates the rules list and stats queries after deactivation."
```

---

### Task 6: Full validation

**Files:** None (validation only)

- [ ] **Step 1: Build full workspace**

```bash
cargo build --workspace
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error" | head -5
```

- [ ] **Step 3: Format**

```bash
cargo fmt --all --check
cd desktop-ui && bun run lint
```

- [ ] **Step 4: Run all Rust tests**

```bash
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

- [ ] **Step 5: Build frontend**

```bash
cd desktop-ui && bun run build
```

- [ ] **Step 6: Commit if needed**

```bash
cargo fmt --all
git add -A
git commit -m "style: format after procedural rules improvements"
```

---

## Summary

| Task | What it fixes | Impact |
|------|--------------|--------|
| 1 | Rule deduplication | No more duplicate rules accumulating over weeks |
| 2 | Signal reinforcement | Rules gain evidence from real user behavior (signal_count > 0) |
| 3 | Stale rule expiration | Outdated rules auto-deactivate after 90 days with low signals |
| 4 | MetaRule → ProceduralRule promotion | User corrections directly create behavioral guidelines |
| 5 | UI deactivate button | Users can remove bad rules from the LLM prompt |
| 6 | Full validation | No regressions |

## Expected Behavior After Implementation

**Before:** Rules accumulated duplicates, never expired, signal_count was always 0, MetaRules were disconnected, users couldn't manage rules.

**After:**
- Week 1: User interacts → facts extracted → rules proposed by reflection with dedup
- Week 2: Same pattern observed → existing rule reinforced (signal_count: 1 → 2 → 3)
- Week 3: User corrects AI 3 times about same thing → MetaRule proposed → user approves → ProceduralRule created with source "mirror"
- Week 4+: Rules with high signals survive compaction; rules with 0 signals after 90 days auto-deactivate
- Anytime: User sees a bad rule → clicks trash icon → rule deactivated → gone from system prompt within 60s

The system prompt's `## Learned Patterns` section now shows rules with real evidence:
```
### productivity
- Suggest break after 90min focus (confidence: 85%, signals: 12)
- Schedule complex tasks before noon (confidence: 75%, signals: 7)
### general
- Ask for clarification on ambiguous requests (confidence: 70%, signals: 4, source: mirror)
```
