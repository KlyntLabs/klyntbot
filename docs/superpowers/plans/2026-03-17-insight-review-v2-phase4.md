# Insight Review V2 — Phase 4: Frontend + Deep Dive

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add frontend components for the evolution timeline, version history, and scope configuration — plus implement the deep dive cognitive context injection on the backend.

**Architecture:** Three new React components (`InsightEvolutionChart`, `InsightVersionList`, `InsightScopePopover`) consume existing + new Tauri commands via dedicated hooks. Backend additions: a `get_version` endpoint, scope config passthrough to the insight pipeline, and wiring the 3 stubbed deep-dive `CognitiveAccessor` methods to real cognitive repos. Recharts (already installed) renders the evolution chart using CSS variable theme tokens.

**Tech Stack:** React 19, TypeScript, Recharts 3, Tailwind v4 + CSS tokens, Tauri 2 IPC, Rust (sqlx, serde)

**Spec:** `docs/superpowers/specs/2026-03-17-insight-review-v2-design.md` (Sections 5.2, 7.5, 11 Phase 4)

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/hooks/useInsightEvolution.ts` | Hook — fetches evolution timeline data via `note_insight_get_evolution` |
| `desktop-ui/src/features/notes/hooks/useInsightVersions.ts` | Hook — fetches version list + loads specific version content |
| `desktop-ui/src/features/notes/components/insight/InsightEvolutionChart.tsx` | Recharts area chart showing progress signals over versions |
| `desktop-ui/src/features/notes/components/insight/InsightVersionList.tsx` | Selectable version list with progress badges + change notes |
| `desktop-ui/src/features/notes/components/insight/InsightScopePopover.tsx` | Scope type selector, radius slider, deep dive toggle |

### Modified files

| File | Change |
|------|--------|
| `crates/app-core/src/handlers/notes/insight.rs` | Add `note_insight_get_version()` handler; modify `note_insight_review()` to accept optional scope config |
| `crates/desktop-shared/src/commands/notes.rs` | Add `InsightScopeConfigParams` DTO |
| `crates/desktop/src/commands/notes.rs` | Add `note_insight_get_version` Tauri command + DEV_COMMANDS + dispatch_dev; update `note_insight_review` signature |
| `crates/desktop/src/main.rs` | Register `note_insight_get_version` |
| `crates/app-core/src/adapters/cognitive_accessor.rs` | Implement 3 deep-dive methods using real cognitive repos |
| `crates/app-core/src/init/mod.rs` | Pass `EntityRepo` + `SqlitePool` to `CognitiveAccessorImpl` |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | Add `openWithScope` action; expose version/evolution sub-state |
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Add History toggle, scope popover button, evolution chart section |

---

## Chunk 1: Backend — Version Detail + Scope Params

### Task 1: Add `note_insight_get_version` endpoint

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs`
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop/src/main.rs`

Returns the full parsed content of a specific insight version by ID (not just the latest). This powers the version history sidebar.

- [ ] **Step 1: Add handler in app-core**

In `crates/app-core/src/handlers/notes/insight.rs`, add to the `impl AppCore` block after `note_insight_get_evolution`:

```rust
    pub async fn note_insight_get_version(
        &self,
        insight_id: &str,
    ) -> Result<InsightReviewResponse, ApiError> {
        let service = self
            .insight_service
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Insight service not available"))?;

        let row = service
            .get_version(insight_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Insight version not found"))?;

        let content: feature_insights::InsightContent =
            serde_json::from_str(&row.content).unwrap_or_default();

        let self_assessment: Option<Vec<QuizQuestion>> = content
            .self_assessment
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        let persona_ids: Option<Vec<String>> = serde_json::from_str(&row.persona_ids).ok();

        Ok(InsightReviewResponse {
            insight_review_id: row.id,
            note_id: row.note_id,
            version: row.version,
            generated_at: row.generated_at,
            synthesis: content.synthesis,
            gap_analysis: content.gap_analysis,
            self_assessment,
            concept_map: content.concept_map,
            perspectives: content.perspectives,
            persona_ids,
        })
    }
```

- [ ] **Step 2: Add `get_version` to InsightService**

In `crates/feature-insights/src/service.rs`, add after `get_latest`:

```rust
    /// Get a specific insight by ID.
    pub async fn get_version(&self, insight_id: &str) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        self.repo.get(insight_id).await
    }
```

- [ ] **Step 3: Add Tauri command + DEV_COMMANDS + dispatch_dev**

In `crates/desktop/src/commands/notes.rs`, add the Tauri command after `note_insight_get_evolution`:

```rust
#[tauri::command]
pub async fn note_insight_get_version(
    state: State<'_, Arc<AppCore>>,
    insight_id: String,
) -> Result<InsightReviewResponse, ApiError> {
    state.note_insight_get_version(&insight_id).await
}
```

Add `"note_insight_get_version"` to `DEV_COMMANDS` after `"note_insight_get_evolution"`.

Add dispatch arm:
```rust
        "note_insight_get_version" => {
            let id = try_field!(dev::get_str(body, "insightId"));
            dev::val(core.note_insight_get_version(&id).await)
        }
