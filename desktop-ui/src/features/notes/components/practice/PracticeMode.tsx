import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useMemo, useRef, useState } from "react";
import type { PracticeEvalResponse } from "../../hooks/usePracticeEvaluation";
import { usePracticeEvaluation } from "../../hooks/usePracticeEvaluation";
import { usePracticeSession } from "../../hooks/usePracticeSession";
import { useSmartSegmentation } from "../../hooks/useSmartSegmentation";
import { gradeToNumber, isStrongGrade } from "../../lib/gradeUtils";
import { PracticeBottomBar } from "./PracticeBottomBar";
import { PracticeDocPanel } from "./PracticeDocPanel";
import { PracticePreview } from "./PracticePreview";
import { PracticeProgressHeader } from "./PracticeProgressHeader";
import { PracticeSessionComplete } from "./PracticeSessionComplete";
import { PracticeSourcePanel } from "./PracticeSourcePanel";

// ── Types ────────────────────────────────────────────────

type Phase = "loading" | "preview" | "active" | "complete";
type BarState = "input" | "eval";

export interface PracticeUnitResult {
  index: number;
  userTranslation: string;
  evaluation: PracticeEvalResponse;
  confidenceRating: number;
  edited: boolean;
}

interface PracticeModeProps {
  noteId: string;
  sourceText: string;
  sourceLang: string;
  targetLang: string;
  startIndex?: number;
  onExit: () => void;
}

// ── PracticeMode orchestrator ────────────────────────────

