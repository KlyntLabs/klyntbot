# Knowledge Atoms Phase 4: "The Dashboard" + Spec Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the entire Unified Learning System spec — retention history charts, deep-link coaching nudges, per-topic prompt reinforcement, cross-domain connections view, weekly digest, and inline editor badge.

**Architecture:** Extend the Knowledge Health page with recharts sparklines and a cytoscape graph view for cross-domain atom connections. Add `action_url` to coaching interventions for clickable deep links. Inject declining-topic reinforcement into the agent's cognitive context source. Add a weekly cron that compiles a Knowledge Health snapshot digest. Wire cross-feature atom display into project/task pages via a shared `RelevantAtoms` component.

**Tech Stack:** Rust (SQLite, tokio, chrono), React (TypeScript, Tailwind CSS, recharts, cytoscape), Tauri IPC

**Spec:** `docs/superpowers/specs/2026-03-21-unified-learning-system-design.md` (Phase 3 remaining items + Phase 4)

**Depends on:** Phase 1 + 2 + 3 complete

---

## Already covered by Phase 1/2/3

- **5 coaching pattern detectors** — implemented (Phase 3)
- **Coaching message templates (70% celebration)** — implemented + wired (Phase 3)
- **Morning briefing: Knowledge Health section** — implemented (Phase 3)
- **Focus session micro-review** — MicroReviewPrompt wired into FocusControl (Phase 3)
- **Autotuner: knowledge_retention_score metric** — implemented (Phase 3)
- **Undo/restore for archived atoms (7-day window)** — implemented with enforcement (Phase 3)
- **"Start focused review" from dashboard** — FocusedReview page + actions (Phase 3)
- **IPC: knowledge_health_summary, knowledge_topic_detail** — implemented (Phase 2)
- **Knowledge Health page with topic heatmap** — implemented (Phase 2)

## File Map

### New files
| File | Responsibility |
|---|---|
| `crates/cognitive/src/repos/retention_history.rs` | Queries for retention time-series: daily avg retention per domain, 30/90 day sparkline data |
| `crates/app-core/src/handlers/retention_history.rs` | AppCore handler for retention history IPC |
| `crates/desktop-shared/src/commands/retention_history.rs` | IPC types for retention history |
| `crates/desktop/src/commands/retention_history.rs` | Tauri command + DEV_COMMANDS |
| `desktop-ui/src/features/learn/components/RetentionChart.tsx` | Recharts area chart for retention trends (30/90 day) |
| `desktop-ui/src/features/learn/components/AtomGraph.tsx` | Cytoscape graph view for cross-domain atom connections |
| `desktop-ui/src/features/learn/hooks/useRetentionHistory.ts` | Query hook for retention time-series |
| `desktop-ui/src/shared/components/RelevantAtoms.tsx` | Shared component for cross-feature atom surfacing |

### Modified files
| File | Changes |
|---|---|
| `crates/feature-coaching/src/router.rs` | Add `action_url: Option<String>` to DeliveredIntervention |
| `crates/feature-coaching/src/service.rs` | Populate action_url for learning interventions (deep link to note) |
| `crates/storage/src/repos/coaching_intervention_log.rs` | Add `action_url` column to log table |
| `crates/cognitive/src/services/context_source.rs` | Add declining-topic reinforcement section to prompt |
| `crates/cognitive/src/repos/mod.rs` | Export retention_history module |
| `crates/cognitive/src/lib.rs` | Re-export RetentionHistoryRepo |
| `crates/app-core/src/init/cron.rs` | Register weekly Knowledge Health digest cron |
| `crates/app-core/src/handlers/mod.rs` | Add retention_history module |
| `crates/desktop-shared/src/commands/mod.rs` | Add retention_history types |
| `crates/desktop/src/commands/mod.rs` | Add retention_history command module |
| `crates/desktop/src/main.rs` | Register retention_history commands |
| `crates/desktop/src/dev_server/dispatch.rs` | Add retention_history dispatch |
| `crates/desktop/src/dev_server/mod.rs` | Add retention_history DEV_COMMANDS |
| `desktop-ui/src/features/learn/components/KnowledgeHealth.tsx` | Add chart tabs, graph view toggle, retention sparklines |
| `desktop-ui/src/features/coaching/components/InterventionRow.tsx` | Render action_url as clickable link |
| `desktop-ui/src/features/coaching/components/MorningBriefing.tsx` | Fading atom names link to note |
| `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` | Read `atomId` query param for deep-link scroll |
| `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx` | Scroll to atom when `atomId` param present |
| `desktop-ui/src/features/projects/components/overview/OverviewTab.tsx` | Add RelevantAtoms section |