```

In `crates/desktop/src/main.rs`, add `commands::notes::note_insight_get_version,` after `note_insight_get_evolution`.

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p desktop -E 'test(dev_server)'`

- [ ] **Step 5: Commit**

```bash
git add crates/feature-insights/src/service.rs crates/app-core/src/handlers/notes/insight.rs crates/desktop-shared/ crates/desktop/
git commit -m "feat(desktop): add note_insight_get_version Tauri command"
```

---

### Task 2: Accept scope config in `note_insight_review`

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs`
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

Currently `note_insight_review` uses `ScopeConfig::default()`. This adds an optional scope config parameter from the frontend.

- [ ] **Step 1: Add DTO in desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`, add after `InsightEvolutionPoint`:

```rust
// ── Insight Scope Config ─────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightScopeConfigParams {
    #[serde(default)]
    pub scope_type: Option<String>,
    pub radius: Option<f64>,
    #[serde(default)]
    pub node_ids: Option<Vec<String>>,
    pub include_cognitive: Option<bool>,
    pub deep_dive: Option<bool>,
    pub merge_threshold: Option<f64>,
}
```

- [ ] **Step 2: Update Tauri command signature**

In `crates/desktop/src/commands/notes.rs`, update the `note_insight_review` command:

```rust
#[tauri::command]
pub async fn note_insight_review(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
    scope_config: Option<desktop_shared::commands::InsightScopeConfigParams>,
) -> Result<InsightReviewStarted, ApiError> {
    state.note_insight_review(&note_id, scope_config.as_ref()).await
}
```

Update the dispatch_dev arm for `"note_insight_review"`:
```rust
        "note_insight_review" => {
            let id = try_field!(dev::get_str(body, "noteId"));
            let scope: Option<desktop_shared::commands::InsightScopeConfigParams> =
                body.get("scopeConfig").and_then(|v| serde_json::from_value(v.clone()).ok());
            dev::val(core.note_insight_review(&id, scope.as_ref()).await)
        }
```

- [ ] **Step 3: Update handler signature + build scope**

In `crates/app-core/src/handlers/notes/insight.rs`, update `note_insight_review`:

```rust
    pub async fn note_insight_review(
        &self,
        note_id: &str,
        scope_params: Option<&InsightScopeConfigParams>,
    ) -> Result<InsightReviewStarted, ApiError> {
```

Replace `let scope = feature_insights::ScopeConfig::default();` with:

```rust
        let scope = match scope_params {
            Some(params) => {
                let mut s = feature_insights::ScopeConfig::default();
                if let Some(ref st) = params.scope_type {
                    s.scope_type = match st.as_str() {
                        "semantic" => feature_insights::ScopeType::Semantic,
                        "project" => feature_insights::ScopeType::Project,
                        "manual" => feature_insights::ScopeType::Manual,
                        _ => feature_insights::ScopeType::Backlinks,
                    };
                }
                if let Some(r) = params.radius { s.radius = r; }
                if let Some(ref ids) = params.node_ids { s.node_ids = ids.clone(); }
                if let Some(c) = params.include_cognitive { s.include_cognitive = c; }
                if let Some(d) = params.deep_dive { s.deep_dive = d; }
                if let Some(m) = params.merge_threshold { s.merge_threshold = m; }
                s
            }
            None => feature_insights::ScopeConfig::default(),
        };
```

Also add the import at the top:
```rust
use desktop_shared::commands::InsightScopeConfigParams;
```

Note: The existing `use desktop_shared::commands::*;` glob import should already cover this. Check if it does — if not, add the specific import.

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p desktop -E 'test(dev_server)'`

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/ crates/desktop/ crates/app-core/src/handlers/notes/insight.rs
git commit -m "feat(app-core): accept scope config in note_insight_review"
```

---

## Chunk 2: Backend — Deep Dive Cognitive

### Task 3: Implement deep-dive CognitiveAccessor methods

**Files:**
- Modify: `crates/app-core/src/adapters/cognitive_accessor.rs`
- Modify: `crates/app-core/src/init/mod.rs`

The three deep-dive methods (`user_model_summary`, `entity_neighborhood`, `fact_history`) are currently stubs returning `None`/empty. The underlying cognitive repos are fully implemented:
- `cognitive::load_user_model(fact_repo)` → `UserModel`
- `cognitive::EntityRepo::get_neighborhood(entity_id, depth)` → `GraphNeighborhood`
- `cognitive::TemporalService::get_fact_history(subject, predicate)` → `Vec<FactVersion>`

- [ ] **Step 1: Add EntityRepo to CognitiveAccessorImpl**

