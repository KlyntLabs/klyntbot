import type { Area } from "@shared/types/tasks";
import { PanelRight } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useIssueDetail } from "../../hooks/useIssueDetail";
import type { DisplayProject } from "../../lib/mappers";
import { DecompositionPanel } from "./DecompositionPanel";
import { IssueDetailBreadcrumb } from "./IssueDetailBreadcrumb";
import { IssueDetailSidebar } from "./IssueDetailSidebar";
import { IssueDetailTabs } from "./IssueDetailTabs";
import { IssueDetailTitle } from "./IssueDetailTitle";

interface IssueDetailViewProps {
  issueId: string;
  projectMap: Map<string, DisplayProject>;
  areaMap: Map<string, Area>;
}

export function IssueDetailView({ issueId, projectMap, areaMap }: IssueDetailViewProps) {
  const detail = useIssueDetail(issueId, projectMap, areaMap);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const manualOverrideRef = useRef(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-collapse below 900px, but respect manual override
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width ?? 0;
      if (!manualOverrideRef.current) {
        setSidebarOpen(width >= 900);
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const toggleSidebar = useCallback(() => {
    manualOverrideRef.current = true;
    setSidebarOpen((prev) => !prev);
  }, []);

  return (
    <div ref={containerRef} className="flex h-full relative">
      {/* Left column — splits when decomposition is open */}
      <div className={`flex-1 min-w-0 flex ${detail.decompositionOpen && detail.decompositionResult ? "" : ""}`}>
        {/* Main content */}
        <div className="flex-1 min-w-0 overflow-y-auto px-6 py-4">
          <IssueDetailBreadcrumb />
          <IssueDetailTitle
            title={detail.task.title}
            onUpdate={(title) => detail.updateTask("title", title)}
          />
          <IssueDetailTabs detail={detail} />
        </div>

        {/* Decomposition panel — slides in from right */}
        {detail.decompositionOpen && detail.decompositionResult && (
          <div className="w-[380px] shrink-0 border-l border-[hsl(var(--border))] bg-[hsl(var(--surface-base))]/50">
            <DecompositionPanel
              result={detail.decompositionResult}
              onApply={detail.applyDecomposition}
              onReject={detail.rejectDecomposition}
              applying={detail.decompositionApplying}
            />
          </div>
        )}
      </div>

      {/* Sidebar toggle */}
      {!sidebarOpen && (
        <button
          type="button"
          onClick={toggleSidebar}
          className="absolute top-3 right-3 p-1.5 rounded hover:bg-[hsl(var(--accent))] text-[hsl(var(--muted-foreground))] z-10"
          aria-label="Show sidebar"
        >
          <PanelRight className="size-4" />
        </button>
      )}

      {/* Right column */}
      {sidebarOpen && <IssueDetailSidebar detail={detail} onClose={toggleSidebar} />}
    </div>
  );
}