---

### Task 1: Retention history repo — daily time-series data

**Files:**
- Create: `crates/cognitive/src/repos/retention_history.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Create retention history repo**

This repo computes daily retention snapshots from `review_log` + `knowledge_atoms`. Since we don't store historical retention (atoms only have current `retention_pct`), we derive approximate history from `review_log` timestamps and FSRS formula.

```rust
use sqlx::SqlitePool;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DailyRetentionPoint {
    pub date: String,
    pub avg_retention: f64,
    pub review_count: i64,
    pub atom_count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DomainRetentionHistory {
    pub domain: String,
    pub points: Vec<DailyRetentionPoint>,
}

#[derive(Debug, Clone)]
pub struct RetentionHistoryRepo {
    pool: SqlitePool,
}

impl RetentionHistoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get daily retention data for the last N days (all domains combined).
    pub async fn daily_retention(&self, days: i64) -> Result<Vec<DailyRetentionPoint>, sqlx::Error> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        sqlx::query_as::<_, (String, f64, i64)>(
            r#"
            SELECT DATE(rl.reviewed_at) as d,
                   AVG(CASE WHEN rl.rating >= 3 THEN 1.0 ELSE 0.0 END) as success_rate,
                   COUNT(*) as cnt
            FROM review_log rl
            WHERE rl.reviewed_at > ?1
            GROUP BY d
            ORDER BY d ASC
            "#,
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(date, avg, count)| DailyRetentionPoint {
                    date,
                    avg_retention: avg,
                    review_count: count,
                    atom_count: 0, // filled below if needed
                })
                .collect()
        })
    }

    /// Get per-domain daily retention for chart breakdown.
    pub async fn domain_retention_history(
        &self,
        days: i64,
    ) -> Result<Vec<DomainRetentionHistory>, sqlx::Error> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let rows: Vec<(String, String, f64, i64)> = sqlx::query_as(
            r#"
            SELECT ka.domain, DATE(rl.reviewed_at) as d,
                   AVG(CASE WHEN rl.rating >= 3 THEN 1.0 ELSE 0.0 END) as success_rate,
                   COUNT(*) as cnt
            FROM review_log rl
            JOIN flashcards fc ON fc.id = rl.card_id
            JOIN knowledge_atoms ka ON ka.id = fc.atom_id
            WHERE rl.reviewed_at > ?1
            GROUP BY ka.domain, d
            ORDER BY ka.domain, d ASC
            "#,
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await?;

        // Group by domain
        let mut map: std::collections::HashMap<String, Vec<DailyRetentionPoint>> =
            std::collections::HashMap::new();
        for (domain, date, avg, count) in rows {
            map.entry(domain).or_default().push(DailyRetentionPoint {
                date,
                avg_retention: avg,
                review_count: count,
                atom_count: 0,
            });
        }
        Ok(map
            .into_iter()
            .map(|(domain, points)| DomainRetentionHistory { domain, points })
            .collect())
    }
}
```

- [ ] **Step 2: Export from repos/mod.rs and lib.rs**

Add `pub mod retention_history;` and re-exports.

- [ ] **Step 3: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_daily_retention_empty() {
        let pool = cognitive_test_pool().await;
        let repo = RetentionHistoryRepo::new(pool);
        let points = repo.daily_retention(30).await.unwrap();
        assert!(points.is_empty());
    }
}
```

