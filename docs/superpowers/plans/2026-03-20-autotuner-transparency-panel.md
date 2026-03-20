# Autotuner Transparency Panel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the autotuner's self-improvement visible to the user through an enhanced ambient indicator, micro-confirmation toasts on promotion, a brain health badge, and an experiment pace control — turning invisible backend optimization into a felt second-brain experience.

**Architecture:** All data already flows via `useAutoTunerStatus` and `useAutoTunerHistory` hooks. This is purely frontend work — no new backend endpoints. We enhance the existing `AmbientIndicator` in the chat header, add a toast variant for promotions via `useEvent`, add a `BrainHealthBadge` to the settings card, and wire the experiment pace control through `config_update_section`.

**Tech Stack:** React 19, TypeScript, Tailwind v4 with CSS tokens, glass-panel system, `useQuery`/`useMutation`/`useEvent` hooks, Biome 2.0 (line width 100)

**Spec:** `docs/superpowers/specs/2026-03-19-autoresearch-design.md` (Transparency Panel UI section, lines 412-524)

**Depends on:** Phase 1+2 autotuner feedback loop commits (unstaged)

---

## File Map

| File | Responsibility | Tasks |
|------|---------------|-------|
| `desktop-ui/src/features/autotuner/components/AmbientIndicator.tsx` | Modify — enhanced indicator with brain health dot + status text | 1 |
| `desktop-ui/src/features/autotuner/components/BrainHealthBadge.tsx` | Create — compact status badge showing growth state | 1 |
| `desktop-ui/src/features/autotuner/components/PromotionToast.tsx` | Create — micro-confirmation card on promotion | 2 |
| `desktop-ui/src/features/autotuner/hooks/usePromotionListener.ts` | Create — listens for promotion events via `useEvent` | 2 |
| `desktop-ui/src/features/autotuner/components/ExperimentPaceControl.tsx` | Create — conservative/balanced/bold toggle | 3 |
| `desktop-ui/src/features/autotuner/components/AutoTunerPanel.tsx` | Modify — add BrainHealthBadge + ExperimentPaceControl | 3 |
| `desktop-ui/src/features/autotuner/components/ChampionCard.tsx` | Modify — add brain_growth status display | 1 |
| `desktop-ui/src/features/autotuner/index.ts` | Modify — export new components | 1,2,3 |
| `desktop-ui/src/features/chat/pages/ChatPage.tsx` | Modify — wire PromotionToast | 2 |
| `crates/desktop/src/commands/autotuner.rs` | Modify — add `autotuner_set_pace` command | 3 |
| `crates/app-core/src/handlers/autotuner.rs` | Modify — add pace getter/setter | 3 |

---

### Task 1: Enhanced AmbientIndicator + BrainHealthBadge

**Files:**
- Modify: `desktop-ui/src/features/autotuner/components/AmbientIndicator.tsx`
- Create: `desktop-ui/src/features/autotuner/components/BrainHealthBadge.tsx`
- Modify: `desktop-ui/src/features/autotuner/components/ChampionCard.tsx`
- Modify: `desktop-ui/src/features/autotuner/index.ts`

The existing `AmbientIndicator` shows static text when a promotion happened. Enhance it with a pulsing dot that reflects `brain_growth.status` and contextual text.

- [ ] **Step 1: Create `BrainHealthBadge.tsx`**