In `crates/app-core/src/adapters/cognitive_accessor.rs`, update the struct to add `entity_repo`:

```rust
pub struct CognitiveAccessorImpl {
    fact_repo: cognitive::SemanticFactRepo,
    memory_repo: cognitive::EpisodicMemoryRepo,
    rule_repo: cognitive::ProceduralRuleRepo,
    entity_repo: cognitive::EntityRepo,
}

impl CognitiveAccessorImpl {
    pub fn new(
        fact_repo: cognitive::SemanticFactRepo,
        memory_repo: cognitive::EpisodicMemoryRepo,
        rule_repo: cognitive::ProceduralRuleRepo,
        entity_repo: cognitive::EntityRepo,
    ) -> Self {
        Self {
            fact_repo,
            memory_repo,
            rule_repo,
            entity_repo,
        }
    }
}
```

No `pool` needed — `TemporalService::new()` takes `SemanticFactRepo` (not a pool), and `fact_repo` is already stored.

- [ ] **Step 2: Implement the three deep-dive methods**

Replace the stubs. Key type details verified against source:
- `UserModel` (at `cognitive/src/types.rs:112`) is a flat struct with fields: `identity`, `energy`, `work`, `finance`, `learning`, `preferences`, `other` — each `Vec<SemanticFact>`.
- `GraphNeighborhood` (at `cognitive/src/repos/entity.rs:61`) has `center: EntityRow`, `neighbors: Vec<EntityRow>`, `relationships: Vec<RelationshipRow>`.
- `RelationshipRow` (at `entity.rs:37`) has `source_entity_id`, `target_entity_id`, `relationship_type` — no label fields. Build a lookup map from `neighbors` to resolve names.
- `FactVersion` (at `cognitive/src/services/temporal.rs:17`) has `fact: SemanticFact` + `is_archived: bool`. Access fields via `v.fact.predicate`, `v.fact.object`, `v.fact.valid_from`.
- `TemporalService::new(fact_repo: SemanticFactRepo)` — takes fact_repo, not pool.

```rust
    async fn user_model_summary(&self, _domain: &str) -> Option<String> {
        let model = cognitive::load_user_model(&self.fact_repo).await;
        let mut parts = Vec::new();

        let domains: &[(&str, &[cognitive::SemanticFact])] = &[
            ("identity", &model.identity),
            ("energy", &model.energy),
            ("work", &model.work),
            ("finance", &model.finance),
            ("learning", &model.learning),
            ("preferences", &model.preferences),
        ];

        for &(name, facts) in domains {
            if !facts.is_empty() {
                let summary: Vec<String> = facts.iter().take(3).map(|f| {
                    format!("{} {}", f.predicate, f.object)
                }).collect();
                parts.push(format!("**{}:** {}", name, summary.join("; ")));
            }
        }
        if parts.is_empty() { return None; }
        Some(parts.join("\n"))
    }

    async fn entity_neighborhood(&self, note_id: &str, depth: u8) -> Vec<String> {
        match self.entity_repo.get_neighborhood(note_id, depth as u32).await {
            Ok(Some(neighborhood)) => {
                // Build ID → name lookup from center + neighbors
                let mut names: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
                names.insert(&neighborhood.center.id, &neighborhood.center.name);
                for n in &neighborhood.neighbors {
                    names.insert(&n.id, &n.name);
                }

                neighborhood.relationships.iter().map(|rel| {
                    let source = names.get(rel.source_entity_id.as_str())
                        .copied().unwrap_or(&rel.source_entity_id);
                    let target = names.get(rel.target_entity_id.as_str())
                        .copied().unwrap_or(&rel.target_entity_id);
                    format!("{source} → {} → {target}", rel.relationship_type)
                }).collect()
            }
            _ => Vec::new(),
        }
    }

    async fn fact_history(&self, subject: &str) -> Vec<String> {
        let temporal_svc = cognitive::TemporalService::new(self.fact_repo.clone());
        match temporal_svc.get_fact_history(subject, "%").await {
            Ok(versions) => versions
                .iter()
                .take(10)
                .map(|v| {
                    format!(
                        "[{}] {} {} → {}",
                        &v.fact.valid_from,
                        subject,
                        v.fact.predicate,
                        v.fact.object,
                    )
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }
```

- [ ] **Step 3: Update init/mod.rs constructor**

In `crates/app-core/src/init/mod.rs`, update the `CognitiveAccessorImpl::new()` call to pass 4 args (no pool):

```rust
        let cognitive_accessor: Arc<dyn feature_insights::CognitiveAccessor> = Arc::new(
            crate::adapters::cognitive_accessor::CognitiveAccessorImpl::new(
                ::cognitive::SemanticFactRepo::new(storage_pool.inner().clone()),
                ::cognitive::EpisodicMemoryRepo::new(storage_pool.inner().clone()),
                ::cognitive::ProceduralRuleRepo::new(storage_pool.inner().clone()),
                ::cognitive::EntityRepo::new(storage_pool.inner().clone()),
            ),
        );
```

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p app-core`

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/adapters/cognitive_accessor.rs crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): implement deep-dive CognitiveAccessor methods"
```

