import { DEV_SSE_BASE, isTauri } from "@shared/lib/utils";
import { Activity, Boxes, Brain, Cpu, GitBranch, Grid3x3, Radio } from "lucide-react";
import { lazy, Suspense, useEffect, useState } from "react";
import { useParams } from "react-router";

const ContextsTab = lazy(() =>
  import("../components/tabs/ContextsTab").then((m) => ({
    default: m.ContextsTab,
  })),
);
const CategoriesTab = lazy(() =>
  import("../components/tabs/CategoriesTab").then((m) => ({
    default: m.CategoriesTab,
  })),
);
const InferenceTab = lazy(() =>
  import("../components/tabs/InferenceTab").then((m) => ({
    default: m.InferenceTab,
  })),
);
const MemoryTab = lazy(() =>
  import("@features/debug/components/tabs/MemoryTab").then((m) => ({
    default: m.MemoryTab,
  })),
);
const CoachingTab = lazy(() =>
  import("@features/debug/components/tabs/CoachingTab").then((m) => ({
    default: m.CoachingTab,
  })),
);
const EventsTab = lazy(() =>
  import("@features/debug/components/tabs/EventsTab").then((m) => ({
    default: m.EventsTab,
  })),
);
const PipelineTab = lazy(() =>
  import("@features/debug/components/tabs/PipelineTab").then((m) => ({
    default: m.PipelineTab,
  })),
);

const COGNITIVE_SSE_EVENTS = [
  "cognitive:domain_event",
  "cognitive:extraction",
  "cognitive:consolidation",
];

type SystemTab =
  | "contexts"
  | "categories"
  | "inference"
  | "memory"
  | "coaching"
  | "events"
  | "pipeline";

const tabs: { id: SystemTab; label: string; icon: typeof Brain }[] = [
  { id: "contexts", label: "Contexts", icon: Boxes },
  { id: "categories", label: "Categories", icon: Grid3x3 },
  { id: "inference", label: "Inference", icon: Cpu },
  { id: "memory", label: "Memory", icon: Brain },
  { id: "coaching", label: "Coaching", icon: Activity },
  { id: "events", label: "Events", icon: Radio },
  { id: "pipeline", label: "Pipeline", icon: GitBranch },
];

function isValidTab(t: string | undefined): t is SystemTab {
  return tabs.some((tab) => tab.id === t);
}

export function SystemPage() {
  const { tab: urlTab } = useParams<{ tab?: string }>();
  const [activeTab, setActiveTab] = useState<SystemTab>(isValidTab(urlTab) ? urlTab : "contexts");

  // Sync with URL param changes
  useEffect(() => {
    if (isValidTab(urlTab) && urlTab !== activeTab) {
      setActiveTab(urlTab);
    }
  }, [urlTab]);

  // Bridge cognitive SSE events in browser dev mode (same as DebugDashboardPage)
  useEffect(() => {
    if (isTauri) return;
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
  }, []);

  return (
    <div className="flex-1 min-w-0 flex flex-col gap-2 overflow-hidden">
      {/* Tab Bar */}
      <div className="h-12 flex items-center px-2 shrink-0">
        <div className="flex-1 flex items-center gap-1.5 overflow-x-auto" role="tablist">
          {tabs.map((tab) => {
            const Icon = tab.icon;
            const isActive = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={isActive}
                onClick={() => setActiveTab(tab.id)}
                className={`flex-1 min-w-0 py-2 rounded-xl text-[13px] font-light transition-all duration-200 flex items-center justify-center gap-1.5 ${
                  isActive
                    ? "glass-button-active text-foreground"
                    : "text-muted-foreground hover:text-foreground hover:bg-card"
                }`}
              >
                <Icon className="w-3.5 h-3.5 shrink-0" strokeWidth={1.5} />
                <span className="truncate">{tab.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-hidden">
        <Suspense
          fallback={
            <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
              Loading...
            </div>
          }
        >
          {activeTab === "contexts" && <ContextsTab />}
          {activeTab === "categories" && <CategoriesTab />}
          {activeTab === "inference" && <InferenceTab />}
          {activeTab === "memory" && <MemoryTab />}
          {activeTab === "coaching" && <CoachingTab />}
          {activeTab === "events" && <EventsTab />}
          {activeTab === "pipeline" && <PipelineTab />}
        </Suspense>
      </div>
    </div>
  );
}