```tsx
import { useAutoTunerStatus } from "@features/autotuner/hooks/useAutoTunerStatus"

const STATUS_CONFIG = {
  needs_feedback: {
    color: "bg-muted",
    pulse: false,
    label: "Waiting for feedback",
  },
  adapting: {
    color: "bg-warning",
    pulse: true,
    label: "Learning from your corrections",
  },
  growing: {
    color: "bg-success",
    pulse: true,
    label: "Actively improving",
  },
} as const

type GrowthStatus = keyof typeof STATUS_CONFIG

export function BrainHealthBadge({ compact = false }: { compact?: boolean }) {
  const { data: status } = useAutoTunerStatus()
  const growth = status?.brainGrowth
  if (!growth || !status?.enabled) return null

  const config = STATUS_CONFIG[growth.status as GrowthStatus]
    ?? STATUS_CONFIG.needs_feedback

  return (
    <div className="flex items-center gap-1.5">
      <span className="relative flex h-2 w-2">
        {config.pulse && (
          <span
            className={`absolute inline-flex h-full w-full rounded-full opacity-75 animate-ping ${config.color}`}
          />
        )}
        <span className={`relative inline-flex rounded-full h-2 w-2 ${config.color}`} />
      </span>
      {!compact && (
        <span className="text-[11px] text-muted font-light">
          {config.label}
        </span>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Enhance `AmbientIndicator.tsx`**

Read the existing file first. Currently it shows static text. Enhance to:
- Show `BrainHealthBadge` with compact dot
- Show contextual text based on `brain_growth.status`:
  - `"needs_feedback"` → `"Help me learn — correct me when I'm wrong"`
  - `"adapting"` → `"Learning from {n} corrections this week"`
  - `"growing"` → `"Getting to know you better — {impact}"`
- Only show when autotuner is enabled
- Clicking navigates to `/settings/general` (existing behavior)

Replace the static `"Getting to know you better"` text with dynamic content from `brain_growth`.

- [ ] **Step 3: Add brain_growth display to `ChampionCard.tsx`**

Read the existing file. Add a section below the champion description showing:
- `BrainHealthBadge` (full, not compact)
- Metrics summary: `"{n} corrections captured · {n} trials evaluated · {n} promoted"` from `brain_growth` fields
- `MetricsHealth` indicators: 3 small dots (green/gray) for `correctionRateAvailable`, `tokenRateAvailable`, `stabilityAvailable`

```tsx
{status.brainGrowth && (
  <div className="mt-3 pt-3 border-t border-border-subtle">
    <BrainHealthBadge />
    <div className="mt-2 flex items-center gap-3 text-[11px] text-muted">
      <span>{status.brainGrowth.correctionsCaptured7d} corrections</span>
      <span>{status.brainGrowth.trialsEvaluated7d} evaluated</span>
      <span>{status.brainGrowth.promotedThisWeek} promoted</span>
    </div>
    {status.metricsHealth && (
      <div className="mt-1.5 flex items-center gap-2 text-[10px] text-muted/60">
        <MetricDot active={status.metricsHealth.correctionRateAvailable} label="Corrections" />
        <MetricDot active={status.metricsHealth.tokenRateAvailable} label="Tokens" />
        <MetricDot active={status.metricsHealth.stabilityAvailable} label="Stability" />
      </div>
    )}
  </div>
)}
```

Where `MetricDot` is a small inline component:
```tsx
function MetricDot({ active, label }: { active: boolean; label: string }) {
  return (
    <span className="flex items-center gap-1">
      <span className={`w-1 h-1 rounded-full ${active ? "bg-success" : "bg-muted/40"}`} />
      {label}
    </span>
  )
}
```

- [ ] **Step 4: Update barrel export**

Add `BrainHealthBadge` to `desktop-ui/src/features/autotuner/index.ts`.

- [ ] **Step 5: Verify**

Run: `cd desktop-ui && bun run lint:fix && bun run build`
Expected: Clean build.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/autotuner/
git commit -m "feat(desktop): add BrainHealthBadge and enhance AmbientIndicator with brain_growth status"
```

---

### Task 2: Promotion micro-confirmation toast

**Files:**
- Create: `desktop-ui/src/features/autotuner/hooks/usePromotionListener.ts`
- Create: `desktop-ui/src/features/autotuner/components/PromotionToast.tsx`
- Modify: `desktop-ui/src/features/chat/pages/ChatPage.tsx`
- Modify: `desktop-ui/src/features/autotuner/index.ts`

The spec says: show a non-blocking toast for the first 3 promotions: "I just improved how I understand you. Want to see what changed?" with a "Show me" action.

The backend emits `AutotunerDecision` events on promotion. We need to:
1. Emit a Tauri event from the backend when a promotion happens (check if this already exists)
2. Listen for it in the frontend
3. Show a toast with a "Show me" link to settings

- [ ] **Step 1: Check backend event emission**

The nightly cycle already emits `DomainEvent::AutotunerDecision` on the bus. Check if the desktop app forwards this as a Tauri event. If not, we'll use polling — `useAutoTunerStatus` already polls, and we can detect promotions by comparing `champion.trial_id` changes.

The simpler approach: use `useAutoTunerStatus` polling + `useRef` to track the previous `champion.trial_id`. When it changes to a new non-null value, show the toast.

- [ ] **Step 2: Create `usePromotionListener.ts`**

```tsx
import { useEffect, useRef } from "react"
import { useAutoTunerStatus } from "@features/autotuner/hooks/useAutoTunerStatus"

export function usePromotionListener(
  onPromotion: (impact: string) => void
) {
  const { data: status } = useAutoTunerStatus()
  const prevTrialId = useRef<string | null>(null)
  const promotionCount = useRef(0)

  useEffect(() => {
    if (!status?.enabled || !status.champion) return

    const currentTrialId = status.champion.trial_id
    if (
      currentTrialId &&
      prevTrialId.current !== null &&
      currentTrialId !== prevTrialId.current &&
      promotionCount.current < 3
    ) {
      promotionCount.current += 1
      onPromotion(status.champion.impact || "response quality improved")
    }
    prevTrialId.current = currentTrialId ?? null
  }, [status?.champion?.trial_id, onPromotion])
}
```