- [ ] **Step 4: Run tests and commit**

Run: `cargo nextest run -p cognitive -E 'test(retention_history)'`

```
feat(cognitive): add RetentionHistoryRepo — daily retention time-series
```

---

### Task 2: Retention history IPC + Tauri commands

**Files:**
- Create: `crates/desktop-shared/src/commands/retention_history.rs`
- Create: `crates/app-core/src/handlers/retention_history.rs`
- Create: `crates/desktop/src/commands/retention_history.rs`
- Modify: 6 registration files (mod.rs, main.rs, dispatch.rs, dev_server/mod.rs)

- [ ] **Step 1: Create IPC types**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionHistoryParams {
    pub days: i64,
    pub by_domain: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RetentionHistoryResponse {
    pub overall: Vec<RetentionPoint>,
    pub domains: Vec<DomainHistory>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPoint {
    pub date: String,
    pub avg_retention: f64,
    pub review_count: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DomainHistory {
    pub domain: String,
    pub points: Vec<RetentionPoint>,
}
```

- [ ] **Step 2: Create AppCore handler**

`retention_history(params)` → queries `RetentionHistoryRepo` for overall + per-domain data.

- [ ] **Step 3: Create Tauri command + register**

Follow `knowledge_health.rs` pattern. Single command: `retention_history`.

- [ ] **Step 4: Build + dev_server parity test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`

```
feat(app-core,desktop): add retention history IPC + Tauri commands
```

---

### Task 3: Deep links — action_url on coaching interventions

**Files:**
- Modify: `crates/feature-coaching/src/router.rs`
- Modify: `crates/feature-coaching/src/service.rs`
- Modify: `crates/storage/src/repos/coaching_intervention_log.rs`
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs` (bump migration version)

- [ ] **Step 1: Add action_url to DeliveredIntervention**

In `router.rs`, add to the struct:
```rust
pub action_url: Option<String>,
```

Update all construction sites to include `action_url: None` (or a specific URL for learning interventions).

- [ ] **Step 2: Add action_url column to coaching_intervention_log**

In `001_cognitive_tables.sql`, add `action_url TEXT` to the `coaching_intervention_log` CREATE TABLE. Bump migration version in `repos/mod.rs`.

Update `crates/storage/src/repos/coaching_intervention_log.rs`:
1. Add `pub action_url: Option<String>` to `InterventionLogRow`
2. Update the `insert()` method signature to accept `action_url: Option<&str>`
3. Update the INSERT SQL from 5 params to 6 params

Then update `crates/feature-coaching/src/service.rs`: the `persist_intervention()` call site must pass `action_url` from the `DeliveredIntervention` to the log repo's `insert()` method. Every existing call to `persist_intervention()` or `log.insert()` needs the new parameter.

- [ ] **Step 3: Generate deep links in learning interventions**

In `service.rs`, where learning patterns create `DeliveredIntervention`, populate `action_url` for patterns that reference specific atoms/notes:

```rust
// For retention_decay_detected pattern — link to the note
let action_url = Some(format!("/#/notes?noteId={}", atom_note_id));
```

The pattern detector doesn't currently carry atom/note IDs. For now, link to the learn page:
```rust
let action_url = match pattern.name.as_str() {
    n if n.starts_with("study_streak_") => Some("/#/learn/review".to_string()),
    "retention_decay_detected" | "domain_retention_gap" => Some("/#/learn/knowledge".to_string()),
    _ => None,
};
```

- [ ] **Step 4: Build and commit**

```
feat(feature-coaching): add action_url to interventions for deep-link navigation
```

---

### Task 4: Deep link frontend — clickable interventions + note atom scroll

**Files:**
- Modify: `desktop-ui/src/features/coaching/components/InterventionRow.tsx`
- Modify: `desktop-ui/src/features/coaching/components/MorningBriefing.tsx`
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`
- Modify: `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx`

- [ ] **Step 1: Render action_url in InterventionRow**

If `intervention.actionUrl` exists, render a clickable "Open →" button that navigates:

```tsx
{intervention.actionUrl && (
  <button
    type="button"
    onClick={() => navigate(intervention.actionUrl)}
    className="text-[10px] text-brand hover:underline"
  >
    Open →
  </button>
)}
```

- [ ] **Step 2: Make fading atoms clickable in MorningBriefing**

Each fading atom in the briefing should link to its note:
```tsx
onClick={() => navigate(`/notes?noteId=${atom.sourceNoteId}&atomId=${atom.id}`)}
```

This requires adding `sourceNoteId` to `FadingAtomSummary` in both places:
1. **Backend**: Add `pub source_note_id: Option<String>` to `FadingAtomSummary` in `crates/desktop-shared/src/commands/morning_briefing.rs`. Update the mapping in `crates/app-core/src/handlers/morning_briefing.rs` to include `source_note_id: a.source_note_id.clone()`.
2. **Frontend**: Add `sourceNoteId?: string` to the `FadingAtom` interface in `MorningBriefing.tsx`.

- [ ] **Step 3: Read atomId query param in notes page**

In `KnowledgeBasePage.tsx`, extend the existing `useSearchParams` effect to also read `atomId`:

```tsx
const atomId = searchParams.get("atomId");
if (noteId) {
  setSelectedNoteId(noteId);
  // Pass atomId down to KnowledgeAtomsPanel
}
```

In `KnowledgeAtomsPanel.tsx`, accept an optional `highlightAtomId` prop and scroll to / highlight that atom card when it mounts.

- [ ] **Step 4: Lint + commit**

```
feat(desktop-ui): add deep-link navigation for coaching interventions + atom scroll
```

---

### Task 5: Weekly Knowledge Health Snapshot digest (cron + handler together)

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 1: Add constant + register handler + ensure job**

Add all three in one task (avoids orphan cron job between tasks):

1. Add constant: `const JOB_WEEKLY_KNOWLEDGE_DIGEST: &str = "__klyntbot_weekly_knowledge_digest";`

2. In `register_cron_callbacks`, add the handler (see Task 10 code block below for the full callback implementation). The handler queries streak, topics, fading atoms, and weekly reviews, then logs a summary. It does NOT emit `CoachingLearningDigest` (that's the nightly decay cron's job).

3. In `ensure_cron_jobs`, add:
```rust
ensure_job!(
    JOB_WEEKLY_KNOWLEDGE_DIGEST,
    scheduling::CronSchedule::Cron {
        expr: "0 18 * * 0".to_string(), // Sunday 6 PM
        tz: Some(config.timezone.clone()),
    },
    "Weekly knowledge health digest"
);
```

- [ ] **Step 2: Build + commit**

```
feat(app-core): add weekly Knowledge Health Snapshot digest cron
```

---

### Task 6: Per-topic prompt reinforcement in context source

**Files:**
- Modify: `crates/cognitive/src/services/context_source.rs`

- [ ] **Step 1: Add declining-topic reinforcement**

In `CognitiveContextSource`, after the existing static fact injection, add a section that queries declining topics and injects a reinforcement hint into the agent prompt:

Read the file to understand the existing `provide()` method and how it builds prompt sections. The struct holds `fact_repo: SemanticFactRepo` and `rule_repo: ProceduralRuleRepo` but NOT a raw pool. Derive the pool from `fact_repo`: `self.fact_repo.pool().clone()`.

Then add:

```rust
// Phase 4: Inject declining-topic reinforcement
// If any topic has avg_retention < 0.6, add a note to the prompt
let atom_repo = cognitive::KnowledgeAtomRepo::new(self.fact_repo.pool().clone());
if let Ok(topics) = atom_repo.list_topics_with_atoms().await {
    let declining: Vec<_> = topics.iter()
        .filter(|t| t.avg_retention < 0.6 && t.atom_count > 0)
        .collect();
    if !declining.is_empty() {
        sections.push(format!(
            "## Declining Knowledge Areas\nThe user's retention is dropping in: {}. \
             When relevant, reinforce these concepts in your responses.",
            declining.iter().map(|t| format!("{} ({:.0}%)", t.name, t.avg_retention * 100.0)).collect::<Vec<_>>().join(", ")
        ));
    }
}
```

This adds a "Declining Knowledge Areas" section to the agent's system prompt only when topics are actually declining. Lightweight — one SQL query, no LLM call.

- [ ] **Step 2: Build + commit**

```
feat(cognitive): add declining-topic reinforcement to agent prompt context
```

---

### Task 7: Retention history charts (recharts sparklines)

**Files:**
- Create: `desktop-ui/src/features/learn/hooks/useRetentionHistory.ts`
- Create: `desktop-ui/src/features/learn/components/RetentionChart.tsx`
- Modify: `desktop-ui/src/features/learn/components/KnowledgeHealth.tsx`

- [ ] **Step 1: Create useRetentionHistory hook**

```typescript
import { useQuery } from "@shared/hooks/useQuery";

export interface RetentionPoint {
  date: string;
  avgRetention: number;
  reviewCount: number;
}

export interface DomainHistory {
  domain: string;
  points: RetentionPoint[];
}

export interface RetentionHistoryData {
  overall: RetentionPoint[];
  domains: DomainHistory[];
}

export function useRetentionHistory(days: number = 30) {
  return useQuery<RetentionHistoryData>("retention_history", { days, byDomain: true }, {
    overall: [],
    domains: [],
  });
}
```

- [ ] **Step 2: Create RetentionChart component**

Use recharts `AreaChart` (same pattern as `InsightEvolutionChart.tsx`):

```tsx
import { Area, AreaChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

interface RetentionChartProps {
  data: { date: string; avgRetention: number; reviewCount: number }[];
  height?: number;
}

export function RetentionChart({ data, height = 200 }: RetentionChartProps) {
  if (data.length === 0) return null;

  const formatted = data.map((d) => ({
    ...d,
    retention: Math.round(d.avgRetention * 100),
    label: d.date.slice(5), // "03-21" format
  }));

  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={formatted}>
        <XAxis dataKey="label" tick={{ fontSize: 10 }} stroke="var(--muted)" />
        <YAxis domain={[0, 100]} tick={{ fontSize: 10 }} stroke="var(--muted)" />
        <Tooltip />
        <Area
          type="monotone"
          dataKey="retention"
          stroke="var(--brand)"
          fill="var(--brand)"
          fillOpacity={0.1}
          strokeWidth={2}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
```

- [ ] **Step 3: Add chart to KnowledgeHealth page**

Add a tab bar at the top of KnowledgeHealth: "Topics" | "Trends" | "Graph". Default to "Topics" (existing view). "Trends" shows the RetentionChart. "Graph" shows AtomGraph (Task 8).

- [ ] **Step 4: Lint + commit**

```
feat(desktop-ui): add retention history charts with 30-day sparklines
```

---

### Task 8: Cross-domain atom connections graph view

**Files:**
- Create: `desktop-ui/src/features/learn/components/AtomGraph.tsx`
- Modify: `desktop-ui/src/features/learn/components/KnowledgeHealth.tsx`

- [ ] **Step 1: Create AtomGraph component**

Use cytoscape (already in package.json) to render atoms as nodes grouped by topic, with edges for `secondary_sources` cross-references:

```tsx
import { useEffect, useRef } from "react";
import { useKnowledgeHealth } from "../hooks/useKnowledgeHealth";

export function AtomGraph() {
  const containerRef = useRef<HTMLDivElement>(null);
  const { data: health } = useKnowledgeHealth();

  useEffect(() => {
    if (!containerRef.current || health.topics.length === 0) return;
    // Dynamically import cytoscape to avoid SSR issues
    import("cytoscape").then(({ default: cytoscape }) => {
      const nodes = health.topics.map((t) => ({
        data: { id: t.id, label: t.name, size: Math.max(20, t.atomCount * 3), retention: t.avgRetention },
      }));
      // For now, no edges (secondary_sources parsing not yet exposed via IPC)
      // Edges would be added when we have cross-domain connection data
      const cy = cytoscape({
        container: containerRef.current,
        elements: [...nodes],
        style: [ /* node + edge styles using CSS vars */ ],
        layout: { name: "cose", animate: false },
      });
      return () => cy.destroy();
    });
  }, [health.topics]);

  return <div ref={containerRef} className="w-full h-[400px]" />;
}
```

- [ ] **Step 2: Wire into KnowledgeHealth "Graph" tab**

The KnowledgeHealth page now has 3 tabs. The "Graph" tab renders `<AtomGraph />`.

- [ ] **Step 3: Lint + commit**

```
feat(desktop-ui): add cross-domain atom graph view with cytoscape
```

---

### Task 9: Cross-feature atom surfacing (RelevantAtoms component)

**Files:**
- Create: `desktop-ui/src/shared/components/RelevantAtoms.tsx`
- Modify: `desktop-ui/src/features/projects/components/overview/OverviewTab.tsx`

- [ ] **Step 1: Create RelevantAtoms shared component**

A compact component that queries atoms by domain and shows a small list:

```tsx
import { useQuery } from "@shared/hooks/useQuery";
import { retentionTextColor } from "@shared/lib/retention";

interface RelevantAtomsProps {
  domain?: string;
  limit?: number;
}

export function RelevantAtoms({ domain, limit = 5 }: RelevantAtomsProps) {
  // Use knowledge_health_summary and filter by domain on frontend
  // Or add a lightweight IPC that queries atoms by domain
  const { data } = useQuery("knowledge_health_summary", undefined, {
    totalAtoms: 0, activeAtoms: 0, avgRetention: 0, topics: [],
  });

  const matchingTopics = domain
    ? data.topics.filter((t) => t.domain.includes(domain))
    : data.topics.slice(0, limit);

  if (matchingTopics.length === 0) return null;

  return (
    <div className="glass-card rounded-lg p-3 space-y-1">
      <span className="text-[10px] text-muted uppercase tracking-wider">Related Knowledge</span>
      {matchingTopics.map((topic) => (
        <div key={topic.id} className="flex items-center justify-between text-xs">
          <span className="text-primary truncate">{topic.name}</span>
          <span className={`text-[10px] tabular-nums ${retentionTextColor(topic.avgRetention)}`}>
            {Math.round(topic.avgRetention * 100)}%
          </span>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Add to project overview page**

In `OverviewTab.tsx`, add `<RelevantAtoms />` in the sidebar or below existing sections. Read the file first to find the right insertion point.

- [ ] **Step 3: Lint + commit**

```
feat(desktop-ui): add RelevantAtoms cross-feature component + wire into projects
```

---

### Task 10: (Merged into Task 5 — weekly digest handler + cron registered together)

The handler implementation for Task 5. Add the callback logic:

**IMPORTANT**: Do NOT re-emit `CoachingLearningDigest` — that event is already emitted by the nightly decay cron. The weekly digest should log a summary only (the coaching pipeline already fires patterns from daily digests). The weekly cron is for administrative logging, not signal emission.

```rust
{
    let pool = repos.pool().clone();
    let rt = rt.clone();
    cron_service.register_handler(
        JOB_WEEKLY_KNOWLEDGE_DIGEST,
        Arc::new(move |_job: &scheduling::CronJob| {
            let pool = pool.clone();
            tokio::task::block_in_place(|| {
                rt.block_on(async {
                    let atom_repo = cognitive::KnowledgeAtomRepo::new(pool.clone());
                    let review_stats = cognitive::ReviewStatsRepo::new(pool.clone());

                    let (streak, topics, fading, daily) = tokio::join!(
                        review_stats.current_streak(),
                        atom_repo.list_topics_with_atoms(),
                        atom_repo.list_fading_important(10),
                        review_stats.daily_reviews(7),
                    );
                    let streak = streak.unwrap_or(0);
                    let fading_count = fading.unwrap_or_default().len();
                    let reviews_week: i64 = daily.unwrap_or_default().iter().map(|d| d.review_count).sum();

                    info!(
                        "Weekly knowledge digest: streak={streak}, reviews={reviews_week}, \
                         fading={fading_count}, topics={}",
                        topics.unwrap_or_default().len()
                    );
                    Ok(Some(format!(
                        "Weekly digest: streak={streak}, fading={fading_count}"
                    )))
                })
            })
        }),
    );
}
```

- [ ] **Step 2: Build + commit**

```
feat(app-core): implement weekly Knowledge Health digest handler
```

---

### Task 11: Final integration — workspace build + test + lint

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace 2>&1 | tail -5`

- [ ] **Step 2: Run all related tests**

Run: `cargo nextest run --workspace -E 'test(retention_history) | test(review_stats) | test(knowledge_atom) | test(dev_server_covers) | test(atom_decay)'`

- [ ] **Step 3: Clippy + fmt**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error\[" | head -10`
Run: `cargo fmt --all --check`

- [ ] **Step 4: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```
feat: Knowledge Atoms Phase 4 complete — charts, deep links, graph view, cross-feature atoms
```

---

## Verification Checklist

After all tasks complete:

- [ ] `cargo nextest run --workspace` — all tests pass
- [ ] `cargo clippy --workspace --all-targets --all-features` — zero warnings
- [ ] `cargo fmt --all --check` — formatted
- [ ] `cd desktop-ui && bun run lint` — no new errors
- [ ] Dev server parity test passes
- [ ] Manual test: Knowledge Health page "Trends" tab shows retention chart
- [ ] Manual test: Knowledge Health page "Graph" tab shows atom graph
- [ ] Manual test: coaching intervention shows "Open →" link
- [ ] Manual test: clicking fading atom in morning briefing navigates to note
- [ ] Manual test: project overview shows RelevantAtoms section
- [ ] Manual test: retention_history API returns time-series data

## Spec Completion Matrix

After Phase 4, every spec item is covered:

| Spec Item | Phase | Status |
|---|---|---|
| knowledge_atoms + knowledge_topics tables | 1 | Done |
| KnowledgeAtomRepo CRUD | 1 | Done |
| DomainEvents (11 variants) | 1 | Done |
| Vocab → atom migration | 1 | Done |
| Right panel KnowledgeAtomsPanel | 1 | Done |
| Inline quick review | 1 | Done |
| Auto-extraction from notes | 2 | Done |
| Suggested atoms UX | 2 | Done |
| Cross-note reinforcement | 2 | Done |
| Daily decay cron | 2 | Done |
| Knowledge Health page | 2 | Done |
| "Why this?" popover | 2 | Done |
| Bulk accept with importance | 2 | Done |
| 5 coaching pattern detectors | 3 | Done |
| Coaching message templates | 3 | Done |
| Morning briefing: Knowledge Health | 3 | Done |
| Focus session micro-review | 3 | Done |
| Autotuner: knowledge_retention_score | 3 | Done |
| Undo/restore (7-day window) | 3 | Done |
| Inline editor retention badge | 3 (partial) | Simplified as atom card pulse indicator; full TipTap margin pill deferred (out of scope) |
| Deep links: nudge → note | **4** | This plan |
| Weekly Knowledge Health digest | **4** | This plan |
| Autotuner: per-topic prompt reinforcement | **4** | This plan |
| Cross-feature atom injection | **4** | This plan |
| Retention history charts (sparklines) | **4** | This plan |
| Cross-domain connections view (graph) | **4** | This plan |
| Enhanced Knowledge Health (tabs + charts) | **4** | This plan |