---

## Chunk 3: Frontend — Evolution Timeline

### Task 4: Evolution timeline hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useInsightEvolution.ts`

- [ ] **Step 1: Create the hook**

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface EvolutionPoint {
  version: number;
  generatedAt: string;
  flashcardSuccess: number;
  semanticDrift: number;
  gapClosure: number;
  quizScore: number;
  overallProgress: number;
  changeNote: string;
}

interface EvolutionData {
  noteId: string;
  noteTitle: string;
  versions: EvolutionPoint[];
}

interface EvolutionState {
  loading: boolean;
  data: EvolutionData | null;
  error: string | null;
}

export function useInsightEvolution() {
  const [state, setState] = useState<EvolutionState>({
    loading: false,
    data: null,
    error: null,
  });

  const fetch = useCallback(async (noteId: string) => {
    setState({ loading: true, data: null, error: null });
    try {
      const data = await ipc<EvolutionData>("note_insight_get_evolution", { noteId });
      setState({ loading: false, data, error: null });
    } catch (e) {
      setState({ loading: false, data: null, error: String(e) });
    }
  }, []);

  const clear = useCallback(() => {
    setState({ loading: false, data: null, error: null });
  }, []);

  return { ...state, fetch, clear };
}
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useInsightEvolution.ts
git commit -m "feat(desktop-ui): add useInsightEvolution hook"
```

---

### Task 5: Evolution chart component

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/InsightEvolutionChart.tsx`

- [ ] **Step 1: Create the chart component**

Uses recharts `AreaChart` to show progress signals over time. Follows patterns from `ScoreTrendChart.tsx`.

```typescript
import { useMemo } from "react";
import {
  Area,
  AreaChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { EvolutionPoint } from "../../hooks/useInsightEvolution";

interface Props {
  versions: EvolutionPoint[];
}

export function InsightEvolutionChart({ versions }: Props) {
  const data = useMemo(
    () =>
      versions.map((v) => ({
        version: `v${v.version}`,
        overall: Math.round(v.overallProgress * 100),
        flashcard: Math.round(v.flashcardSuccess * 100),
        gaps: Math.round(v.gapClosure * 100),
        stability: Math.round((1 - v.semanticDrift) * 100),
        changeNote: v.changeNote,
      })),
    [versions],
  );

  if (data.length < 1) return null;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between px-1">
        <span className="text-[11px] font-medium text-muted-foreground">
          Learning Progress
        </span>
        <div className="flex items-center gap-3">
          {LEGEND.map((item) => (
            <span
              key={item.label}
              className="flex items-center gap-1 text-[9px] text-dim"
            >
              <span
                className="w-1.5 h-1.5 rounded-full"
                style={{ backgroundColor: item.color }}
              />
              {item.label}
            </span>
          ))}
        </div>
      </div>
      <ResponsiveContainer width="100%" height={140}>
        <AreaChart data={data} margin={{ top: 4, right: 8, bottom: 0, left: -20 }}>
          <XAxis
            dataKey="version"
            tick={{ fill: "var(--text-dim)", fontSize: 10 }}
            axisLine={false}
            tickLine={false}
          />
          <YAxis
            domain={[0, 100]}
            tick={{ fill: "var(--text-dim)", fontSize: 10 }}
            axisLine={false}
            tickLine={false}
            width={28}
            tickFormatter={(v) => `${v}%`}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: "var(--card)",
              border: "1px solid var(--border)",
              borderRadius: 8,
              fontSize: 11,
            }}
            formatter={(value: number, name: string) => [
              `${value}%`,
              LABEL_MAP[name] ?? name,
            ]}
            labelFormatter={(label) => label}
          />
          <Area
            type="monotone"
            dataKey="overall"
            stroke="var(--brand)"
            fill="var(--brand)"
            fillOpacity={0.15}
            strokeWidth={2}
            dot={{ r: 3, fill: "var(--brand)" }}
          />
          <Area
            type="monotone"
            dataKey="flashcard"
            stroke="var(--success)"
            fill="none"
            strokeWidth={1}
            strokeDasharray="4 2"
            dot={false}
          />
          <Area
            type="monotone"
            dataKey="gaps"
            stroke="var(--chart-2)"
            fill="none"
            strokeWidth={1}
            strokeDasharray="4 2"
            dot={false}
          />
          <Area
            type="monotone"
            dataKey="stability"
            stroke="var(--purple)"
            fill="none"
            strokeWidth={1}
            strokeDasharray="4 2"
            dot={false}
          />
        </AreaChart>
      </ResponsiveContainer>
      {/* Change notes under chart */}
      {versions.length > 0 && (
        <p className="text-[10px] text-dim italic px-1">
          Latest: {versions[versions.length - 1]?.changeNote}
        </p>
      )}
    </div>
  );
}

const LEGEND = [
  { label: "Overall", color: "var(--brand)" },
  { label: "Flashcards", color: "var(--success)" },
  { label: "Gap Closure", color: "var(--chart-2)" },
  { label: "Stability", color: "var(--purple)" },
];

const LABEL_MAP: Record<string, string> = {
  overall: "Overall Progress",
  flashcard: "Flashcard Success",
  gaps: "Gap Closure",
  stability: "Content Stability",
};
```