Note: `promotionCount` resets on page refresh (not persisted). For a more robust counter, read from `LearningStateRepo["autotuner_promotion_toast_count"]` via a new API — but that's over-engineering for now. A `useRef` is sufficient.

- [ ] **Step 3: Create `PromotionToast.tsx`**

A glass-card style inline toast (not using the global `ToastContainer` — this is richer, like `AutoFocusToast`):

```tsx
import { useState } from "react"
import { useNavigate } from "react-router-dom"

interface PromotionToastProps {
  impact: string
  onDismiss: () => void
}

export function PromotionToast({ impact, onDismiss }: PromotionToastProps) {
  const navigate = useNavigate()

  return (
    <div className="glass-card border-l-2 p-3 animate-[slideIn_0.2s_ease-out]"
      style={{ borderLeftColor: "var(--success)" }}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium text-foreground">
            I just improved how I understand you
          </p>
          <p className="text-xs text-muted mt-0.5">{impact}</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <button
            type="button"
            onClick={() => {
              navigate("/settings/general")
              onDismiss()
            }}
            className="text-xs text-brand hover:text-brand/80 font-medium"
          >
            Show me
          </button>
          <button
            type="button"
            onClick={onDismiss}
            className="text-xs text-muted hover:text-foreground"
          >
            Dismiss
          </button>
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 4: Wire into ChatPage.tsx**

Read `ChatPage.tsx`. Find where `CoachingNudge` is rendered (between message area and input). Add `PromotionToast` in a similar position:

```tsx
import { usePromotionListener, PromotionToast } from "@features/autotuner"

// Inside ChatPage component:
const [promotionImpact, setPromotionImpact] = useState<string | null>(null)

usePromotionListener((impact) => {
  setPromotionImpact(impact)
  // Auto-dismiss after 15 seconds
  setTimeout(() => setPromotionImpact(null), 15000)
})

// In JSX, near CoachingNudge:
{promotionImpact && (
  <div className="px-4 pb-2">
    <PromotionToast
      impact={promotionImpact}
      onDismiss={() => setPromotionImpact(null)}
    />
  </div>
)}
```

- [ ] **Step 5: Update barrel export**

Add `PromotionToast`, `usePromotionListener` to `index.ts`.

- [ ] **Step 6: Verify**

Run: `cd desktop-ui && bun run lint:fix && bun run build`
Expected: Clean build.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/autotuner/ desktop-ui/src/features/chat/pages/ChatPage.tsx
git commit -m "feat(desktop): add promotion micro-confirmation toast in chat"
```

---

### Task 3: Experiment Pace Control

**Files:**
- Create: `desktop-ui/src/features/autotuner/components/ExperimentPaceControl.tsx`
- Modify: `desktop-ui/src/features/autotuner/components/AutoTunerPanel.tsx`
- Modify: `crates/app-core/src/handlers/autotuner.rs`
- Modify: `crates/desktop/src/commands/autotuner.rs`
- Modify: `desktop-ui/src/features/autotuner/types.ts`

The experiment pace (conservative/balanced/bold) is already stored in `LearningStateRepo["autotuner_experiment_pace"]`. We need:
1. A backend command to get/set the pace
2. A frontend toggle control

- [ ] **Step 1: Add pace to `AutoTunerStatus` response**

In `crates/app-core/src/handlers/autotuner.rs`, add `experiment_pace: Option<String>` to `AutoTunerStatus`. Populate it by reading `learning_state["autotuner_experiment_pace"]` (the orchestrator already has this key).

In `desktop-ui/src/features/autotuner/types.ts`, add:
```ts
export interface AutoTunerStatus {
  // ... existing fields
  experimentPace: string | null
}
```

- [ ] **Step 2: Add `autotuner_set_pace` backend command**

In `crates/app-core/src/handlers/autotuner.rs`, add:
```rust
pub async fn autotuner_set_pace(&self, pace: &str) -> Result<(), ApiError> {
    let orch = self.autotuner_orchestrator()
        .ok_or(ApiError::not_found("autotuner not enabled"))?;
    // Validate pace
    match pace {
        "conservative" | "balanced" | "bold" => {}
        _ => return Err(ApiError::validation("pace must be conservative, balanced, or bold")),
    }
    orch.learning_state_repo()
        .set("autotuner_experiment_pace", &serde_json::Value::String(pace.into()))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}
```

