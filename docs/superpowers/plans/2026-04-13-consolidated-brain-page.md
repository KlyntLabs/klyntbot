# Consolidated Brain Page & AI Settings — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate 3 AI pages (Coaching, Brain/Mirror, System) into a single "Brain" dashboard with card-grid + in-page expansion, consolidate AI settings into a single Settings tab, and clean up removed routes/sidebar items.

**Architecture:** The new Brain page uses a card-grid overview with in-place expansion to detail views. Each card reuses existing tab/page components from coaching, mirror, system, and debug features. A new AI settings page extracts AI-related sections from GeneralSettings and PersonalizationSettings. The System and Coaching sidebar items are removed; Categories moves to Settings.

**Tech Stack:** React, React Router, Tailwind CSS v4, Tauri IPC via `useQuery`/`useMutation`, Lucide icons.

**Spec:** `docs/superpowers/specs/2026-04-13-consolidated-brain-page-design.md`

---

## File Structure

### New files:
- `desktop-ui/src/features/brain/BrainPage.tsx` — Main page: health strip + card grid + activity stream
- `desktop-ui/src/features/brain/components/HealthStrip.tsx` — 4 metric cards at top
- `desktop-ui/src/features/brain/components/BrainCard.tsx` — Reusable summary card with expand behavior
- `desktop-ui/src/features/brain/components/MemoryDetail.tsx` — Memory & Knowledge detail (wraps MemoryTab)
- `desktop-ui/src/features/brain/components/CoachingDetail.tsx` — Coaching detail (wraps coaching components)
- `desktop-ui/src/features/brain/components/MirrorDetail.tsx` — Mirror detail (wraps mirror components)
- `desktop-ui/src/features/brain/components/ContextsDetail.tsx` — Contexts detail (wraps ContextsTab + inference stats)
- `desktop-ui/src/features/brain/components/ActivityStream.tsx` — Collapsible events/pipeline stream
- `desktop-ui/src/features/brain/index.ts` — Barrel export
- `desktop-ui/src/features/settings/pages/AiSettings.tsx` — Consolidated AI settings tab
- `desktop-ui/src/features/settings/pages/CategoriesSettings.tsx` — Categories (moved from system)

### Modified files:
- `desktop-ui/src/app/router.tsx` — Replace brain/coaching/system routes, add AI settings route, add categories settings route
- `desktop-ui/src/app/layouts/Sidebar.tsx` — Remove Coaching and System items
- `desktop-ui/src/features/settings/components/SettingsLayout.tsx` — Add "AI" and "Categories" tabs
- `desktop-ui/src/features/settings/index.ts` — Add AiSettings and CategoriesSettings exports
- `desktop-ui/src/features/settings/pages/GeneralSettings.tsx` — Remove agent defaults and autotuner sections
- `desktop-ui/src/features/settings/pages/PersonalizationSettings.tsx` — Remove cognitive/learning/routing sections (keep theme + provider keys)

### Untouched (reused as-is):
- `desktop-ui/src/features/mirror/components/*.tsx` — All mirror subcomponents
- `desktop-ui/src/features/coaching/components/*.tsx` — All coaching subcomponents
- `desktop-ui/src/features/coaching/pages/*.tsx` — Overview, Patterns, History pages
- `desktop-ui/src/features/debug/components/tabs/*.tsx` — MemoryTab, EventsTab, PipelineTab
- `desktop-ui/src/features/system/components/tabs/ContextsTab.tsx` — Contexts tab
- `desktop-ui/src/features/system/components/tabs/InferenceTab.tsx` — Inference tab
- `desktop-ui/src/features/autotuner/*.tsx` — AutoTuner components and hooks

---

## Task 1: Create BrainCard component

**Files:**
- Create: `desktop-ui/src/features/brain/components/BrainCard.tsx`

- [ ] **Step 1: Create BrainCard component**

```tsx
import { ArrowLeft } from "lucide-react";
import type { ReactNode } from "react";

interface BrainCardProps {
  id: string;
  title: string;
  subtitle: string;
  icon: ReactNode;
  accentClass: string;
  summary: ReactNode;
  detail: ReactNode;
  expanded: boolean;
  onExpand: () => void;
  onCollapse: () => void;
  actions?: ReactNode;
}

export function BrainCard({
  title,
  subtitle,
  icon,
  accentClass,
  summary,
  detail,
  expanded,
  onExpand,
  onCollapse,
  actions,
}: BrainCardProps) {
  if (expanded) {
    return (
      <div className="animate-in fade-in duration-200">
        {/* Detail header */}
        <div className="flex items-center gap-3 mb-5">
          <button
            type="button"
            onClick={onCollapse}
            className="size-7 rounded-lg bg-surface-low flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors"
          >
            <ArrowLeft className="size-3.5" />
          </button>
          <div className="flex items-center gap-2.5 flex-1 min-w-0">
            <div className={`size-8 rounded-lg flex items-center justify-center ${accentClass}`}>
              {icon}
            </div>
            <div className="min-w-0">
              <h2 className="text-sm font-semibold text-foreground">{title}</h2>
              <p className="text-2xs text-muted-foreground">{subtitle}</p>
            </div>
          </div>
          {actions && <div className="flex items-center gap-2">{actions}</div>}
        </div>
        {/* Detail content */}
        {detail}
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={onExpand}
      className="w-full text-left bg-surface-lowest border border-border rounded-xl p-5 hover:border-border-hover transition-colors duration-200 cursor-pointer"
    >
      <div className="flex items-center gap-2.5 mb-3.5">
        <div className={`size-8 rounded-lg flex items-center justify-center ${accentClass}`}>
          {icon}
        </div>
        <div className="min-w-0">
          <h3 className="text-[13px] font-semibold text-foreground">{title}</h3>
          <p className="text-2xs text-muted-foreground">{subtitle}</p>
        </div>
      </div>
      {summary}
    </button>
  );
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: no errors in BrainCard.tsx

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/brain/components/BrainCard.tsx
git commit -m "feat(desktop-ui): add BrainCard component for dashboard"
```

---

## Task 2: Create HealthStrip component

**Files:**
- Create: `desktop-ui/src/features/brain/components/HealthStrip.tsx`

- [ ] **Step 1: Create HealthStrip component**