- [ ] **Step 2: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/InsightEvolutionChart.tsx
git commit -m "feat(desktop-ui): add InsightEvolutionChart recharts component"
```

---

## Chunk 4: Frontend — Version History

### Task 6: Version history hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useInsightVersions.ts`

- [ ] **Step 1: Create the hook**

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface InsightVersion {
  id: string;
  version: number;
  generatedAt: string;
  inputHash: string;
  hasParent: boolean;
}

interface VersionsState {
  loading: boolean;
  versions: InsightVersion[];
  selectedId: string | null;
}

export function useInsightVersions() {
  const [state, setState] = useState<VersionsState>({
    loading: false,
    versions: [],
    selectedId: null,
  });

  const fetch = useCallback(async (noteId: string) => {
    setState((prev) => ({ ...prev, loading: true }));
    try {
      const versions = await ipc<InsightVersion[]>(
        "note_insight_list_versions",
        { noteId },
      );
      setState({ loading: false, versions, selectedId: null });
    } catch {
      setState({ loading: false, versions: [], selectedId: null });
    }
  }, []);

  const select = useCallback((id: string | null) => {
    setState((prev) => ({ ...prev, selectedId: id }));
  }, []);

  return { ...state, fetch, select };
}
```

- [ ] **Step 2: Lint + commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/features/notes/hooks/useInsightVersions.ts
git commit -m "feat(desktop-ui): add useInsightVersions hook"
```

---

### Task 7: Version list component

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/InsightVersionList.tsx`

- [ ] **Step 1: Create the component**

```typescript
import { Clock, GitBranch } from "lucide-react";
import type { InsightVersion } from "../../hooks/useInsightVersions";

interface Props {
  versions: InsightVersion[];
  selectedId: string | null;
  currentId: string | null;
  onSelect: (id: string | null) => void;
}

