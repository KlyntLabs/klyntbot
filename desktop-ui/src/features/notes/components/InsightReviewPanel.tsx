import * as PopoverPrimitive from "@radix-ui/react-popover";
import { useCopyToClipboard } from "@shared/hooks/useCopyToClipboard";
import { ipc } from "@shared/hooks/useIpc";
import { cn } from "@shared/lib/utils";
import {
  BookOpen,
  Brain,
  Copy,
  FileInput,
  FilePlus,
  History,
  RefreshCw,
  RotateCcw,
  Sparkles,
  X,
} from "lucide-react";
import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { useInsightEvolution } from "../hooks/useInsightEvolution";
import type {
  InsightReviewActions,
  InsightReviewCachedResponse,
  InsightReviewState,
  TabId,
  TabStatus,
} from "../hooks/useInsightReview";
import { useInsightSSE } from "../hooks/useInsightSSE";
import { useInsightVersions } from "../hooks/useInsightVersions";
import { ChangesBanner } from "./insight/ChangesBanner";
import { ConceptMapTab } from "./insight/ConceptMapTab";
import { FlashcardReview } from "./insight/FlashcardReview";
import { GapAnalysisTab } from "./insight/GapAnalysisTab";
import {
  DEFAULT_SCOPE,
  InsightScopePopover,
  type ScopeConfig,
} from "./insight/InsightScopePopover";
import { InsightVersionList } from "./insight/InsightVersionList";
import { PracticeHistoryTab } from "./insight/PracticeHistoryTab";
import { ScopePreview } from "./insight/ScopePreview";
import { SelfAssessmentTab } from "./insight/SelfAssessmentTab";
import { SynthesisTab } from "./insight/SynthesisTab";

const InsightEvolutionChart = lazy(() =>
  import("./insight/InsightEvolutionChart").then((m) => ({ default: m.InsightEvolutionChart })),
);

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
  { id: "practice", label: "Practice" },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function statusDotClass(status: TabStatus): string {
  switch (status) {
    case "idle":
      return "bg-fg-secondary/40 w-1.5 h-1.5 rounded-full";
    case "loading":
    case "streaming":
      return "bg-purple w-1.5 h-1.5 rounded-full animate-pulse";
    case "done":
      return "bg-status-success w-1.5 h-1.5 rounded-full";
    case "error":
      return "bg-status-danger w-1.5 h-1.5 rounded-full";
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
    case "practice":
      return "done";
  }
}

// ---------------------------------------------------------------------------
// InsightReviewPanel
// ---------------------------------------------------------------------------