```tsx
import { useQuery } from "@shared/hooks/useQuery";

interface MemoryStats {
  activeFacts: number;
  archivedFacts: number;
  episodicCount: number;
  rulesCount: number;
}

interface MirrorState {
  latestBrainVersion: { version: number; promotedAt: string } | null;
  recentTrialPreviews: unknown[];
}

interface CoachingSituation {
  energyLevel?: number;
  focusState?: number;
}

interface CoachingInterventions {
  length: number;
}

export function HealthStrip() {
  const { data: memStats } = useQuery<MemoryStats>("cognitive_memory_stats", undefined, {
    activeFacts: 0,
    archivedFacts: 0,
    episodicCount: 0,
    rulesCount: 0,
  });

  const { data: mirrorState } = useQuery<MirrorState>("get_mirror_state", undefined, {
    latestBrainVersion: null,
    recentTrialPreviews: [],
  });

  const { data: situation } = useQuery<CoachingSituation>("coaching_situation", undefined, {});

  const { data: interventions } = useQuery<CoachingInterventions[]>(
    "coaching_pending_interventions",
    undefined,
    [],
  );

  const brainVersion = mirrorState.latestBrainVersion;
  const trialCount = mirrorState.recentTrialPreviews?.length ?? 0;
  const pendingCount = interventions?.length ?? 0;

  return (
    <div className="grid grid-cols-4 gap-3">
      <MetricCard
        label="Knowledge Trust"
        value={`${memStats.activeFacts}`}
        sub={`${memStats.activeFacts} facts · ${memStats.episodicCount} episodic`}
        valueClass="text-success"
      />
      <MetricCard
        label="Brain Version"
        value={brainVersion ? `v${brainVersion.version}` : "v1"}
        sub={brainVersion ? new Date(brainVersion.promotedAt).toLocaleDateString() : "Initial"}
        valueClass="text-foreground"
      />
      <MetricCard
        label="Coaching"
        value={situation.focusState !== undefined ? "Active" : "Idle"}
        sub={`${pendingCount} pending`}
        valueClass="text-info"
      />
      <MetricCard
        label="Experiments"
        value={`${trialCount}`}
        sub={trialCount === 0 ? "No active trials" : `${trialCount} active`}
        valueClass="text-purple"
      />
    </div>
  );
}

function MetricCard({
  label,
  value,
  sub,
  valueClass,
}: {
  label: string;
  value: string;
  sub: string;
  valueClass: string;
}) {
  return (
    <div className="bg-surface-lowest border border-border rounded-xl px-4 py-3">
      <p className="text-2xs uppercase tracking-wide text-dim mb-1">{label}</p>
      <p className={`text-xl font-semibold ${valueClass}`}>{value}</p>
      <p className="text-2xs text-dim mt-0.5">{sub}</p>
    </div>
  );
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/brain/components/HealthStrip.tsx
git commit -m "feat(desktop-ui): add HealthStrip component for brain dashboard"
```

---

## Task 3: Create detail wrapper components

**Files:**
- Create: `desktop-ui/src/features/brain/components/MemoryDetail.tsx`
- Create: `desktop-ui/src/features/brain/components/CoachingDetail.tsx`
- Create: `desktop-ui/src/features/brain/components/MirrorDetail.tsx`
- Create: `desktop-ui/src/features/brain/components/ContextsDetail.tsx`

These are thin wrappers that compose existing components into the detail view layout.

- [ ] **Step 1: Create MemoryDetail**

```tsx
import { MemoryTab } from "@features/debug";

export function MemoryDetail() {
  return <MemoryTab />;
}
```

- [ ] **Step 2: Create CoachingDetail**

This wraps the coaching overview, patterns, and history into a single detail view with internal sub-tabs.

```tsx
import { useState } from "react";
import { CoachingOverviewPage } from "@features/coaching";
import { PatternsPage } from "@features/coaching";
import { HistoryPage } from "@features/coaching";

type CoachingSection = "overview" | "patterns" | "history";

const sections: { id: CoachingSection; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "patterns", label: "Patterns" },
  { id: "history", label: "History" },
];

export function CoachingDetail() {
  const [active, setActive] = useState<CoachingSection>("overview");

  return (
    <div className="flex flex-col gap-3">
      {/* Sub-tabs */}
      <div className="flex items-center gap-1.5">
        {sections.map((s) => (
          <button
            key={s.id}
            type="button"
            onClick={() => setActive(s.id)}
            className={`px-3 py-1.5 rounded-lg text-xs font-light transition-colors ${
              active === s.id
                ? "bg-surface-low text-foreground"
                : "text-muted-foreground hover:text-foreground"
            }`}
          >
            {s.label}
          </button>
        ))}
      </div>
      {/* Content */}
      {active === "overview" && <CoachingOverviewPage />}
      {active === "patterns" && <PatternsPage />}
      {active === "history" && <HistoryPage />}
    </div>
  );
}
```

- [ ] **Step 3: Create MirrorDetail**

```tsx
import { useQuery } from "@shared/hooks/useQuery";
import { BrainTimeline } from "@features/mirror/components/BrainTimeline";
import type { BrainVersion } from "@features/mirror/components/BrainTimeline";
import { ExperimentWatchlist } from "@features/mirror/components/ExperimentWatchlist";
import type { TrialPreview } from "@features/mirror/components/ExperimentWatchlist";
import { MetaRulesSection } from "@features/mirror/components/MetaRulesSection";
import type { MetaRule } from "@features/mirror/components/MetaRulesSection";
import { MirrorInput } from "@features/mirror/components/MirrorInput";
import { NarrativeCard } from "@features/mirror/components/NarrativeCard";
import type { TrendNarrative } from "@features/mirror/components/NarrativeCard";
import { RoutingDonut } from "@features/mirror/components/RoutingDonut";
import type { RoutingSnapshot } from "@features/mirror/components/RoutingDonut";
import { SnippetFeed } from "@features/mirror/components/SnippetFeed";
import type { NarrativeSnippet } from "@features/mirror/components/SnippetFeed";

interface MirrorState {
  lastRoutingSnapshot: RoutingSnapshot | null;
  latestTrendNarrative: TrendNarrative | null;
  pendingSnippets: NarrativeSnippet[];
  activeMetaRules: MetaRule[];
  pendingMetaRules: MetaRule[];
  latestBrainVersion: BrainVersion | null;
  recentTrialPreviews: TrialPreview[];
}

const DEFAULT_MIRROR_STATE: MirrorState = {
  lastRoutingSnapshot: null,
  latestTrendNarrative: null,
  pendingSnippets: [],
  activeMetaRules: [],
  pendingMetaRules: [],
  latestBrainVersion: null,
  recentTrialPreviews: [],
};

export function MirrorDetail() {
  const { data: mirrorState, refetch } = useQuery<MirrorState>(
    "get_mirror_state",
    undefined,
    DEFAULT_MIRROR_STATE,
  );

  return (
    <div className="flex flex-col gap-6 max-w-2xl">
      <NarrativeCard narrative={mirrorState?.latestTrendNarrative} />
      <SnippetFeed snippets={mirrorState?.pendingSnippets ?? []} />
      <ExperimentWatchlist previews={mirrorState?.recentTrialPreviews ?? []} onAction={refetch} />
      <MetaRulesSection
        activeRules={mirrorState?.activeMetaRules ?? []}
        pendingRules={mirrorState?.pendingMetaRules ?? []}
        onRuleAction={refetch}
      />
      <BrainTimeline />
      <RoutingDonut snapshot={mirrorState?.lastRoutingSnapshot} />
      <MirrorInput />
    </div>
  );
}
```

- [ ] **Step 4: Create ContextsDetail**

```tsx
import { lazy, Suspense } from "react";

const ContextsTab = lazy(() =>
  import("@features/system/components/tabs/ContextsTab").then((m) => ({
    default: m.ContextsTab,
  })),
);

export function ContextsDetail() {
  return (
    <Suspense
      fallback={
        <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
          Loading...
        </div>
      }
    >
      <ContextsTab />
    </Suspense>
  );
}
```