export function InsightVersionList({
  versions,
  selectedId,
  currentId,
  onSelect,
}: Props) {
  if (versions.length === 0) {
    return (
      <p className="text-[11px] text-dim italic px-3 py-4">
        No version history yet.
      </p>
    );
  }

  return (
    <div className="flex flex-col">
      {versions.map((v) => {
        const isActive = selectedId
          ? selectedId === v.id
          : currentId === v.id;
        const date = new Date(v.generatedAt);
        const dateStr = date.toLocaleDateString(undefined, {
          month: "short",
          day: "numeric",
        });
        const timeStr = date.toLocaleTimeString(undefined, {
          hour: "2-digit",
          minute: "2-digit",
        });

        return (
          <button
            key={v.id}
            type="button"
            onClick={() => onSelect(isActive && selectedId ? null : v.id)}
            className={`flex items-start gap-2 px-3 py-2 text-left transition-colors border-l-2 ${
              isActive
                ? "border-purple bg-white/[0.04]"
                : "border-transparent hover:bg-white/[0.02]"
            }`}
          >
            <div className="flex flex-col gap-0.5 min-w-0">
              <div className="flex items-center gap-1.5">
                <span className="text-[11px] font-medium text-foreground">
                  v{v.version}
                </span>
                {v.hasParent && (
                  <GitBranch
                    size={10}
                    className="text-muted-foreground"
                    title="Merged from related insight"
                  />
                )}
              </div>
              <div className="flex items-center gap-1 text-[10px] text-dim">
                <Clock size={9} />
                <span>
                  {dateStr} {timeStr}
                </span>
              </div>
            </div>
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Lint + commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/features/notes/components/insight/InsightVersionList.tsx
git commit -m "feat(desktop-ui): add InsightVersionList component"
```

---

## Chunk 5: Frontend — Scope Config Popover

### Task 8: Scope config popover component

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/InsightScopePopover.tsx`

Follows the pattern from `GraphSettingsPopover.tsx` — a popover with sliders and toggles.

- [ ] **Step 1: Create the component**

```typescript
import { useState } from "react";

export interface ScopeConfig {
  scopeType: "backlinks" | "semantic" | "project" | "manual";
  radius: number;
  includeCognitive: boolean;
  deepDive: boolean;
}

const DEFAULT_SCOPE: ScopeConfig = {
  scopeType: "backlinks",
  radius: 0.72,
  includeCognitive: true,
  deepDive: false,
};

interface Props {
  value: ScopeConfig;
  onChange: (config: ScopeConfig) => void;
  onClose: () => void;
}

const SCOPE_TYPES = [
  { id: "backlinks" as const, label: "Backlinks", desc: "Wikilink references" },
  { id: "semantic" as const, label: "Semantic", desc: "Similar by embedding" },
  { id: "project" as const, label: "Project", desc: "Same notebook" },
  { id: "manual" as const, label: "Manual", desc: "Selected notes" },
];

export function InsightScopePopover({ value, onChange, onClose }: Props) {
  return (
    <div className="absolute right-0 top-full mt-1 z-50 w-64 glass-panel rounded-xl p-3 flex flex-col gap-3 shadow-xl">
      {/* Scope type */}
      <div className="flex flex-col gap-1.5">
        <label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
          Scope
        </label>
        <div className="grid grid-cols-2 gap-1">
          {SCOPE_TYPES.map((st) => (
            <button
              key={st.id}
              type="button"
              onClick={() => onChange({ ...value, scopeType: st.id })}
              className={`px-2 py-1.5 rounded-md text-[10px] text-left transition-colors ${
                value.scopeType === st.id
                  ? "bg-purple/20 text-purple-300 border border-purple/30"
                  : "bg-white/[0.04] text-muted-foreground hover:bg-white/[0.06] border border-transparent"
              }`}
            >
              <div className="font-medium">{st.label}</div>
              <div className="text-[9px] text-dim">{st.desc}</div>
            </button>
          ))}
        </div>
      </div>

      {/* Radius slider (only for semantic) */}
      {value.scopeType === "semantic" && (
        <div className="flex flex-col gap-1">
          <div className="flex items-center justify-between">
            <label className="text-[10px] font-medium text-muted-foreground">
              Similarity Radius
            </label>
            <span className="text-[10px] text-dim">
              {value.radius.toFixed(2)}
            </span>
          </div>
          <input
            type="range"
            min={0.5}
            max={0.95}
            step={0.01}
            value={value.radius}
            onChange={(e) =>
              onChange({ ...value, radius: Number.parseFloat(e.target.value) })
            }
            className="w-full accent-purple h-1"
          />
        </div>
      )}

      {/* Toggles */}
      <div className="flex flex-col gap-2 pt-1 border-t border-border">
        <Toggle
          label="Cognitive Context"
          description="Include facts, memories, rules"
          checked={value.includeCognitive}
          onChange={(c) => onChange({ ...value, includeCognitive: c })}
        />
        <Toggle
          label="Deep Dive"
          description="User model + entity graph + history"
          checked={value.deepDive}
          onChange={(d) => onChange({ ...value, deepDive: d })}
        />
      </div>

      {/* Close */}
      <button
        type="button"
        onClick={onClose}
        className="self-end text-[10px] text-dim hover:text-muted-foreground transition-colors"
      >
        Done
      </button>
    </div>
  );
}

export { DEFAULT_SCOPE };

function Toggle({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      className="flex items-center gap-2 text-left"
      role="switch"
      aria-checked={checked}
    >
      <div
        className={`w-7 h-4 rounded-full transition-colors flex items-center px-0.5 ${
          checked ? "bg-purple" : "bg-accent"
        }`}
      >
        <div
          className={`w-3 h-3 rounded-full bg-white transition-transform ${
            checked ? "translate-x-3" : "translate-x-0"
          }`}
        />
      </div>
      <div className="flex flex-col">
        <span className="text-[10px] font-medium text-foreground">{label}</span>
        <span className="text-[9px] text-dim">{description}</span>
      </div>
    </button>
  );
}
```

- [ ] **Step 2: Lint + commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/features/notes/components/insight/InsightScopePopover.tsx
git commit -m "feat(desktop-ui): add InsightScopePopover component"
```

---

## Chunk 6: Frontend — Integration + Best Node Tooltip

### Task 9: Best node tooltip (scope coverage hint)

**Files:**
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`

The spec (Section 4.3) requires a tooltip showing scope coverage before generating: "Triggering insight here will see 12 related notes (including 'Deep Work' and 'Focus Sessions')." This is a lightweight UX hint — not a full feature.

Implementation: When the insight panel opens and content is loading, show a brief line below the header indicating the scope coverage. The data comes from the evolution response or can be derived from the version list count. For a first pass, show a simple hint based on the scope type:

- [ ] **Step 1: Add scope hint to the panel header area**

In `InsightReviewPanel.tsx`, add below the header div (between header and tab bar):

```tsx
{/* Scope coverage hint */}
{state.isOpen && (
  <div className="px-3 py-1.5 border-b border-border text-[10px] text-dim flex items-center gap-1">
    <span>Scope:</span>
    <span className="text-muted-foreground capitalize">{scopeConfig.scopeType}</span>
    {scopeConfig.deepDive && (
      <span className="text-purple text-[9px] ml-1">(deep dive)</span>
    )}
    {evolution.data && (
      <span className="ml-auto">
        {evolution.data.versions.length} version{evolution.data.versions.length !== 1 ? "s" : ""}
      </span>
    )}
  </div>
)}
```

This provides a lightweight scope indicator. A full "Best node" tooltip with semantic similarity search (top-5 notes, score > 0.75) would require a new backend endpoint (`note_insight_scope_preview`). That endpoint can be added as a follow-up — the UI slot is ready.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/notes/components/InsightReviewPanel.tsx
git commit -m "feat(desktop-ui): add scope coverage hint in InsightReviewPanel header"
```

---

### Task 10: Wire evolution, versions, and scope into InsightReviewPanel

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useInsightReview.ts`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`

This is the integration task — connects all new components to the existing panel. The approach:

1. Add a "History" toggle button in the panel header (next to Settings/Personas icon)
2. When toggled, show a collapsible section above the tabs with:
   - Evolution chart (if >1 version)
   - Version list
3. Add a scope config popover triggered by a new button in the header
4. Pass scope config to `open()` action

- [ ] **Step 1: Update useInsightReview — extract `applyCachedContent` helper + add `openWithScope`**

In `desktop-ui/src/features/notes/hooks/useInsightReview.ts`:

First, extract the cache-loading logic from the `open` function into a reusable helper. Place this inside the `useInsightReview` function body, before the `open` callback:

```typescript
  // Shared helper: apply cached insight content to tab state
  const applyCachedContent = useCallback(
    (cached: InsightReviewCachedResponse) => {
      setState((prev) => {
        const tabs = { ...prev.tabs };

        tabs.synthesis = { status: "done", content: cached.synthesis ?? "" };
        tabs.gaps = { status: "done", content: cached.gapAnalysis ?? "" };
        tabs.assessment = {
          status: "done",
          questions: cached.selfAssessment ?? [],
        };

        const cm = cached.conceptMap ?? "";
        if (cm.startsWith("FALLBACK:")) {
          tabs.conceptMap = { status: "done", mermaid: "", fallbackText: cm.slice("FALLBACK:".length) };
        } else {
          tabs.conceptMap = { status: "done", mermaid: cm, fallbackText: "" };
        }

        tabs.perspectives = {
          status: cached.perspectives ? "done" : "idle",
          content: cached.perspectives ?? "",
          personas: [],
        };

        return { ...prev, tabs };
      });
    },
    [],
  );
```

Then refactor `open` to call `applyCachedContent(cached)` instead of duplicating the tab-setting logic. Add the `InsightReviewActions` interface update:

```typescript
export interface InsightReviewActions {
  open: (noteId: string) => Promise<void>;
  openWithScope: (noteId: string, scopeConfig: Record<string, unknown>) => Promise<void>;
  applyCachedContent: (cached: InsightReviewCachedResponse) => void;
  close: () => void;
  switchTab: (tab: TabId) => void;
  regenerateTab: (tab: TabId) => Promise<void>;
  saveFlashcards: (deckName: string) => Promise<void>;
  answerQuestion: (questionId: string, answer: string) => void;
  revealAnswer: (questionId: string) => void;
  revealAll: () => void;
}
```

**Also export `InsightReviewCachedResponse`** from the hook so the panel can use it for version loading:

```typescript
export interface InsightReviewCachedResponse {
  insightReviewId: string;
  noteId: string;
  synthesis: string | null;
  gapAnalysis: string | null;
  selfAssessment: QuizQuestion[] | null;
  conceptMap: string | null;
  perspectives: string | null;
  personaIds: string[] | null;
}
```

Add `openWithScope` (same as `open` but passes `scopeConfig` to IPC):

```typescript
  const openWithScope = useCallback(
    async (noteId: string, scopeConfig: Record<string, unknown>) => {
      setState({
        ...INITIAL_STATE,
        isOpen: true,
        noteId,
        tabs: {
          synthesis: { status: "loading", content: "" },
          gaps: { status: "loading", content: "" },
          assessment: { status: "loading", questions: [] },
          conceptMap: { status: "loading", mermaid: "", fallbackText: "" },
          perspectives: { status: "loading", content: "", personas: [] },
        },
      });

      const response = await ipc<InsightReviewStartedResponse>("note_insight_review", {
        noteId,
        scopeConfig,
      });

      setState((prev) => ({
        ...prev,
        insightReviewId: response.insightReviewId,
        contentHash: response.contentHash,
      }));

      if (response.cached) {
        const cached = await ipc<InsightReviewCachedResponse>("note_insight_cache_get", { noteId });
        applyCachedContent(cached);
      } else {
        setState((prev) => ({
          ...prev,
          tabs: {
            ...prev.tabs,
            synthesis: { status: "streaming", content: "" },
          },
        }));
      }
    },
    [applyCachedContent],
  );
```

Add `openWithScope` and `applyCachedContent` to the actions object.

- [ ] **Step 2: Update InsightReviewPanel — add History toggle + Scope button**

In `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`:

Add imports at top:
```typescript
import { History, Sliders } from "lucide-react";
import { ipc } from "@shared/hooks/useIpc";
import type { InsightReviewCachedResponse } from "../hooks/useInsightReview";
import { useInsightEvolution } from "../hooks/useInsightEvolution";
import { useInsightVersions } from "../hooks/useInsightVersions";
import { InsightEvolutionChart } from "./insight/InsightEvolutionChart";
import { InsightVersionList } from "./insight/InsightVersionList";
import { InsightScopePopover, DEFAULT_SCOPE, type ScopeConfig } from "./insight/InsightScopePopover";
```

Add state in the component body:
```typescript
const [showHistory, setShowHistory] = useState(false);
const [showScope, setShowScope] = useState(false);
const [scopeConfig, setScopeConfig] = useState<ScopeConfig>(DEFAULT_SCOPE);
const evolution = useInsightEvolution();
const versions = useInsightVersions();
```

Fetch evolution + versions when panel opens (add `useEffect`):
```typescript
import { useEffect } from "react";

useEffect(() => {
  if (state.isOpen && state.noteId) {
    evolution.fetch(state.noteId);
    versions.fetch(state.noteId);
  } else {
    evolution.clear();
  }
}, [state.isOpen, state.noteId]);
```

In the header, add History and Scope buttons (before the Settings2/persona button):
```tsx
<div className="relative">
  <button
    type="button"
    onClick={() => setShowScope((p) => !p)}
    className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
    title="Scope Config"
  >
    <Sliders size={12} />
  </button>
  {showScope && (
    <InsightScopePopover
      value={scopeConfig}
      onChange={setScopeConfig}
      onClose={() => setShowScope(false)}
    />
  )}
</div>
<button
  type="button"
  onClick={() => setShowHistory((p) => !p)}
  className={`p-1 rounded-md transition-colors ${
    showHistory
      ? "text-purple bg-purple/10"
      : "text-muted-foreground hover:text-foreground hover:bg-accent"
  }`}
  title="Version History"
>
  <History size={12} />
</button>
```

Add collapsible history section between tab bar and content area:
```tsx
{/* History panel (collapsible) */}
{showHistory && (
  <div className="border-b border-border shrink-0 max-h-[300px] overflow-y-auto">
    {/* Evolution chart */}
    {evolution.data && evolution.data.versions.length > 0 && (
      <div className="p-3 border-b border-border">
        <InsightEvolutionChart versions={evolution.data.versions} />
      </div>
    )}
    {/* Version list */}
    <InsightVersionList
      versions={versions.versions}
      selectedId={versions.selectedId}
      currentId={state.insightReviewId}
      onSelect={async (id) => {
        versions.select(id);
        if (id) {
          try {
            const versionData = await ipc<InsightReviewCachedResponse>(
              "note_insight_get_version",
              { insightId: id },
            );
            actions.applyCachedContent(versionData);
          } catch {
            // Silently fail — version may have been deleted
          }
        }
      }}
    />
  </div>
)}
```

**Note:** The version content loading when selecting a past version will reuse the same state-setting pattern as the cache-hit path in `useInsightReview.open()`. This may require extracting a shared `loadInsightContent(cached, setState)` helper.

- [ ] **Step 3: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Test**

Run: `cd desktop-ui && bun run test`
Run: `cd desktop-ui && bun run build` (verify no type errors)

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(desktop-ui): wire evolution timeline, version history, and scope config into InsightReviewPanel"
```

---

## Chunk 7: Verification

### Task 11: Full verification

- [ ] **Step 1: Backend tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings.

- [ ] **Step 3: Format (both Rust + frontend)**

Run: `cargo fmt --all`
Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Frontend build**

Run: `cd desktop-ui && bun run build`
Expected: no TypeScript errors.

- [ ] **Step 5: Manual smoke test**

Start: `cargo tauri dev`

1. Open a note → click Insight Review → verify tabs work as before
2. Click the Scope icon (Sliders) → verify popover shows with scope type buttons, radius slider (only for semantic), and toggles
3. Click the History icon → verify evolution chart appears (if >1 version)
4. Verify version list shows versions with dates
5. Click a past version → verify content loads
6. Toggle Deep Dive ON → regenerate → verify longer/richer synthesis (more cognitive context)
7. Test the evolution endpoint:

```bash
NOTE_ID="<your-note-id>"
curl -s http://localhost:3456/api/note_insight_get_evolution \
  -X POST -H "Content-Type: application/json" \
  -d "{\"noteId\": \"$NOTE_ID\"}" | python3 -m json.tool
```

- [ ] **Step 6: Commit if needed**

```bash
cargo fmt --all && cd desktop-ui && bun run lint:fix
git add -A && git commit -m "style: format Insight Review V2 Phase 4"
```
