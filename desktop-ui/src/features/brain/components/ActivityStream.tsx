import { DEV_SSE_BASE, isTauri } from "@shared/lib/utils";
import { ChevronDown, ChevronRight } from "lucide-react";
import { lazy, Suspense, useEffect, useState } from "react";

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
        {open ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
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
            <Suspense
              fallback={<div className="text-sm text-muted-foreground p-4">Loading...</div>}
            >
              {activeTab === "events" && <EventsTab />}
              {activeTab === "pipeline" && <PipelineTab />}
            </Suspense>
          </div>
        </div>
      )}
    </div>
  );
}
