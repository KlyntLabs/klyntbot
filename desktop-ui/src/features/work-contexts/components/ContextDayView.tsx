import { useSidebarOpen } from "@features/dashboard/lib/layers";
import type { ContextTimelineBlock, WorkContext, WorkContextDetail } from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useContextTimeline } from "../hooks/useContextTimeline";
import { useWorkContextDetail, useWorkContexts } from "../hooks/useWorkContexts";
import { ContextDetailPanel } from "./ContextDetailPanel";
import { ContextSearchDialog } from "./ContextSearchDialog";
import { ContextSidebar } from "./ContextSidebar";
import { ContextTimeline } from "./ContextTimeline";

const DEFAULT_HOUR_HEIGHT = 60;
const HOUR_GUTTER = 48;
const HOURS = Array.from({ length: 24 }, (_, i) => i);

interface ContextDayViewProps {
  date: string;
  isToday: boolean;
}

export function ContextDayView({ date, isToday }: ContextDayViewProps) {
  const { data: blocks } = useContextTimeline(date);
  const { data: contexts } = useWorkContexts();
  const sidebarOpen = useSidebarOpen();

  const [selectedContextId, setSelectedContextId] = useState<string | null>(null);
  const { data: detail } = useWorkContextDetail(selectedContextId);
  const [panelOpen, setPanelOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);

  const scrollRef = useRef<HTMLDivElement>(null);
  const hourHeight = DEFAULT_HOUR_HEIGHT;
  const totalHeight = 24 * hourHeight;

  const dayStart = useMemo(() => new Date(`${date}T00:00:00`), [date]);

  // Scroll to current hour on mount
  useEffect(() => {
    if (scrollRef.current) {
      const targetHour = isToday ? new Date().getHours() - 1 : 8;
      scrollRef.current.scrollTop = Math.max(0, targetHour * hourHeight);
    }
  }, [isToday, hourHeight]);

  const handleBlockClick = useCallback((block: ContextTimelineBlock) => {
    if (block.contextId) {
      setSelectedContextId(block.contextId);
      setPanelOpen(true);
    }
  }, []);

  const handleContextSelect = useCallback((ctx: WorkContext) => {
    setSelectedContextId(ctx.id);
    setPanelOpen(true);
  }, []);

  return (
    <div className="flex gap-2 h-full">
      <div className="flex-1 glass-card overflow-hidden flex flex-col">
        <div ref={scrollRef} className="flex-1 overflow-y-auto">
          {/* Header */}
          <div className="sticky top-0 z-20 border-b border-border bg-surface-floating px-4 py-1.5">
            <span className="text-[11px] text-muted-foreground font-medium">Work Contexts</span>
          </div>

          <div className="relative" style={{ height: totalHeight }}>
            {/* Hour lines */}
            {HOURS.map((h) => (
              <div
                key={h}
                className="absolute w-full flex items-start"
                style={{ top: h * hourHeight }}
              >
                <div
                  className="text-[10px] text-muted-foreground text-right pr-2 select-none"
                  style={{ width: HOUR_GUTTER }}
                >
                  {h === 0 ? "" : formatHour(h)}
                </div>
                <div className="flex-1 border-t border-border" />
              </div>
            ))}

            {/* Context blocks */}
            <div className="absolute inset-0" style={{ left: HOUR_GUTTER }}>
              <ContextTimeline
                blocks={blocks}
                hourHeight={hourHeight}
                dayStart={dayStart}
                onBlockClick={handleBlockClick}
              />
            </div>
          </div>
        </div>
      </div>

      {/* Context sidebar */}
      {sidebarOpen && (
        <div className="w-72 glass-card overflow-y-auto p-3">
          <ContextSidebar
            contexts={contexts}
            selectedId={selectedContextId ?? undefined}
            onSelect={handleContextSelect}
            onSearchClick={() => setSearchOpen(true)}
          />
        </div>
      )}

      {/* Detail panel */}
      <ContextDetailPanel
        open={panelOpen}
        onClose={() => setPanelOpen(false)}
        detail={detail as WorkContextDetail | null}
      />

      {/* Search dialog */}
      <ContextSearchDialog
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        onSelect={handleContextSelect}
      />
    </div>
  );
}

function formatHour(h: number): string {
  if (h === 0) return "12 AM";
  if (h < 12) return `${h} AM`;
  if (h === 12) return "12 PM";
  return `${h - 12} PM`;
}
