import { ipc } from "@shared/hooks/useIpc";
import {
  BookOpen,
  Brain,
  Copy,
  FileInput,
  FilePlus,
  History,
  RefreshCw,
  Settings2,
  Sliders,
  X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
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
import { usePersonas } from "../hooks/usePersonas";
import { ConceptMapTab } from "./insight/ConceptMapTab";
import { GapAnalysisTab } from "./insight/GapAnalysisTab";
import { InsightEvolutionChart } from "./insight/InsightEvolutionChart";
import {
  DEFAULT_SCOPE,
  InsightScopePopover,
  type ScopeConfig,
} from "./insight/InsightScopePopover";
import { InsightVersionList } from "./insight/InsightVersionList";
import { ManagePersonasModal } from "./insight/ManagePersonasModal";
import { PerspectivesTab } from "./insight/PerspectivesTab";
import { SelfAssessmentTab } from "./insight/SelfAssessmentTab";
import { SynthesisTab } from "./insight/SynthesisTab";

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
  { id: "perspectives", label: "Perspectives" },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function statusDotClass(status: TabStatus): string {
  switch (status) {
    case "idle":
      return "bg-muted-foreground/40 w-1.5 h-1.5 rounded-full";
    case "loading":
    case "streaming":
      return "bg-purple w-1.5 h-1.5 rounded-full animate-pulse";
    case "done":
      return "bg-success w-1.5 h-1.5 rounded-full";
    case "error":
      return "bg-destructive w-1.5 h-1.5 rounded-full";
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
    case "perspectives":
      return state.tabs.perspectives.status;
  }
}

// ---------------------------------------------------------------------------
// InsightReviewPanel
// ---------------------------------------------------------------------------

export function InsightReviewPanel({ state, actions }: InsightReviewPanelProps) {
  const [copied, setCopied] = useState(false);
  const [showPersonaManager, setShowPersonaManager] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [showScope, setShowScope] = useState(false);
  const [scopeConfig, setScopeConfig] = useState<ScopeConfig>(DEFAULT_SCOPE);
  const [allPersonas, personaActions] = usePersonas();
  const evolution = useInsightEvolution();
  const versions = useInsightVersions();
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showHistory, state.isOpen, state.noteId]);

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
      case "perspectives":
        return state.tabs.perspectives.content;
    }
  }, [state]);

  const hasActiveContent = getActiveContent().length > 0;

  const handleCopy = useCallback(async () => {
    const content = getActiveContent();
    if (!content) return;
    await navigator.clipboard.writeText(content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [getActiveContent]);

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
      <div className="flex items-center gap-2 px-3 py-2.5 border-b border-border shrink-0">
        <Brain size={14} className="text-purple shrink-0" />
        <span className="text-[12px] font-medium text-foreground flex-1">Insight Review</span>
        <div className="relative">
          <button
            type="button"
            onClick={() => setShowScope((p) => !p)}
            className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            title="Scope Config"
          >
            <Sliders size={12} />
          </button>
          {showScope && (
            <InsightScopePopover
              value={scopeConfig}
              onChange={setScopeConfig}
              onClose={() => setShowScope(false)}
            />
          )}
        </div>
        <button
          type="button"
          onClick={() => setShowHistory((p) => !p)}
          className={`p-1 rounded-md transition-colors ${
            showHistory
              ? "text-purple bg-purple/10"
              : "text-muted-foreground hover:text-foreground hover:bg-accent"
          }`}
          title="Version History"
        >
          <History size={12} />
        </button>
        <button
          type="button"
          onClick={() => setShowPersonaManager(true)}
          className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
          title="Manage Personas"
        >
          <Settings2 size={12} />
        </button>
        <button
          type="button"
          onClick={() => {
            if (state.noteId) {
              void actions.open(state.noteId, {
                scopeType: scopeConfig.scopeType,
                radius: scopeConfig.radius,
                includeCognitive: scopeConfig.includeCognitive,
                deepDive: scopeConfig.deepDive,
              });
            }
          }}
          disabled={!state.noteId}
          className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-accent text-muted-foreground hover:text-foreground hover:bg-accent/80 transition-colors disabled:text-dim disabled:cursor-not-allowed"
          title="Regenerate all tabs with current scope"
        >
          <RefreshCw size={10} />
          Regenerate
        </button>
        <button
          type="button"
          onClick={() => actions.close()}
          className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
          aria-label="Close Insight Review"
        >
          <X size={14} />
        </button>
      </div>

      {/* Scope coverage hint */}
      {state.isOpen && (
        <div className="px-3 py-1.5 border-b border-border text-[10px] text-dim flex items-center gap-1">
          <span>Scope:</span>
          <span className="text-muted-foreground capitalize">{scopeConfig.scopeType}</span>
          {scopeConfig.deepDive && <span className="text-purple text-[9px] ml-1">(deep dive)</span>}
          {evolution.data && (
            <span className="ml-auto">
              {evolution.data.versions.length} version
              {evolution.data.versions.length !== 1 ? "s" : ""}
            </span>
          )}
        </div>
      )}

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
                  ? "border-purple-400 text-foreground"
                  : "border-transparent text-muted-foreground hover:text-foreground"
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
        <div className="border-b border-border shrink-0 max-h-[300px] overflow-y-auto">
          {evolution.data && evolution.data.versions.length > 0 && (
            <div className="p-3 border-b border-border">
              <InsightEvolutionChart versions={evolution.data.versions} />
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
        {state.activeTab === "synthesis" && (
          <SynthesisTab
            status={state.tabs.synthesis.status}
            content={state.tabs.synthesis.content}
          />
        )}
        {state.activeTab === "gaps" && (
          <GapAnalysisTab
            status={state.tabs.gaps.status}
            content={state.tabs.gaps.content}
            onCreateNote={handleCreateDeepDiveNote}
          />
        )}
        {state.activeTab === "assessment" && (
          <SelfAssessmentTab
            status={state.tabs.assessment.status}
            questions={state.tabs.assessment.questions}
            quizState={state.quizState}
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
          />
        )}
        {state.activeTab === "perspectives" && (
          <PerspectivesTab
            status={state.tabs.perspectives.status}
            content={state.tabs.perspectives.content}
            personas={state.tabs.perspectives.personas}
          />
        )}
      </div>

      {/* Footer actions */}
      <div className="flex items-center gap-1.5 px-3 py-2 border-t border-border shrink-0">
        <button
          type="button"
          onClick={handleInsertIntoNote}
          disabled={!hasActiveContent}
          className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-accent text-muted-foreground hover:text-foreground hover:bg-accent/80 transition-colors disabled:text-dim disabled:cursor-not-allowed"
          title="Insert into note"
        >
          <FileInput size={10} />
          Insert
        </button>
        <button
          type="button"
          onClick={handleCreateInsightNote}
          disabled={!hasActiveContent}
          className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-accent text-muted-foreground hover:text-foreground hover:bg-accent/80 transition-colors disabled:text-dim disabled:cursor-not-allowed"
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
              className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-brand/20 text-brand hover:bg-brand/30 transition-colors"
              title="Save as flashcard deck"
            >
              <BookOpen size={10} />
              Save as Deck
            </button>
          )}
        <button
          type="button"
          onClick={handleCopy}
          disabled={!hasActiveContent}
          className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-accent text-muted-foreground hover:text-foreground hover:bg-accent/80 transition-colors disabled:text-dim disabled:cursor-not-allowed ml-auto"
          title="Copy to clipboard"
        >
          <Copy size={10} />
          {copied ? "Copied!" : "Copy"}
        </button>
      </div>
      {showPersonaManager && (
        <ManagePersonasModal
          personas={allPersonas}
          actions={personaActions}
          onClose={() => setShowPersonaManager(false)}
        />
      )}
    </div>
  );
}