- [ ] **Step 5: Verify all compile**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/brain/components/MemoryDetail.tsx desktop-ui/src/features/brain/components/CoachingDetail.tsx desktop-ui/src/features/brain/components/MirrorDetail.tsx desktop-ui/src/features/brain/components/ContextsDetail.tsx
git commit -m "feat(desktop-ui): add detail wrapper components for brain dashboard"
```

---

## Task 4: Create ActivityStream component

**Files:**
- Create: `desktop-ui/src/features/brain/components/ActivityStream.tsx`

- [ ] **Step 1: Create ActivityStream**

```tsx
import { lazy, Suspense, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { DEV_SSE_BASE, isTauri } from "@shared/lib/utils";
import { useEffect } from "react";

const EventsTab = lazy(() =>
  import("@features/debug/components/tabs/EventsTab").then((m) => ({ default: m.EventsTab })),
);
const PipelineTab = lazy(() =>
  import("@features/debug/components/tabs/PipelineTab").then((m) => ({ default: m.PipelineTab })),
);

const COGNITIVE_SSE_EVENTS = [
  "cognitive:domain_event",
  "cognitive:extraction",
  "cognitive:consolidation",
];

type StreamTab = "events" | "pipeline";

export function ActivityStream() {
  const [open, setOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<StreamTab>("events");

  // Bridge SSE in browser dev mode
  useEffect(() => {
    if (!open || isTauri) return;
    const es = new EventSource(`${DEV_SSE_BASE}/api/cognitive/stream`);
    for (const eventName of COGNITIVE_SSE_EVENTS) {
      es.addEventListener(eventName, (e: MessageEvent) => {
        try {
          const payload = JSON.parse(e.data);
          window.dispatchEvent(new CustomEvent(eventName, { detail: payload }));
        } catch {
          /* skip malformed */
        }
      });
    }
    return () => es.close();
  }, [open]);

  return (
    <div className="border-t border-border pt-4">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors w-full"
      >
        {open ? (
          <ChevronDown className="size-3.5" />
        ) : (
          <ChevronRight className="size-3.5" />
        )}
        <span className="text-[13px] font-medium">Activity Stream</span>
        <span className="text-2xs text-dim ml-1">Events · Pipeline</span>
      </button>

      {open && (
        <div className="mt-3 animate-in fade-in duration-200">
          <div className="flex items-center gap-1.5 mb-3">
            {(["events", "pipeline"] as const).map((tab) => (
              <button
                key={tab}
                type="button"
                onClick={() => setActiveTab(tab)}
                className={`px-3 py-1.5 rounded-lg text-xs font-light transition-colors capitalize ${
                  activeTab === tab
                    ? "bg-surface-low text-foreground"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                {tab}
              </button>
            ))}
          </div>
          <div className="h-[400px] overflow-y-auto">
            <Suspense fallback={<div className="text-sm text-muted-foreground p-4">Loading...</div>}>
              {activeTab === "events" && <EventsTab />}
              {activeTab === "pipeline" && <PipelineTab />}
            </Suspense>
          </div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/brain/components/ActivityStream.tsx
git commit -m "feat(desktop-ui): add ActivityStream collapsible for brain dashboard"
```

---

## Task 5: Create BrainPage (main dashboard)

**Files:**
- Create: `desktop-ui/src/features/brain/BrainPage.tsx`
- Create: `desktop-ui/src/features/brain/index.ts`

- [ ] **Step 1: Create BrainPage**

```tsx
import { useQuery } from "@shared/hooks/useQuery";
import { Brain, Crosshair, Eye, Boxes } from "lucide-react";
import { useNavigate, useParams } from "react-router";
import { BrainCard } from "./components/BrainCard";
import { HealthStrip } from "./components/HealthStrip";
import { MemoryDetail } from "./components/MemoryDetail";
import { CoachingDetail } from "./components/CoachingDetail";
import { MirrorDetail } from "./components/MirrorDetail";
import { ContextsDetail } from "./components/ContextsDetail";
import { ActivityStream } from "./components/ActivityStream";

type BrainSection = "memory" | "coaching" | "mirror" | "contexts";

interface MemoryStats {
  activeFacts: number;
  episodicCount: number;
  rulesCount: number;
}

interface MirrorState {
  latestTrendNarrative: { narrative: string } | null;
  latestBrainVersion: { version: number } | null;
  pendingMetaRules: unknown[];
  recentTrialPreviews: unknown[];
}

interface CoachingSituation {
  energyLevel?: number;
  focusState?: number;
  deadlinePressure?: number;
  coachingReceptivity?: number;
}

interface InferenceStats {
  activeContexts: number;
  archivedContexts: number;
  assignmentRate: number;
}

export function BrainPage() {
  const navigate = useNavigate();
  const { section } = useParams<{ section?: string }>();
  const expanded = (section as BrainSection) || null;

  const setExpanded = (s: BrainSection | null) => {
    navigate(s ? `/brain/${s}` : "/brain", { replace: true });
  };

  // Lightweight queries for summary cards (only when overview is shown)
  const { data: memStats } = useQuery<MemoryStats>(
    "cognitive_memory_stats",
    undefined,
    { activeFacts: 0, episodicCount: 0, rulesCount: 0 },
  );

  const { data: mirrorState } = useQuery<MirrorState>(
    "get_mirror_state",
    undefined,
    { latestTrendNarrative: null, latestBrainVersion: null, pendingMetaRules: [], recentTrialPreviews: [] },
  );

  const { data: situation } = useQuery<CoachingSituation>(
    "coaching_situation",
    undefined,
    {},
  );

  const { data: inferenceStats } = useQuery<InferenceStats>(
    "get_inference_stats",
    undefined,
    { activeContexts: 0, archivedContexts: 0, assignmentRate: 0 },
  );

  const cards: {
    id: BrainSection;
    title: string;
    subtitle: string;
    icon: React.ReactNode;
    accentClass: string;
    summary: React.ReactNode;
    detail: React.ReactNode;
  }[] = [
    {
      id: "memory",
      title: "Memory & Knowledge",
      subtitle: "User model, semantic facts, episodic memories",
      icon: <Brain className="size-4 text-success" strokeWidth={1.5} />,
      accentClass: "bg-success/15",
      summary: (
        <div className="flex gap-5 text-xs">
          <span>
            <span className="text-lg font-semibold text-success">{memStats.activeFacts}</span>{" "}
            <span className="text-muted-foreground">facts</span>
          </span>
          <span>
            <span className="text-lg font-semibold text-info">{memStats.episodicCount}</span>{" "}
            <span className="text-muted-foreground">episodic</span>
          </span>
          <span>
            <span className="text-lg font-semibold text-purple">{memStats.rulesCount}</span>{" "}
            <span className="text-muted-foreground">rules</span>
          </span>
        </div>
      ),
      detail: <MemoryDetail />,
    },
    {
      id: "coaching",
      title: "Coaching & Patterns",
      subtitle: "Situation awareness, interventions, behavior patterns",
      icon: <Crosshair className="size-4 text-info" strokeWidth={1.5} />,
      accentClass: "bg-info/15",
      summary: (
        <div className="flex items-center gap-4">
          {[
            { label: "Energy", value: situation.energyLevel },
            { label: "Focus", value: situation.focusState },
            { label: "Deadline", value: situation.deadlinePressure },
            { label: "Receptive", value: situation.coachingReceptivity },
          ].map((g) => (
            <div key={g.label} className="text-center">
              <div className="size-9 rounded-full border-2 border-info/40 flex items-center justify-center text-2xs font-semibold text-info">
                {g.value ?? "—"}
              </div>
              <p className="text-2xs text-dim mt-1">{g.label}</p>
            </div>
          ))}
        </div>
      ),
      detail: <CoachingDetail />,
    },
    {
      id: "mirror",
      title: "Mirror & Reflection",
      subtitle: "Weekly reflections, brain versions, skill routing",
      icon: <Eye className="size-4 text-purple" strokeWidth={1.5} />,
      accentClass: "bg-purple/15",
      summary: (
        <div>
          <div className="bg-surface-low rounded-lg p-3 mb-2">
            <p className="text-2xs text-dim mb-1">Latest Reflection</p>
            <p className="text-xs text-muted-foreground italic line-clamp-2">
              {mirrorState.latestTrendNarrative?.narrative ??
                "Your first weekly reflection will appear after 7 days of use."}
            </p>
          </div>
          <p className="text-2xs text-dim">
            Brain v{mirrorState.latestBrainVersion?.version ?? 1} ·{" "}
            {mirrorState.pendingMetaRules?.length ?? 0} meta-rules ·{" "}
            {mirrorState.recentTrialPreviews?.length ?? 0} trials
          </p>
        </div>
      ),
      detail: <MirrorDetail />,
    },
    {
      id: "contexts",
      title: "Contexts & Inference",
      subtitle: "Work context detection, assignment, merging",
      icon: <Boxes className="size-4 text-warning" strokeWidth={1.5} />,
      accentClass: "bg-warning/15",
      summary: (
        <div className="flex gap-5 text-xs">
          <span>
            <span className="text-lg font-semibold text-warning">{inferenceStats.activeContexts}</span>{" "}
            <span className="text-muted-foreground">active</span>
          </span>
          <span>
            <span className="text-lg font-semibold text-dim">{inferenceStats.archivedContexts}</span>{" "}
            <span className="text-muted-foreground">archived</span>
          </span>
          <span>
            <span className="text-lg font-semibold text-info">
              {Math.round(inferenceStats.assignmentRate * 100)}%
            </span>{" "}
            <span className="text-muted-foreground">assignment</span>
          </span>
        </div>
      ),
      detail: <ContextsDetail />,
    },
  ];

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0 overflow-y-auto">
      <div className="flex flex-col gap-5 p-6 max-w-4xl w-full mx-auto">
        {/* Health Strip — always visible unless in detail view */}
        {!expanded && <HealthStrip />}

        {/* Card grid or expanded detail */}
        {expanded ? (
          (() => {
            const card = cards.find((c) => c.id === expanded);
            if (!card) return null;
            return (
              <BrainCard
                id={card.id}
                title={card.title}
                subtitle={card.subtitle}
                icon={card.icon}
                accentClass={card.accentClass}
                summary={card.summary}
                detail={card.detail}
                expanded={true}
                onExpand={() => {}}
                onCollapse={() => setExpanded(null)}
              />
            );
          })()
        ) : (
          <div className="grid grid-cols-2 gap-4">
            {cards.map((card) => (
              <BrainCard
                key={card.id}
                id={card.id}
                title={card.title}
                subtitle={card.subtitle}
                icon={card.icon}
                accentClass={card.accentClass}
                summary={card.summary}
                detail={card.detail}
                expanded={false}
                onExpand={() => setExpanded(card.id)}
                onCollapse={() => {}}
              />
            ))}
          </div>
        )}

        {/* Activity Stream — only on overview */}
        {!expanded && <ActivityStream />}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create barrel export**

```tsx
// desktop-ui/src/features/brain/index.ts
export { BrainPage } from "./BrainPage";
```

- [ ] **Step 3: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/brain/
git commit -m "feat(desktop-ui): add consolidated BrainPage dashboard"
```

---

## Task 6: Create AiSettings page

**Files:**
- Create: `desktop-ui/src/features/settings/pages/AiSettings.tsx`

- [ ] **Step 1: Create AiSettings**

This extracts agent defaults from GeneralSettings and cognitive/learning/routing from PersonalizationSettings into one page. Reuses the same `config_get_section`/`config_update_section` IPC pattern.

```tsx
import { AutoTunerPanel } from "@features/autotuner";
import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import { SaveButton, Toggle } from "@shared/ui";
import { useState } from "react";
import { COGNITIVE_MODELS } from "../shared/cognitive-models";

// ── Constants ───────────────────────────────────────────────────────

const PROVIDERS = [
  { value: "anthropic", label: "Anthropic" },
  { value: "openai", label: "OpenAI" },
  { value: "openrouter", label: "OpenRouter" },
  { value: "deepseek", label: "DeepSeek" },
  { value: "gemini", label: "Google Gemini" },
  { value: "groq", label: "Groq" },
  { value: "vllm", label: "vLLM" },
  { value: "zhipu", label: "Zhipu" },
  { value: "dashscope", label: "DashScope" },
  { value: "moonshot", label: "Moonshot" },
  { value: "minimax", label: "MiniMax" },
  { value: "aihubmix", label: "AIHubMix" },
] as const;

const MODEL_PRESETS: Record<string, { value: string; label: string }[]> = {
  anthropic: [
    { value: "anthropic/claude-opus-4-5", label: "Claude Opus 4.5" },
    { value: "anthropic/claude-sonnet-4-5", label: "Claude Sonnet 4.5" },
    { value: "anthropic/claude-haiku-3-5", label: "Claude Haiku 3.5" },
  ],
  openai: [
    { value: "openai/gpt-4o", label: "GPT-4o" },
    { value: "openai/gpt-4o-mini", label: "GPT-4o Mini" },
  ],
  deepseek: [
    { value: "deepseek/deepseek-chat", label: "DeepSeek Chat" },
    { value: "deepseek/deepseek-reasoner", label: "DeepSeek Reasoner" },
  ],
  gemini: [
    { value: "gemini/gemini-2.5-pro", label: "Gemini 2.5 Pro" },
    { value: "gemini/gemini-2.5-flash", label: "Gemini 2.5 Flash" },
  ],
};

const MAX_TOKEN_OPTIONS = [
  { value: 2048, label: "2,048" },
  { value: 4096, label: "4,096" },
  { value: 8192, label: "8,192" },
  { value: 16384, label: "16,384" },
  { value: 32768, label: "32,768" },
];

const ANALYSIS_INTERVAL_OPTIONS = [
  { value: 900, label: "15 minutes" },
  { value: 1800, label: "30 minutes" },
  { value: 3600, label: "1 hour" },
  { value: 7200, label: "2 hours" },
  { value: 14400, label: "4 hours" },
];

// ── Types ───────────────────────────────────────────────────────────

interface AgentsConfig {
  defaults?: {
    model?: string;
    provider?: string;
    temperature?: number;
    maxTokens?: number;
  };
  monthlyBudgetUsd?: number;
}

interface CognitiveData {
  provider?: string;
  model?: string;
  temperature?: number;
  maxTokens?: number;
  intelligenceMode?: string;
  atomExtraction?: { enabled?: boolean };
}

interface LearningData {
  enabled?: boolean;
  analysisIntervalSecs?: number;
  minThreshold?: number;
  maxThreshold?: number;
  minOutcomesForAdaptation?: number;
}

interface ProviderManagerData {
  primary?: string;
  fallback?: string;
  classifierModel?: string;
}

// ── Component ───────────────────────────────────────────────────────

export function AiSettings() {
  const toast = useToastContext();

  // ── Data fetching ───────────────────────────────────────────────
  const { data: agents, refetch: refetchAgents } = useQuery<AgentsConfig>(
    "config_get_section",
    { section: "agents" },
    { defaults: {} },
  );

  const { data: cognitive, refetch: refetchCognitive } = useQuery<CognitiveData>(
    "config_get_section",
    { section: "cognitive" },
    {},
  );

  const { data: learning, refetch: refetchLearning } = useQuery<LearningData>(
    "config_get_section",
    { section: "learning" },
    { enabled: true },
  );

  const { data: providerManager, refetch: refetchPm } = useQuery<ProviderManagerData>(
    "config_get_section",
    { section: "providerManager" },
    {},
  );

  // ── Agent defaults state ────────────────────────────────────────
  const defaults = agents.defaults ?? {};
  const [agentEdits, setAgentEdits] = useState<Record<string, unknown>>({});
  const [savingAgent, setSavingAgent] = useState(false);

  const agentVal = <T,>(key: string, fallback: T): T => {
    if (key in agentEdits) return agentEdits[key] as T;
    return ((defaults as Record<string, unknown>)[key] ?? fallback) as T;
  };

  const hasAgentChanges = Object.keys(agentEdits).length > 0;

  const activeProvider = agentVal("provider", "") as string;
  const modelOptions = MODEL_PRESETS[activeProvider] ?? [];

  const saveAgentDefaults = async () => {
    setSavingAgent(true);
    try {
      await ipc("config_update_section", {
        section: "agents",
        patch: { defaults: agentEdits },
      });
      refetchAgents();
      setAgentEdits({});
    } catch {
      toast.show("Failed to save agent defaults");
    } finally {
      setSavingAgent(false);
    }
  };

  // ── Provider routing state ──────────────────────────────────────
  const [pmEdits, setPmEdits] = useState<Record<string, unknown>>({});
  const [savingPm, setSavingPm] = useState(false);

  const pmVal = (key: string): string => {
    if (key in pmEdits) return (pmEdits[key] ?? "") as string;
    return ((providerManager as Record<string, unknown>)[key] ?? "") as string;
  };

  const hasPmChanges = Object.keys(pmEdits).length > 0;

  const savePm = async () => {
    setSavingPm(true);
    try {
      const patch: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(pmEdits)) patch[k] = v || null;
      await ipc("config_update_section", { section: "providerManager", patch });
      setPmEdits({});
      refetchPm();
    } catch {
      toast.show("Failed to save provider routing");
    } finally {
      setSavingPm(false);
    }
  };

  // ── Cognitive state ─────────────────────────────────────────────
  const [cogEdits, setCogEdits] = useState<Record<string, unknown>>({});
  const [savingCog, setSavingCog] = useState(false);

  const cogProvider = ("provider" in cogEdits ? cogEdits.provider : (cognitive.provider ?? "")) as string;
  const cogModel = ("model" in cogEdits ? cogEdits.model : (cognitive.model ?? "")) as string;
  const effectiveCogProvider = cogProvider || activeProvider;
  const cogModelOptions = COGNITIVE_MODELS[effectiveCogProvider] ?? [];

  const hasCogChanges = Object.keys(cogEdits).length > 0;

  const saveCognitive = async () => {
    setSavingCog(true);
    try {
      const patch: Record<string, unknown> = {};
      if ("provider" in cogEdits) patch.provider = cogEdits.provider || null;
      if ("model" in cogEdits) patch.model = cogEdits.model || null;
      if ("temperature" in cogEdits) patch.temperature = cogEdits.temperature;
      if ("maxTokens" in cogEdits) patch.maxTokens = cogEdits.maxTokens;
      if ("atomExtraction.enabled" in cogEdits) {
        patch.atomExtraction = { enabled: cogEdits["atomExtraction.enabled"] };
      }
      if ("intelligenceMode" in cogEdits) patch.intelligenceMode = cogEdits.intelligenceMode;
      await ipc("config_update_section", { section: "cognitive", patch });
      refetchCognitive();
      setCogEdits({});
    } catch {
      toast.show("Failed to save cognitive config");
    } finally {
      setSavingCog(false);
    }
  };

  // ── Learning state ──────────────────────────────────────────────
  const [learnEdits, setLearnEdits] = useState<Record<string, unknown>>({});
  const [savingLearn, setSavingLearn] = useState(false);

  const learnVal = <T,>(key: string, fallback: T): T => {
    if (key in learnEdits) return learnEdits[key] as T;
    return ((learning as Record<string, unknown>)[key] ?? fallback) as T;
  };

  const hasLearnChanges = Object.keys(learnEdits).length > 0;

  const saveLearn = async () => {
    setSavingLearn(true);
    try {
      await ipc("config_update_section", { section: "learning", patch: learnEdits });
      refetchLearning();
      setLearnEdits({});
    } catch {
      toast.show("Failed to save learning config");
    } finally {
      setSavingLearn(false);
    }
  };

  // ── Shared select class ─────────────────────────────────────────
  const selectClass =
    "w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors";

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">AI</h2>
        <p className="text-[13px] text-muted-foreground mt-1">
          Model defaults, cognitive pipeline, learning, and optimization
        </p>
      </div>

      <div className="space-y-4">
        {/* ── Agent Defaults ─────────────────────────────────── */}
        <SettingsCard title="Agent Defaults">
          <div className="space-y-3">
            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Provider</span>
              <select
                value={agentVal("provider", "")}
                onChange={(e) => setAgentEdits((prev) => ({ ...prev, provider: e.target.value }))}
                className={selectClass}
              >
                <option value="" className="bg-popover">Auto-detect</option>
                {PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value} className="bg-popover">{p.label}</option>
                ))}
              </select>
            </label>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Default model</span>
              {modelOptions.length > 0 ? (
                <select
                  value={agentVal("model", "")}
                  onChange={(e) => setAgentEdits((prev) => ({ ...prev, model: e.target.value }))}
                  className={selectClass}
                >
                  <option value="" className="bg-popover">Default for provider</option>
                  {modelOptions.map((m) => (
                    <option key={m.value} value={m.value} className="bg-popover">{m.label}</option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  value={agentVal("model", "")}
                  onChange={(e) => setAgentEdits((prev) => ({ ...prev, model: e.target.value }))}
                  placeholder="e.g. anthropic/claude-opus-4-5"
                  className={`${selectClass} placeholder:text-dim`}
                />
              )}
            </label>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Temperature</span>
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.1"
                  value={agentVal("temperature", 0.7)}
                  onChange={(e) =>
                    setAgentEdits((prev) => ({ ...prev, temperature: Number.parseFloat(e.target.value) }))
                  }
                  className="w-full accent-brand"
                />
                <span className="text-2xs text-dim">{agentVal("temperature", 0.7)}</span>
              </label>
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Max tokens</span>
                <select
                  value={agentVal("maxTokens", 8192)}
                  onChange={(e) =>
                    setAgentEdits((prev) => ({ ...prev, maxTokens: Number.parseInt(e.target.value, 10) }))
                  }
                  className={selectClass}
                >
                  {MAX_TOKEN_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value} className="bg-popover">{o.label}</option>
                  ))}
                </select>
              </label>
            </div>

            {hasAgentChanges && (
              <div className="flex justify-end">
                <SaveButton onClick={saveAgentDefaults} saving={savingAgent} />
              </div>
            )}
          </div>
        </SettingsCard>

        {/* ── Provider Routing ────────────────────────────────── */}
        <SettingsCard title="Provider Routing">
          <div className="space-y-3">
            <p className="text-[11px] text-dim -mt-1">Automatic failover and routing</p>
            {(["primary", "fallback"] as const).map((field) => (
              <label key={field} className="block">
                <span className="block text-[11px] text-muted-foreground mb-1 capitalize">
                  {field} provider
                </span>
                <select
                  value={pmVal(field)}
                  onChange={(e) => setPmEdits((prev) => ({ ...prev, [field]: e.target.value }))}
                  className={selectClass}
                >
                  <option value="" className="bg-popover">
                    {field === "primary" ? "Auto (use agent default)" : "None"}
                  </option>
                  {PROVIDERS.map((p) => (
                    <option key={p.value} value={p.value} className="bg-popover">{p.label}</option>
                  ))}
                </select>
              </label>
            ))}
            {hasPmChanges && (
              <div className="flex justify-end">
                <SaveButton onClick={savePm} saving={savingPm} />
              </div>
            )}
          </div>
        </SettingsCard>

        {/* ── Cognitive Pipeline ──────────────────────────────── */}
        <SettingsCard title="Cognitive Pipeline">
          <div className="space-y-3">
            <p className="text-[11px] text-dim -mt-1">
              Background AI for memory extraction, consolidation, and reflection
            </p>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Provider override</span>
              <select
                value={cogProvider}
                onChange={(e) =>
                  setCogEdits((prev) => ({ ...prev, provider: e.target.value, model: "" }))
                }
                className={selectClass}
              >
                <option value="" className="bg-popover">
                  Same as main ({PROVIDERS.find((p) => p.value === activeProvider)?.label || "auto"})
                </option>
                {PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value} className="bg-popover">{p.label}</option>
                ))}
              </select>
            </label>

            <div>
              <span className="block text-[11px] text-muted-foreground mb-1">Model</span>
              {cogModelOptions.length > 0 ? (
                <select
                  value={cogModel}
                  onChange={(e) => setCogEdits((prev) => ({ ...prev, model: e.target.value }))}
                  className={selectClass}
                >
                  <option value="" className="bg-popover">Same as main agent model</option>
                  {cogModelOptions.map((m) => (
                    <option key={m.value} value={m.value} className="bg-popover">
                      {m.label}{m.recommended ? " ★" : ""}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  value={cogModel}
                  onChange={(e) => setCogEdits((prev) => ({ ...prev, model: e.target.value }))}
                  placeholder="Leave blank to use main agent model"
                  className={`${selectClass} placeholder:text-dim`}
                />
              )}
            </div>

            <div className="flex items-center justify-between pt-1 border-t border-border">
              <div>
                <span className="text-xs text-muted-foreground">Deep Intelligence Mode</span>
                <p className="text-[11px] text-dim">Full LLM processing instead of heuristics</p>
              </div>
              <Toggle
                checked={
                  "intelligenceMode" in cogEdits
                    ? cogEdits.intelligenceMode === "deep"
                    : cognitive.intelligenceMode === "deep"
                }
                onChange={(v) =>
                  setCogEdits((prev) => ({ ...prev, intelligenceMode: v ? "deep" : "standard" }))
                }
              />
            </div>

            <div className="flex items-center justify-between pt-1 border-t border-border">
              <div>
                <span className="text-xs text-muted-foreground">Auto-extract knowledge atoms</span>
                <p className="text-[11px] text-dim">Extract concepts and facts from notes</p>
              </div>
              <Toggle
                checked={
                  "atomExtraction.enabled" in cogEdits
                    ? (cogEdits["atomExtraction.enabled"] as boolean)
                    : (cognitive.atomExtraction?.enabled ?? true)
                }
                onChange={(v) => setCogEdits((prev) => ({ ...prev, "atomExtraction.enabled": v }))}
              />
            </div>

            {hasCogChanges && (
              <div className="flex justify-end">
                <SaveButton onClick={saveCognitive} saving={savingCog} />
              </div>
            )}
          </div>
        </SettingsCard>

        {/* ── Learning & Adaptation ──────────────────────────── */}
        <SettingsCard title="Learning & Adaptation">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-xs text-muted-foreground">Enable learning</span>
                <p className="text-[11px] text-dim">Adaptive confidence thresholds</p>
              </div>
              <Toggle
                checked={learnVal("enabled", true)}
                onChange={(v) => setLearnEdits((prev) => ({ ...prev, enabled: v }))}
              />
            </div>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Analysis interval</span>
              <select
                value={learnVal("analysisIntervalSecs", 3600)}
                onChange={(e) =>
                  setLearnEdits((prev) => ({
                    ...prev,
                    analysisIntervalSecs: Number.parseInt(e.target.value, 10),
                  }))
                }
                className={selectClass}
              >
                {ANALYSIS_INTERVAL_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value} className="bg-popover">{o.label}</option>
                ))}
              </select>
            </label>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Min threshold</span>
                <input
                  type="number"
                  value={learnVal("minThreshold", 0.4)}
                  onChange={(e) =>
                    setLearnEdits((prev) => ({
                      ...prev,
                      minThreshold: Number.parseFloat(e.target.value) || 0.4,
                    }))
                  }
                  step="0.05"
                  min="0"
                  max="1"
                  className={selectClass}
                />
              </label>
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Max threshold</span>
                <input
                  type="number"
                  value={learnVal("maxThreshold", 0.9)}
                  onChange={(e) =>
                    setLearnEdits((prev) => ({
                      ...prev,
                      maxThreshold: Number.parseFloat(e.target.value) || 0.9,
                    }))
                  }
                  step="0.05"
                  min="0"
                  max="1"
                  className={selectClass}
                />
              </label>
            </div>

            {hasLearnChanges && (
              <div className="flex justify-end">
                <SaveButton onClick={saveLearn} saving={savingLearn} />
              </div>
            )}
          </div>
        </SettingsCard>

        {/* ── AutoTuner ──────────────────────────────────────── */}
        <SettingsCard title="AutoTuner">
          <p className="text-[11px] text-dim mb-3">
            Continuous self-optimization via A/B experiments
          </p>
          <AutoTunerPanel />
        </SettingsCard>

        {/* ── Inference Engine ────────────────────────────────── */}
        <InferenceSettingsCard />
      </div>
    </div>
  );
}

// ── Inference Settings (separate component to isolate state) ─────

function InferenceSettingsCard() {
  const toast = useToastContext();
  const { data: config, refetch } = useQuery<Record<string, unknown>>(
    "config_get_section",
    { section: "inference" },
    {},
  );

  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [saving, setSaving] = useState(false);

  const val = (key: string, fallback: number): number => {
    if (key in edits) return edits[key] as number;
    return (config[key] as number) ?? fallback;
  };

  const hasChanges = Object.keys(edits).length > 0;

  const save = async () => {
    setSaving(true);
    try {
      await ipc("config_update_section", { section: "inference", patch: edits });
      refetch();
      setEdits({});
    } catch {
      toast.show("Failed to save inference config");
    } finally {
      setSaving(false);
    }
  };

  const selectClass =
    "w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors";

  return (
    <SettingsCard title="Inference Engine">
      <div className="space-y-3">
        <p className="text-[11px] text-dim -mt-1">Work context detection and assignment</p>

        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Assignment threshold</span>
          <div className="flex items-center gap-2">
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              value={val("assignmentThreshold", 0.6)}
              onChange={(e) =>
                setEdits((prev) => ({
                  ...prev,
                  assignmentThreshold: Number.parseFloat(e.target.value),
                }))
              }
              className="flex-1 accent-brand"
            />
            <span className="text-2xs text-dim w-8 text-right">
              {val("assignmentThreshold", 0.6)}
            </span>
          </div>
        </label>

        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Merge threshold</span>
          <div className="flex items-center gap-2">
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              value={val("mergeThreshold", 0.8)}
              onChange={(e) =>
                setEdits((prev) => ({
                  ...prev,
                  mergeThreshold: Number.parseFloat(e.target.value),
                }))
              }
              className="flex-1 accent-brand"
            />
            <span className="text-2xs text-dim w-8 text-right">
              {val("mergeThreshold", 0.8)}
            </span>
          </div>
        </label>

        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Max active contexts</span>
          <input
            type="number"
            value={val("maxActiveContexts", 20)}
            onChange={(e) =>
              setEdits((prev) => ({
                ...prev,
                maxActiveContexts: Number.parseInt(e.target.value, 10) || 20,
              }))
            }
            min="5"
            max="100"
            className={selectClass}
          />
        </label>

        {hasChanges && (
          <div className="flex justify-end">
            <SaveButton onClick={save} saving={saving} />
          </div>
        )}
      </div>
    </SettingsCard>
  );
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/settings/pages/AiSettings.tsx
git commit -m "feat(desktop-ui): add consolidated AI settings page"
```

---

## Task 7: Create CategoriesSettings page

**Files:**
- Create: `desktop-ui/src/features/settings/pages/CategoriesSettings.tsx`

- [ ] **Step 1: Create CategoriesSettings**

Thin wrapper that renders the existing CategoriesTab inside the settings layout.

```tsx
import { lazy, Suspense } from "react";

const CategoriesTab = lazy(() =>
  import("@features/system/components/tabs/CategoriesTab").then((m) => ({
    default: m.CategoriesTab,
  })),
);

export function CategoriesSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Categories</h2>
        <p className="text-[13px] text-muted-foreground mt-1">
          Productivity categories and tracked applications
        </p>
      </div>
      <Suspense
        fallback={
          <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
            Loading...
          </div>
        }
      >
        <CategoriesTab />
      </Suspense>
    </div>
  );
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run lint`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/settings/pages/CategoriesSettings.tsx
git commit -m "feat(desktop-ui): add CategoriesSettings page (moved from system)"
```

---

## Task 8: Update router

**Files:**
- Modify: `desktop-ui/src/app/router.tsx`

- [ ] **Step 1: Update lazy imports**

In `desktop-ui/src/app/router.tsx`, replace the Mirror and Coaching lazy imports and add Brain + AI settings imports:

Replace lines 32-49 (Mirror + Coaching imports):

```tsx
// ── Brain Feature (consolidated) ─────────────────────────────
const BrainPage = lazy(() =>
  import("../features/brain").then((m) => ({ default: m.BrainPage })),
);
```

Add after the SettingsLayout imports (around line 84):

```tsx
const AiSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.AiSettings })),
);
const CategoriesSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.CategoriesSettings })),
);
```

Remove the SystemPage lazy import (lines 60-62).

- [ ] **Step 2: Update route definitions**

Replace the brain route (line 209):

```tsx
{ path: "/brain", element: <BrainPage /> },
{ path: "/brain/:section", element: <BrainPage /> },
```

Remove the coaching routes (lines 213-235):

```tsx
// Remove these:
// { path: "/coaching", ... }
// { path: "/coaching/patterns", ... }
// { path: "/coaching/history", ... }
```

Remove the system routes (lines 202-203):

```tsx
// Remove these:
// { path: "/system", ... }
// { path: "/system/:tab", ... }
```

Add redirect for old coaching/system URLs:

```tsx
{ path: "/coaching", element: <Navigate to="/brain/coaching" replace /> },
{ path: "/coaching/*", element: <Navigate to="/brain/coaching" replace /> },
{ path: "/system", element: <Navigate to="/brain/contexts" replace /> },
{ path: "/system/memory", element: <Navigate to="/brain/memory" replace /> },
{ path: "/system/contexts", element: <Navigate to="/brain/contexts" replace /> },
{ path: "/system/events", element: <Navigate to="/brain" replace /> },
{ path: "/system/pipeline", element: <Navigate to="/brain" replace /> },
{ path: "/system/inference", element: <Navigate to="/brain/contexts" replace /> },
{ path: "/system/categories", element: <Navigate to="/settings/categories" replace /> },
```

Update the categories redirect (line 211):

```tsx
{ path: "/categories", element: <Navigate to="/settings/categories" replace /> },
```

Update debug redirect (line 236):

```tsx
{ path: "/debug", element: <Navigate to="/brain" replace /> },
```

Add AI settings + categories settings routes after the existing settings routes:

```tsx
{
  path: "/settings/ai",
  element: (
    <SettingsLayout>
      <AiSettings />
    </SettingsLayout>
  ),
},
{
  path: "/settings/categories",
  element: (
    <SettingsLayout>
      <CategoriesSettings />
    </SettingsLayout>
  ),
},
```

- [ ] **Step 3: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/app/router.tsx
git commit -m "feat(desktop-ui): update routes for consolidated brain page"
```

---

## Task 9: Update Sidebar

**Files:**
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx`

- [ ] **Step 1: Remove Coaching and System items from sidebar**

In `desktop-ui/src/app/layouts/Sidebar.tsx`, update the items array. Remove the Coaching and System entries:

```tsx
const items = [
  { key: "Chat", icon: MessageSquare, path: "/chat" },
  { key: "Dashboard", icon: LayoutDashboard, path: "/" },
  { key: "Tasks", icon: CheckSquare, path: "/tasks" },
  { key: "Notes", icon: FileText, path: "/notes" },
  { key: "Learn", icon: GraduationCap, path: "/learn" },
  { key: "Finance", icon: Wallet, path: "/finance" },
  { key: "Brain", icon: Brain, path: "/brain" },
  { key: "Automations", icon: Timer, path: "/automations" },
  { key: "Settings", icon: Settings, path: "/settings", bottom: true },
];
```

Also remove the `Cpu` and `Sparkles` icon imports if they become unused.

- [ ] **Step 2: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/app/layouts/Sidebar.tsx
git commit -m "feat(desktop-ui): remove Coaching and System from sidebar"
```

---

## Task 10: Update Settings layout and exports

**Files:**
- Modify: `desktop-ui/src/features/settings/components/SettingsLayout.tsx`
- Modify: `desktop-ui/src/features/settings/index.ts`

- [ ] **Step 1: Add AI and Categories to settings sidebar**

In `desktop-ui/src/features/settings/components/SettingsLayout.tsx`, add the new tabs to the `sections` array. Add import for `Sparkles` and `Grid3x3` from lucide-react:

```tsx
import {
  Archive,
  ArrowLeft,
  BrainCircuit,
  Cable,
  Container,
  GitBranch,
  Grid3x3,
  ListChecks,
  Mic,
  Plug,
  Rocket,
  SlidersHorizontal,
  Sparkles,
  User,
  Wrench,
} from "lucide-react";
```

Update the sections array — add AI after General, add Categories after Work Contexts:

```tsx
const sections = [
  { label: "General", path: "/settings/general", icon: SlidersHorizontal },
  { label: "AI", path: "/settings/ai", icon: Sparkles },
  { label: "Configuration", path: "/settings/configuration", icon: Wrench },
  { label: "Personalization", path: "/settings/personalization", icon: User },
  { label: "Voice", path: "/settings/voice", icon: Mic },
  { label: "MCP servers", path: "/settings/mcp", icon: Plug },
  { label: "Git", path: "/settings/git", icon: GitBranch },
  { label: "Environments", path: "/settings/environments", icon: Container },
  { label: "Tasks & Notifications", path: "/settings/tasks", icon: ListChecks },
  { label: "Work Contexts", path: "/settings/work-contexts", icon: BrainCircuit },
  { label: "Categories", path: "/settings/categories", icon: Grid3x3 },
  { label: "Launcher", path: "/settings/launcher", icon: Rocket },
  { label: "Integrations", path: "/settings/integrations", icon: Cable },
  { label: "Archived threads", path: "/settings/archived", icon: Archive },
];
```

- [ ] **Step 2: Add exports to settings index**

In `desktop-ui/src/features/settings/index.ts`, add:

```tsx
export { AiSettings } from "./pages/AiSettings";
export { CategoriesSettings } from "./pages/CategoriesSettings";
```

- [ ] **Step 3: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/settings/components/SettingsLayout.tsx desktop-ui/src/features/settings/index.ts
git commit -m "feat(desktop-ui): add AI and Categories tabs to settings"
```

---

## Task 11: Slim down GeneralSettings and PersonalizationSettings

**Files:**
- Modify: `desktop-ui/src/features/settings/pages/GeneralSettings.tsx`
- Modify: `desktop-ui/src/features/settings/pages/PersonalizationSettings.tsx`

- [ ] **Step 1: Remove agent defaults + autotuner from GeneralSettings**

In `desktop-ui/src/features/settings/pages/GeneralSettings.tsx`:

1. Remove the `AutoTunerPanel` import (line 1)
2. Remove the `AgentDefaults` and `AgentsConfig` interfaces (lines 11-20)
3. Remove the `agentsConfig` query (lines 41-45)
4. Remove all agent defaults state: `defaults`, `model`, `setModel`, `temperature`, `setTemperature`, `maxTokens`, `setMaxTokens`, `saving`, `setSaving` (lines 91-96)
5. Remove `currentModel`, `currentTemp`, `currentMaxTokens`, `handleSaveDefaults`, `hasChanges` (lines 98-123)
6. Remove the "Agent defaults" SettingsCard (lines 191-235)
7. Remove the "AI Self-Improvement" SettingsCard (lines 240-246)

The GeneralSettings should only contain: System info, Keyboard Shortcuts, and Permissions.

- [ ] **Step 2: Remove cognitive/learning/routing from PersonalizationSettings**

In `desktop-ui/src/features/settings/pages/PersonalizationSettings.tsx`:

1. Remove imports: `COGNITIVE_MODELS`, `Toggle` (lines 6, 8)
2. Remove type interfaces: `CognitiveData`, `LearningData`, `ProviderManagerData` (lines 37-58)
3. Remove queries: `learning`, `cognitive`, `providerManager` (lines 76-92)
4. Remove state blocks: `learningEdits`/`savingLearning`/`getLearningValue`/`hasLearningChanges`/`saveLearning` (lines 164-189)
5. Remove state blocks: `cognitiveEdits`/`savingCognitive`/`cogProvider`/`cogModel`/etc/`saveCognitive` (lines 192-231)
6. Remove state blocks: `pmEdits`/`savingPm`/`pmVal`/`hasPmChanges`/`saveProviderManager` (lines 234-260)
7. Remove the "Cognitive AI" SettingsCard (lines 343-462)
8. Remove the "Learning" SettingsCard (lines 464-556)
9. Remove the "Provider routing" SettingsCard (lines 558-621)

The PersonalizationSettings should only contain: Theme and Provider & Model (API keys/base).

- [ ] **Step 3: Verify it compiles**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/settings/pages/GeneralSettings.tsx desktop-ui/src/features/settings/pages/PersonalizationSettings.tsx
git commit -m "refactor(desktop-ui): extract AI settings from General and Personalization"
```

---

## Task 12: Visual testing and polish

**Files:**
- No new files — test existing work

- [ ] **Step 1: Start dev server**

Run: `cd desktop-ui && bun run dev`

In a separate terminal:
Run: `cargo tauri dev`

- [ ] **Step 2: Test Brain page overview**

Navigate to `/brain`. Verify:
- Health strip shows 4 metric cards
- 4 summary cards visible in 2x2 grid
- Activity stream toggle at bottom (collapsed by default)

- [ ] **Step 3: Test card expansion**

Click each card and verify:
- Memory & Knowledge → shows MemoryTab content with back button
- Coaching & Patterns → shows coaching overview with sub-tabs (Overview/Patterns/History)
- Mirror & Reflection → shows all mirror components
- Contexts & Inference → shows contexts tab

- [ ] **Step 4: Test URL routing**

Navigate directly to:
- `/brain/memory` → opens Memory detail
- `/brain/coaching` → opens Coaching detail
- `/brain/mirror` → opens Mirror detail
- `/brain/contexts` → opens Contexts detail
- `/brain` → back to overview

- [ ] **Step 5: Test Activity Stream**

On the overview, click "Activity Stream" toggle. Verify:
- Events tab loads and shows domain events
- Pipeline tab loads and shows extraction/consolidation logs

- [ ] **Step 6: Test Settings > AI**

Navigate to `/settings/ai`. Verify:
- Agent defaults section with provider dropdown, model dropdown, temperature slider, max tokens dropdown
- Provider routing section with primary/fallback dropdowns
- Cognitive pipeline with provider override, model dropdown, intelligence mode toggle, atom extraction toggle
- Learning & adaptation with enable toggle, analysis interval dropdown, threshold inputs
- AutoTuner panel renders

- [ ] **Step 7: Test Settings > Categories**

Navigate to `/settings/categories`. Verify the categories page renders correctly.

- [ ] **Step 8: Test old URL redirects**

Navigate to:
- `/coaching` → should redirect to `/brain/coaching`
- `/system` → should redirect to `/brain/contexts`
- `/system/memory` → should redirect to `/brain/memory`
- `/system/categories` → should redirect to `/settings/categories`

- [ ] **Step 9: Test sidebar**

Verify:
- Coaching item is gone
- System item is gone
- Brain item still present and navigates to `/brain`

- [ ] **Step 10: Test GeneralSettings is clean**

Navigate to `/settings/general`. Verify:
- Only System info, Keyboard Shortcuts, and Permissions remain
- No Agent defaults or AutoTuner sections

- [ ] **Step 11: Test PersonalizationSettings is clean**

Navigate to `/settings/personalization`. Verify:
- Only Theme and Provider & Model remain
- No Cognitive AI, Learning, or Provider routing sections

- [ ] **Step 12: Fix any visual issues found**

Address any layout, spacing, or styling issues discovered during testing.

- [ ] **Step 13: Run linter**

Run: `cd desktop-ui && bun run lint:fix`
Expected: clean pass

- [ ] **Step 14: Commit any polish fixes**

```bash
git add -A
git commit -m "fix(desktop-ui): visual polish for brain dashboard"
```

---

## Task 13: Run tests and final check

**Files:**
- No new files

- [ ] **Step 1: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: all tests pass (or existing failures only)

- [ ] **Step 2: Run biome lint**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 3: Run Rust build to ensure desktop crate still compiles**

Run: `cargo build -p desktop`
Expected: success (no Rust changes, but verify Tauri config is still valid)

- [ ] **Step 4: Final commit if any remaining changes**

```bash
git add -A
git commit -m "chore(desktop-ui): final cleanup for brain page consolidation"
```
