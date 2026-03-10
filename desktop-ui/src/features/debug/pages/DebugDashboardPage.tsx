import { Activity, Brain, GitBranch, Radio } from "lucide-react";
import { useEffect, useState } from "react";
import { DEV_SSE_BASE, isTauri } from "@shared/lib/utils";
import { CoachingTab } from "../components/tabs/CoachingTab";
import { EventsTab } from "../components/tabs/EventsTab";
import { MemoryTab } from "../components/tabs/MemoryTab";
import { PipelineTab } from "../components/tabs/PipelineTab";

const COGNITIVE_SSE_EVENTS = [
  "cognitive:domain_event",
  "cognitive:extraction",
  "cognitive:consolidation",
];

type DebugTab = "memory" | "coaching" | "events" | "pipeline";

const tabs: { id: DebugTab; label: string; icon: typeof Brain }[] = [
  { id: "memory", label: "Memory", icon: Brain },
  { id: "coaching", label: "Coaching", icon: Activity },
  { id: "events", label: "Events", icon: Radio },
  { id: "pipeline", label: "Pipeline", icon: GitBranch },
];

export function DebugDashboardPage() {
  const [activeTab, setActiveTab] = useState<DebugTab>("memory");

  // In browser dev mode, connect to cognitive SSE stream and bridge to CustomEvents
  // so useEvent listeners in child tabs receive domain events.
  useEffect(() => {
    if (isTauri) return;

    const es = new EventSource(`${DEV_SSE_BASE}/api/cognitive/stream`);
    for (const eventName of COGNITIVE_SSE_EVENTS) {
      es.addEventListener(eventName, (e: MessageEvent) => {
        try {
          const payload = JSON.parse(e.data);
          window.dispatchEvent(new CustomEvent(eventName, { detail: payload }));
        } catch {
          // skip malformed events
        }
      });
    }
    return () => es.close();
  }, []);

  return (
    <div className="flex-1 min-w-0 flex flex-col gap-2 overflow-hidden">
      {/* Tab Bar — matches FinanceLayout style */}
      <div className="h-12 flex items-center px-2 shrink-0">
        <div className="flex-1 flex items-center gap-1.5" role="tablist">
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
                className={`flex-1 py-2 rounded-xl text-[13px] font-light transition-all duration-200 flex items-center justify-center gap-1.5 ${
                  isActive
                    ? "glass-button-active text-primary"
                    : "text-muted hover:text-secondary hover:bg-white/[0.04]"
                }`}
              >
                <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
                {tab.label}
              </button>
            );
          })}
        </div>
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeTab === "memory" && <MemoryTab />}
        {activeTab === "coaching" && <CoachingTab />}
        {activeTab === "events" && <EventsTab />}
        {activeTab === "pipeline" && <PipelineTab />}
      </div>
    </div>
  );
}