export function InsightReviewPanel({ state, actions }: InsightReviewPanelProps) {
  const { copied, copy } = useCopyToClipboard();
  const [showHistory, setShowHistory] = useState(false);
  const [scopeConfig, setScopeConfig] = useState<ScopeConfig>(DEFAULT_SCOPE);
  const [showFlashcardReview, setShowFlashcardReview] = useState(false);
  const evolution = useInsightEvolution();
  const versions = useInsightVersions();
  const activeStatus = tabStatus(state, state.activeTab);
  useInsightSSE(state.isOpen);

  // Lazy-fetch evolution + versions only when History panel is shown
  useEffect(() => {
    if (showHistory && state.isOpen && state.noteId) {
      evolution.fetch(state.noteId);
      versions.fetch(state.noteId);
    }
    if (!state.isOpen) {
      evolution.clear();
    }
  }, [
    showHistory,
    state.isOpen,
    state.noteId,
    evolution.clear,
    evolution.fetch,
    versions.fetch,
    evolution,
    versions,
  ]);

  const handleCreateDeepDiveNote = useCallback(async (title: string, body: string) => {
    try {
      await ipc("note_create", { params: { title, body } });
    } catch {
      // Silently fail — user can create the note manually
    }
  }, []);

  // Get active tab content as text
  const getActiveContent = useCallback((): string => {
    switch (state.activeTab) {
      case "synthesis":
        return state.tabs.synthesis.content;
      case "gaps":
        return state.tabs.gaps.content;
      case "assessment":
        return state.tabs.assessment.questions
          .map((q, i) => `${i + 1}. ${q.question}\n   Answer: ${q.correctAnswer}`)
          .join("\n\n");
      case "concept-map":
        return state.tabs.conceptMap.mermaid || state.tabs.conceptMap.fallbackText;
      case "practice":
        return "";
    }
  }, [state]);

  const hasActiveContent = getActiveContent().length > 0;

  const handleCopy = useCallback(async () => {
    const content = getActiveContent();
    if (content) await copy(content);
  }, [getActiveContent, copy]);

  const handleInsertIntoNote = useCallback(async () => {
    const content = getActiveContent();
    if (!content || !state.noteId) return;
    const date = new Date().toLocaleDateString();
    const section = `\n\n## Insight Review — ${date}\n\n${content}`;
    // Dispatch a custom event that the editor can listen for
    window.dispatchEvent(
      new CustomEvent("insight:insert-into-note", {
        detail: { noteId: state.noteId, content: section },
      }),
    );
  }, [getActiveContent, state.noteId]);

  const handleCreateInsightNote = useCallback(async () => {
    const content = getActiveContent();
    if (!content) return;
    try {
      await ipc("note_create", {
        params: { title: `Insight: ${state.activeTab}`, body: content },
      });
    } catch {
      // Silently fail
    }
  }, [getActiveContent, state.activeTab]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2.5 border-b border-separator shrink-0">
        <Brain size={14} className="text-purple shrink-0" />
        <span className="text-ui-sm font-medium text-fg flex-1">Learn</span>
        <InsightScopePopover value={scopeConfig} onChange={setScopeConfig} />
        <button
          type="button"
          onClick={() => setShowHistory((p) => !p)}
          className={`p-1 rounded-md transition-colors ${
            showHistory
              ? "text-purple bg-purple/10"
              : "text-fg-secondary hover:text-fg hover:bg-control-hover"
          }`}
          title="Version History"
        >
          <History size={12} />
        </button>
        <button
          type="button"
          onClick={() => actions.close()}
          className="p-1 rounded-md text-fg-secondary hover:text-fg hover:bg-control-hover transition-colors"
          aria-label="Close"
        >
          <X size={14} />
        </button>
      </div>

      {/* Scope preview */}
      {state.isOpen && state.noteId && (
        <ScopePreview noteId={state.noteId} scopeConfig={scopeConfig} />
      )}

      {/* What's Changed banner */}
      {state.changesSummary && <ChangesBanner summary={state.changesSummary} />}

      {/* Tab bar */}
      <div className="flex border-b border-separator shrink-0 overflow-x-auto">
        {TABS.map((tab) => {
          const status = tabStatus(state, tab.id);
          const isActive = state.activeTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => actions.switchTab(tab.id)}
              className={`flex items-center gap-1.5 px-3 py-2 text-ui-xs whitespace-nowrap transition-colors border-b-2 ${
                isActive
                  ? "border-purple-400 text-fg"
                  : "border-transparent text-fg-secondary hover:text-fg"
              }`}
            >
              <span className={statusDotClass(status)} />
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* History panel (collapsible) */}
      {showHistory && (
        <div className="border-b border-separator shrink-0 max-h-[300px] overflow-y-auto">
          {evolution.data && evolution.data.versions.length > 0 && (
            <div className="p-3 border-b border-separator">
              <Suspense
                fallback={
                  <div className="flex items-center justify-center h-full text-fg-secondary text-sm">
                    Loading...
                  </div>
                }
              >
                <InsightEvolutionChart versions={evolution.data.versions} />
              </Suspense>
            </div>
          )}
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

      {/* Content area */}
      <div className="flex-1 overflow-y-auto p-4 min-h-0">
        {state.activeTab === "practice" ? (
          <PracticeHistoryTab noteId={state.noteId} />
        ) : activeStatus === "idle" || activeStatus === "error" ? (
          <div className="flex flex-col items-center justify-center h-full gap-3">
            <p className="text-ui-xs text-fg-dim">
              {activeStatus === "error" ? "Generation failed" : "No content generated yet"}
            </p>
            <button
              type="button"
              onClick={() => actions.regenerateTab(state.activeTab)}
              className="flex items-center gap-1.5 text-ui-xs px-3 py-1.5 rounded-md bg-purple/20 text-purple hover:bg-purple/30 transition-colors"
            >
              <Sparkles size={12} />
              Generate {TABS.find((t) => t.id === state.activeTab)?.label}
            </button>
          </div>
        ) : (
          <>
            {state.activeTab === "synthesis" && (
              <SynthesisTab
                status={state.tabs.synthesis.status}
                content={state.tabs.synthesis.content}
                noteId={state.noteId}
                squadId={state.squadId}
              />
            )}
            {state.activeTab === "gaps" && (
              <GapAnalysisTab
                status={state.tabs.gaps.status}
                content={state.tabs.gaps.content}
                noteId={state.noteId}
                squadId={state.squadId}
                onCreateNote={handleCreateDeepDiveNote}
              />
            )}
            {state.activeTab === "assessment" && (
              <SelfAssessmentTab
                status={state.tabs.assessment.status}
                questions={state.tabs.assessment.questions}
                quizState={state.quizState}
                noteId={state.noteId}
                squadId={state.squadId}
                onAnswer={actions.answerQuestion}
                onReveal={actions.revealAnswer}
                onRevealAll={actions.revealAll}
                onSaveFlashcards={actions.saveFlashcards}
              />
            )}
            {state.activeTab === "concept-map" && (
              <ConceptMapTab
                status={state.tabs.conceptMap.status}
                mermaid={state.tabs.conceptMap.mermaid}
                fallbackText={state.tabs.conceptMap.fallbackText}
                noteId={state.noteId}
                squadId={state.squadId}
              />
            )}
          </>
        )}
      </div>

      {/* Footer actions */}
      <div className="flex items-center gap-1.5 px-3 py-2 border-t border-separator shrink-0">
        <button
          type="button"
          onClick={handleInsertIntoNote}
          disabled={!hasActiveContent}
          className="flex items-center gap-1 text-ui-xs px-2 py-1 rounded-md bg-control-hover text-fg-secondary hover:text-fg hover:bg-control-hover/80 transition-colors disabled:text-fg-dim disabled:cursor-not-allowed"
          title="Insert into note"
        >
          <FileInput size={10} />
          Insert
        </button>
        <button
          type="button"
          onClick={handleCreateInsightNote}
          disabled={!hasActiveContent}
          className="flex items-center gap-1 text-ui-xs px-2 py-1 rounded-md bg-control-hover text-fg-secondary hover:text-fg hover:bg-control-hover/80 transition-colors disabled:text-fg-dim disabled:cursor-not-allowed"
          title="Create note from insight"
        >
          <FilePlus size={10} />
          Create note
        </button>
        {state.activeTab === "assessment" &&
          state.tabs.assessment.questions.length > 0 &&
          Object.keys(state.quizState.answers).length >=
            state.tabs.assessment.questions.length * 0.5 && (
            <button
              type="button"
              onClick={() => actions.saveFlashcards(`insight-${Date.now()}`)}
              className="flex items-center gap-1 text-ui-xs px-2 py-1 rounded-md bg-brand/20 text-brand hover:bg-brand/30 transition-colors"
              title="Save as flashcard deck"
            >
              <BookOpen size={10} />
              Save as Deck
            </button>
          )}
        <PopoverPrimitive.Root open={showFlashcardReview} onOpenChange={setShowFlashcardReview}>
          <PopoverPrimitive.Trigger asChild>
            <button
              type="button"
              className="flex items-center gap-1 text-ui-xs px-2 py-1 rounded-md bg-white/[0.04] text-fg-secondary hover:text-fg hover:bg-white/[0.06]"
              title="Review due flashcards"
            >
              <RotateCcw size={10} />
              Review
            </button>
          </PopoverPrimitive.Trigger>
          <PopoverPrimitive.Portal>
            <PopoverPrimitive.Content
              side="left"
              align="end"
              sideOffset={12}
              collisionPadding={16}
              className={cn(
                "z-50 w-[360px] max-h-[min(520px,80vh)] overflow-y-auto rounded-xl border border-separator glass-panel p-0 text-fg shadow-xl outline-none",
                "data-[state=open]:animate-in data-[state=closed]:animate-out",
                "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
                "data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
                "data-[side=left]:slide-in-from-right-2 data-[side=top]:slide-in-from-bottom-2",
              )}
            >
              <FlashcardReview onClose={() => setShowFlashcardReview(false)} />
            </PopoverPrimitive.Content>
          </PopoverPrimitive.Portal>
        </PopoverPrimitive.Root>
        {hasActiveContent && (
          <button
            type="button"
            onClick={() => actions.regenerateTab(state.activeTab)}
            className="flex items-center gap-1 text-ui-xs px-2 py-1 rounded-md bg-control-hover text-fg-secondary hover:text-fg hover:bg-control-hover/80 transition-colors"
            title="Regenerate this tab"
          >
            <RefreshCw size={10} />
          </button>
        )}
        <button
          type="button"
          onClick={handleCopy}
          disabled={!hasActiveContent}
          className="flex items-center gap-1 text-ui-xs px-2 py-1 rounded-md bg-control-hover text-fg-secondary hover:text-fg hover:bg-control-hover/80 transition-colors disabled:text-fg-dim disabled:cursor-not-allowed ml-auto"
          title="Copy to clipboard"
        >
          <Copy size={10} />
          {copied ? "Copied!" : "Copy"}
        </button>
      </div>
      {/* FlashcardReview now renders in a Radix Popover portal above */}
    </div>
  );
}