In `crates/desktop/src/commands/autotuner.rs`, add the Tauri command:
```rust
#[tauri::command]
pub async fn autotuner_set_pace(
    state: State<'_, Arc<AppCore>>,
    pace: String,
) -> Result<(), ApiError> {
    state.autotuner_set_pace(&pace).await
}
```

Register the command in the Tauri builder and add to `DEV_COMMANDS`.

- [ ] **Step 3: Create `ExperimentPaceControl.tsx`**

A 3-option segmented control:

```tsx
import { useMutation } from "@shared/hooks/useMutation"
import { useAutoTunerStatus } from "@features/autotuner/hooks/useAutoTunerStatus"

const PACES = [
  { value: "conservative", label: "Conservative", desc: "Small, safe tweaks" },
  { value: "balanced", label: "Balanced", desc: "Mix of safe and bold" },
  { value: "bold", label: "Bold", desc: "Aggressive exploration" },
] as const

export function ExperimentPaceControl() {
  const { data: status, refetch } = useAutoTunerStatus()
  const { mutate } = useMutation<void, { pace: string }>("autotuner_set_pace")
  const currentPace = status?.experimentPace ?? "balanced"

  const handleChange = async (pace: string) => {
    await mutate({ pace })
    refetch()
  }

  return (
    <div className="space-y-2">
      <label className="text-xs font-medium text-muted">Experiment Pace</label>
      <div className="flex rounded-lg overflow-hidden border border-border-subtle">
        {PACES.map(({ value, label }) => (
          <button
            key={value}
            type="button"
            onClick={() => handleChange(value)}
            className={`flex-1 px-3 py-1.5 text-xs font-medium transition-colors ${
              currentPace === value
                ? "bg-brand/15 text-brand border-brand/30"
                : "text-muted hover:text-foreground hover:bg-white/[0.03]"
            }`}
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  )
}
```

- [ ] **Step 4: Add ExperimentPaceControl to AutoTunerPanel**

Read `AutoTunerPanel.tsx`. Add the pace control below the `ChampionCard` section, above `ExperimentTimeline`:

```tsx
{status.enabled && !status.paused && <ExperimentPaceControl />}
```

- [ ] **Step 5: Update types + barrel export**

Add `experimentPace` to `AutoTunerStatus` in `types.ts`. Export `ExperimentPaceControl` from `index.ts`.

- [ ] **Step 6: Verify frontend**

Run: `cd desktop-ui && bun run lint:fix && bun run build`
Expected: Clean build.

- [ ] **Step 7: Verify backend**

Run: `cargo check --workspace && cargo clippy -p app-core -p desktop --all-targets`
Expected: Clean.

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/autotuner/ crates/app-core/src/handlers/autotuner.rs crates/desktop/src/commands/autotuner.rs
git commit -m "feat(desktop): add experiment pace control (conservative/balanced/bold)"
```

---

### Task 4: Final verification + lint

- [ ] **Step 1: Run frontend build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`
Expected: Clean.

- [ ] **Step 2: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All pass.

- [ ] **Step 3: Run backend checks**

Run: `cargo check --workspace && cargo clippy --workspace --all-targets --all-features`
Expected: Clean (only pre-existing warnings).

- [ ] **Step 4: Run backend tests**

Run: `cargo nextest run -p app-core -p desktop --no-fail-fast`
Expected: All pass.

- [ ] **Step 5: Visual verification**

Open `http://localhost:1420` in browser:
1. Navigate to chat — verify `AmbientIndicator` shows brain health dot + contextual text
2. Navigate to Settings → General → AI Self-Improvement — verify:
   - `BrainHealthBadge` with dot + status label
   - Brain growth metrics (corrections, evaluated, promoted)
   - MetricsHealth dots (green/gray)
   - Experiment Pace toggle (conservative/balanced/bold)
3. If a promotion occurs — verify toast appears in chat with "Show me" link

- [ ] **Step 6: Commit if any fixes**

```bash
git add -A && git commit -m "chore: fix lint/format from transparency panel"
```

---

## Dependency Graph

```
Task 1 (BrainHealthBadge + AmbientIndicator) — independent
Task 2 (PromotionToast) — independent
Task 3 (ExperimentPaceControl) — independent (needs small backend change)
Task 4 (verification) — depends on all

Tasks 1, 2, 3 can run in parallel.
Task 4 runs last.
```
