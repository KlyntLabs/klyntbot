import { Brain, RefreshCw, X } from "lucide-react";
import type {
  InsightReviewActions,
  InsightReviewState,
  TabId,
  TabStatus,
} from "../hooks/useInsightReview";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface InsightReviewPanelProps {
  state: InsightReviewState;
  actions: InsightReviewActions;
}

// ---------------------------------------------------------------------------
// Tab definitions
// ---------------------------------------------------------------------------

const TABS: { id: TabId; label: string }[] = [
  { id: "synthesis", label: "Synthesis" },
  { id: "gaps", label: "Gap Analysis" },
  { id: "assessment", label: "Self-Assessment" },
  { id: "concept-map", label: "Concept Map" },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function statusDotClass(status: TabStatus): string {
  switch (status) {
    case "idle":
      return "bg-white/10 w-1.5 h-1.5 rounded-full";
    case "loading":
    case "streaming":
      return "bg-purple-400 w-1.5 h-1.5 rounded-full animate-pulse";
    case "done":
      return "bg-green-400 w-1.5 h-1.5 rounded-full";
    case "error":
      return "bg-red-400 w-1.5 h-1.5 rounded-full";
  }
}

function tabStatus(state: InsightReviewState, tabId: TabId): TabStatus {
  switch (tabId) {
    case "synthesis":
      return state.tabs.synthesis.status;
    case "gaps":
      return state.tabs.gaps.status;
    case "assessment":
      return state.tabs.assessment.status;
    case "concept-map":
      return state.tabs.conceptMap.status;
  }
}

// ---------------------------------------------------------------------------
// SkeletonLoader
// ---------------------------------------------------------------------------

function SkeletonLoader() {
  return (
    <div className="space-y-3 animate-pulse">
      <div className="h-3 bg-white/[0.04] rounded w-3/4" />
      <div className="h-3 bg-white/[0.04] rounded w-full" />
      <div className="h-3 bg-white/[0.04] rounded w-5/6" />
    </div>
  );
}

// ---------------------------------------------------------------------------
// InsightReviewPanel
// ---------------------------------------------------------------------------

export function InsightReviewPanel({ state, actions }: InsightReviewPanelProps) {
  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2.5 border-b border-border shrink-0">
        <Brain size={14} className="text-purple-400 shrink-0" />
        <span className="text-[12px] font-medium text-primary flex-1">Insight Review</span>
        <button
          type="button"
          disabled
          className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-white/[0.04] text-dim cursor-not-allowed"
          title="Regenerate all tabs"
        >
          <RefreshCw size={10} />
          Regenerate All
        </button>
        <button
          type="button"
          onClick={() => actions.close()}
          className="p-1 rounded-md text-muted hover:text-primary hover:bg-white/[0.06] transition-colors"
          aria-label="Close Insight Review"
        >
          <X size={14} />
        </button>
      </div>

      {/* Tab bar */}
      <div className="flex border-b border-border shrink-0 overflow-x-auto">
        {TABS.map((tab) => {
          const status = tabStatus(state, tab.id);
          const isActive = state.activeTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => actions.switchTab(tab.id)}
              className={`flex items-center gap-1.5 px-3 py-2 text-[11px] whitespace-nowrap transition-colors border-b-2 ${
                isActive
                  ? "border-purple-400 text-primary"
                  : "border-transparent text-muted hover:text-secondary"
              }`}
            >
              <span className={statusDotClass(status)} />
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* Content area */}
      <div className="flex-1 overflow-y-auto p-4">
        {state.activeTab === "synthesis" && (
          <div className="text-[11px] text-muted">
            {state.tabs.synthesis.status === "loading" && <SkeletonLoader />}
            {state.tabs.synthesis.status === "streaming" && (
              <p className="leading-relaxed whitespace-pre-wrap">{state.tabs.synthesis.content}</p>
            )}
            {state.tabs.synthesis.status === "done" && (
              <p className="leading-relaxed whitespace-pre-wrap">{state.tabs.synthesis.content}</p>
            )}
            {state.tabs.synthesis.status === "idle" && (
              <p className="text-dim italic">Click to start analysis</p>
            )}
            {state.tabs.synthesis.status === "error" && (
              <p className="text-red-400/70 italic">Failed to generate synthesis.</p>
            )}
          </div>
        )}
        {state.activeTab === "gaps" && (
          <div className="text-[11px] text-muted">
            {state.tabs.gaps.status === "loading" && <SkeletonLoader />}
            {state.tabs.gaps.status === "done" ? (
              <p className="leading-relaxed whitespace-pre-wrap">{state.tabs.gaps.content}</p>
            ) : state.tabs.gaps.status === "idle" ? (
              <p className="text-dim italic">Loading gap analysis...</p>
            ) : state.tabs.gaps.status === "error" ? (
              <p className="text-red-400/70 italic">Failed to generate gap analysis.</p>
            ) : null}
          </div>
        )}
        {state.activeTab === "assessment" && (
          <div className="text-[11px] text-muted">
            {state.tabs.assessment.status === "loading" && <SkeletonLoader />}
            {state.tabs.assessment.status === "done" &&
              (state.tabs.assessment.questions.length > 0 ? (
                <p>{state.tabs.assessment.questions.length} questions</p>
              ) : (
                <p className="text-dim italic">No questions generated.</p>
              ))}
            {(state.tabs.assessment.status === "idle" ||
              state.tabs.assessment.status === "streaming") && (
              <p className="text-dim italic">Loading quiz...</p>
            )}
            {state.tabs.assessment.status === "error" && (
              <p className="text-red-400/70 italic">Failed to generate quiz.</p>
            )}
          </div>
        )}
        {state.activeTab === "concept-map" && (
          <div className="text-[11px] text-muted">
            {state.tabs.conceptMap.status === "loading" && <SkeletonLoader />}
            {state.tabs.conceptMap.status === "done" &&
              (state.tabs.conceptMap.mermaid ? (
                <p>Concept map ready</p>
              ) : state.tabs.conceptMap.fallbackText ? (
                <p className="leading-relaxed whitespace-pre-wrap">
                  {state.tabs.conceptMap.fallbackText}
                </p>
              ) : (
                <p className="text-dim italic">No concept map data.</p>
              ))}
            {(state.tabs.conceptMap.status === "idle" ||
              state.tabs.conceptMap.status === "streaming") && (
              <p className="text-dim italic">Loading concept map...</p>
            )}
            {state.tabs.conceptMap.status === "error" && (
              <p className="text-red-400/70 italic">Failed to generate concept map.</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
