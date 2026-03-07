import { Activity, Brain, Cpu, GitBranch, Radio } from "lucide-react";
import { useState } from "react";
import { CoachingTab } from "./tabs/CoachingTab";
import { EventsTab } from "./tabs/EventsTab";
import { MemoryTab } from "./tabs/MemoryTab";
import { PipelineTab } from "./tabs/PipelineTab";
import { SystemTab } from "./tabs/SystemTab";

type DebugTab = "memory" | "coaching" | "events" | "pipeline" | "system";

const tabs: { id: DebugTab; label: string; icon: typeof Brain }[] = [
  { id: "memory", label: "Memory", icon: Brain },
  { id: "coaching", label: "Coaching", icon: Activity },
  { id: "events", label: "Events", icon: Radio },
  { id: "pipeline", label: "Pipeline", icon: GitBranch },
  { id: "system", label: "System", icon: Cpu },
];

export function DebugDashboard() {
  const [activeTab, setActiveTab] = useState<DebugTab>("memory");

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
        {activeTab === "system" && <SystemTab />}
      </div>
    </div>
  );
}