export function PracticeMode({
  noteId,
  sourceText: _sourceText,
  sourceLang,
  targetLang,
  startIndex,
  onExit,
}: PracticeModeProps) {
  const [phase, setPhase] = useState<Phase>("loading");
  const [currentIndex, setCurrentIndex] = useState(startIndex ?? 0);
  const [results, setResults] = useState<PracticeUnitResult[]>([]);
  const [barState, setBarState] = useState<BarState>("input");
  const [editText, setEditText] = useState<string | undefined>(undefined);
  const [wasEdited, setWasEdited] = useState(false);
  const [existingSessionInfo, setExistingSessionInfo] = useState<{
    currentIndex: number;
    averageScore?: number;
    savedResults: PracticeUnitResult[];
  } | null>(null);
  const [toastMessage, setToastMessage] = useState<string | null>(null);

  const segmentation = useSmartSegmentation();
  const practiceSession = usePracticeSession();
  const evaluation = usePracticeEvaluation();

  const initRef = useRef(false);
  const sessionStartedAt = useRef<Date>(new Date());

  // On mount: segment note and check for existing session
  useEffect(() => {
    if (initRef.current) return;
    initRef.current = true;

    async function init() {
      // Run segmentation and session lookup in parallel (independent operations)
      const [segResult, existing] = await Promise.all([
        segmentation.segment(noteId, sourceLang, targetLang),
        practiceSession.getSession(noteId),
      ]);

      if (!segResult) {
        setPhase("preview");
        return;
      }

      if (existing && existing.status === "active") {
        // Store session info for the preview resume view
        const savedResults: PracticeUnitResult[] = existing.results
          ? JSON.parse(existing.results)
          : [];
        setExistingSessionInfo({
          currentIndex: existing.currentIndex,
          averageScore: existing.averageScore ?? undefined,
          savedResults,
        });
      }
      setPhase("preview");
    }

    init();
  }, [noteId, sourceLang, targetLang, segmentation.segment, practiceSession.getSession]);

  // Start a new session
  const handleStart = async (fromIndex?: number) => {
    const idx = fromIndex ?? startIndex ?? 0;
    const activeSegments = segmentation.segments.filter((s) => !s.skipped);
    if (activeSegments.length === 0) return;

    const session = await practiceSession.startSession(
      noteId,
      segmentation.segments,
      sourceLang,
      targetLang,
      idx,
    );
    if (session) {
      setCurrentIndex(idx);
      setResults([]);
      setBarState("input");
      setEditText(undefined);
      setWasEdited(false);
      sessionStartedAt.current = new Date();
      setPhase("active");
    }
  };

  // Resume an existing session
  const handleResume = () => {
    if (!existingSessionInfo) return;
    setCurrentIndex(existingSessionInfo.currentIndex);
    setResults(existingSessionInfo.savedResults);
    setBarState("input");
    setEditText(undefined);
    setWasEdited(false);
    sessionStartedAt.current = new Date();
    setPhase("active");
  };

  // Submit a translation for evaluation
  const handleSubmit = async (userTranslation: string) => {
    if (!practiceSession.session) return;

    const evalResult = await evaluation.submitUnit(
      practiceSession.session.id,
      currentIndex,
      userTranslation,
    );
    if (evalResult) {
      // If we had a previous result for this index (edit flow), replace it
      setResults((prev) => {
        const existing = prev.findIndex((r) => r.index === currentIndex);
        const entry: PracticeUnitResult = {
          index: currentIndex,
          userTranslation,
          evaluation: evalResult,
          confidenceRating: 0,
          edited: wasEdited,
        };
        if (existing >= 0) {
          const updated = [...prev];
          updated[existing] = entry;
          return updated;
        }
        return [...prev, entry];
      });
      setEditText(undefined);
      setBarState("eval");
    }
  };

  // Confirm unit and advance
  const handleConfirm = async (finalTranslation: string, confidence: number, edited: boolean) => {
    if (!practiceSession.session) return;

    const isEdited = edited || wasEdited;

    // Update the latest result with confidence and final translation
    setResults((prev) => {
      const updated = [...prev];
      const last = updated[updated.length - 1];
      if (last) {
        last.confidenceRating = confidence;
        last.edited = isEdited;
        last.userTranslation = finalTranslation;
      }
      return updated;
    });

    const evalGrade = evaluation.evaluation?.overallGrade ?? "";
    const evalScores = evaluation.evaluation?.scores
      ? JSON.stringify(evaluation.evaluation.scores)
      : undefined;

    const confirmResult = await practiceSession.confirmUnit(
      practiceSession.session.id,
      currentIndex,
      finalTranslation,
      confidence,
      isEdited,
      evalGrade,
      evalScores,
    );

    if (confirmResult) {
      // Show micro-toast for non-perfect grades (saved to knowledge graph)
      const lastEval = evaluation.evaluation;
      if (lastEval) {
        const grade = lastEval.overallGrade;
        if (!isStrongGrade(grade)) {
          setToastMessage("Atom saved \u00b7 This unit now lives in your knowledge graph");
          setTimeout(() => setToastMessage(null), 1500);
        }
      }

      if (confirmResult.isComplete) {
        setPhase("complete");
      } else {
        setCurrentIndex(confirmResult.nextIndex);
        setBarState("input");
        setEditText(undefined);
        setWasEdited(false);
        evaluation.reset();
      }
    }
  };

  // Edit: go back to input with pre-filled text
  const handleEdit = () => {
    const lastResult = results.find((r) => r.index === currentIndex);
    setEditText(lastResult?.userTranslation ?? "");
    setWasEdited(true);
    setBarState("input");
  };

  // ── Render by phase ────────────────────────────────────

  const activeSegments = segmentation.segments.filter((s) => !s.skipped);

  const completedIndices = useMemo(() => new Set(results.map((r) => r.index)), [results]);

  const averageScore = useMemo(() => {
    if (results.length === 0) return undefined;
    const total = results.reduce((sum, r) => sum + gradeToNumber(r.evaluation.overallGrade), 0);
    return total / results.length;
  }, [results]);

  const currentSuggestedFocus = activeSegments[currentIndex]?.suggestedFocus ?? "accuracy";

  const docPanelResults = useMemo(
    () =>
      results.map((r) => ({
        index: r.index,
        finalTranslation: r.edited
          ? r.userTranslation
          : r.evaluation.modelTranslation || r.userTranslation,
        grade: r.evaluation.overallGrade,
      })),
    [results],
  );

  const sourceSegments = useMemo(
    () =>
      segmentation.segments.map((s) => ({
        index: s.index,
        text: s.text,
        type: s.segmentType,
        suggestedFocus: s.suggestedFocus,
      })),
    [segmentation.segments],
  );

  if (phase === "loading") {
    return (
      <div className="flex-1 flex items-center justify-center">
        <span className="text-sm text-fg-secondary">Loading practice session...</span>
      </div>
    );
  }

  if (phase === "preview") {
    const previewSegments = activeSegments.map((s) => ({
      index: s.index,
      text: s.text,
      type: s.segmentType,
      suggestedFocus: s.suggestedFocus,
    }));
    return (
      <PracticePreview
        segments={previewSegments}
        estimatedMins={segmentation.estimatedMins}
        existingSession={
          existingSessionInfo
            ? {
                currentIndex: existingSessionInfo.currentIndex,
                averageScore: existingSessionInfo.averageScore,
              }
            : null
        }
        onStart={() => handleStart()}
        onResume={handleResume}
        onCancel={onExit}
      />
    );
  }

  if (phase === "complete") {
    const completeResults = results.map((r) => ({
      index: r.index,
      finalTranslation: r.edited
        ? r.userTranslation
        : r.evaluation.modelTranslation || r.userTranslation,
      grade: r.evaluation.overallGrade,
      scores: r.evaluation.scores,
    }));

    const handleSaveToSR = async () => {
      if (!practiceSession.session) return;
      await practiceSession.completeSession(practiceSession.session.id, true);
    };

    const handleSaveAsNote = async () => {
      const lines = completeResults.map((r) => `**[${r.grade}]** ${r.finalTranslation}`);
      const body = `# Practice Translation (${sourceLang} → ${targetLang})\n\n${lines.join("\n\n")}`;
      const title = `Practice: ${sourceLang}→${targetLang} ${new Date().toLocaleDateString()}`;
      try {
        await ipc("note_create", { params: { title, body } });
      } catch (e) {
        console.error("Failed to save practice as note:", e);
      }
    };

    return (
      <PracticeSessionComplete
        results={completeResults}
        totalSegments={activeSegments.length}
        startedAt={sessionStartedAt.current}
        onSaveToSR={handleSaveToSR}
        onSaveAsNote={handleSaveAsNote}
        onExit={onExit}
      />
    );
  }

  // phase === "active"
  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <PracticeProgressHeader
        currentIndex={currentIndex}
        totalSegments={activeSegments.length}
        suggestedFocus={currentSuggestedFocus}
        averageScore={averageScore}
        onExit={onExit}
      />

      {/* Split panes */}
      <div className="flex flex-1 min-h-0 gap-px">
        <div className="flex-1 overflow-auto">
          <PracticeSourcePanel
            segments={sourceSegments}
            currentIndex={currentIndex}
            completedIndices={completedIndices}
          />
        </div>
        <div className="w-px bg-border shrink-0" />
        <div className="flex-1 overflow-auto">
          <PracticeDocPanel
            results={docPanelResults}
            currentIndex={currentIndex}
            totalSegments={activeSegments.length}
          />
        </div>
      </div>

      {/* Bottom bar */}
      <PracticeBottomBar
        state={barState}
        currentSegmentText={activeSegments[currentIndex]?.text ?? ""}
        evaluation={evaluation.evaluation ?? undefined}
        loading={evaluation.evaluating}
        error={evaluation.error}
        initialText={editText}
        onSubmit={handleSubmit}
        onConfirm={handleConfirm}
        onEdit={handleEdit}
      />

      {/* Micro-toast */}
      {toastMessage && (
        <div className="fixed bottom-4 right-4 bg-bg-elevated border border-separator rounded-lg px-4 py-2 text-ui-sm text-fg-secondary shadow-lg z-50 animate-fade-in">
          {toastMessage}
        </div>
      )}
    </div>
  );
}
