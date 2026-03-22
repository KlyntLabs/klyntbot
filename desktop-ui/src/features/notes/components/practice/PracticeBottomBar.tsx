import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { useEffect, useRef, useState } from "react";
import type { PracticeCorrection, PracticeScores } from "../../hooks/usePracticeEvaluation";
import { gradeBgClass, gradeColorClass } from "../../lib/gradeUtils";
import { ConfidenceTap } from "./ConfidenceTap";

function gradeToConfidence(grade: string): number {
  if (grade === "A+" || grade === "A") return 5;
  if (grade === "A-" || grade === "B+") return 4;
  if (grade === "B" || grade === "B-") return 3;
  if (grade === "C+" || grade === "C") return 2;
  return 1;
}

// ── Props ─────────────────────────────────────────────────

export interface PracticeBottomBarProps {
  state: "input" | "eval";
  currentSegmentText: string;
  evaluation?: {
    overallGrade: string;
    scores: PracticeScores;
    corrections: PracticeCorrection[];
    modelTranslation: string;
    encouragement: string;
    improvementHint: string | null;
    coachingNudge: string | null;
  };
  loading?: boolean;
  error?: string | null;
  initialText?: string;
  onSubmit: (userTranslation: string) => void;
  onConfirm: (finalTranslation: string, confidence: number, edited: boolean) => void;
  onEdit: () => void;
}

// ── Input state ───────────────────────────────────────────

function InputBar({
  currentSegmentText,
  loading,
  error,
  initialText,
  onSubmit,
}: {
  currentSegmentText: string;
  loading?: boolean;
  error?: string | null;
  initialText?: string;
  onSubmit: (text: string) => void;
}) {
  const [text, setText] = useState(initialText ?? "");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Reset text when initialText changes (e.g. new segment or edit pre-fill)
  useEffect(() => {
    setText(initialText ?? "");
  }, [initialText]);

  // Auto-focus textarea
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (text.trim() && !loading) {
        onSubmit(text.trim());
      }
    }
  };

  return (
    <div className="flex flex-col gap-1">
      {loading ? (
        <div className="flex items-center gap-2 p-3">
          <span className="text-xs text-muted">Evaluating</span>
          <ThinkingDots size="sm" />
        </div>
      ) : (
        <textarea
          ref={textareaRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type your translation..."
          rows={2}
          className="bg-transparent p-3 text-sm text-primary resize-none placeholder:text-dim focus:outline-none"
        />
      )}
      {error && <p className="text-red-400 text-xs">{error}</p>}
    </div>
  );
}

// ── Eval state ────────────────────────────────────────────

function EvalBar({
  evaluation,
  onConfirm,
  onEdit,
  lastTranslation,
}: {
  evaluation: NonNullable<PracticeBottomBarProps["evaluation"]>;
  onConfirm: (finalTranslation: string, confidence: number, edited: boolean) => void;
  onEdit: () => void;
  lastTranslation: string;
}) {
  const [confidence, setConfidence] = useState(() => gradeToConfidence(evaluation.overallGrade));
  const [modelExpanded, setModelExpanded] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const {
    overallGrade,
    scores,
    corrections,
    modelTranslation,
    encouragement,
    improvementHint,
    coachingNudge,
  } = evaluation;

  const scoreDimensions = [
    { label: "Meaning", value: scores.meaning },
    { label: "Grammar", value: scores.grammar },
    { label: "Natural", value: scores.naturalness },
    { label: "Words", value: scores.wordChoice },
  ];

  // Enter key triggers confirm in eval state
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        onConfirm(lastTranslation, confidence, false);
      }
    };
    const container = containerRef.current;
    if (container) {
      container.addEventListener("keydown", handleKeyDown);
      // Focus the container so keyboard events are captured
      container.focus();
    }
    return () => {
      container?.removeEventListener("keydown", handleKeyDown);
    };
  }, [onConfirm, lastTranslation, confidence]);

  return (
    <div ref={containerRef} className="flex flex-col gap-2 outline-none" tabIndex={-1}>
      {/* Row 1: Grade + dimension scores */}
      <div className="flex items-center gap-2 flex-wrap">
        <span className={`text-2xl font-bold ${gradeColorClass(overallGrade)}`}>
          {overallGrade}
        </span>
        {scoreDimensions.map((dim) => (
          <span
            key={dim.label}
            className={`text-[9px] px-2 py-0.5 rounded-full ${gradeBgClass(overallGrade)} ${gradeColorClass(overallGrade)}`}
          >
            {dim.label}: {dim.value}
          </span>
        ))}
      </div>

      {/* Row 2: Corrections */}
      {corrections.length > 0 && (
        <div className="flex flex-col gap-1">
          {corrections.map((c, i) => (
            <div key={`correction-${i}`} className="text-xs">
              <span className="text-red-400 line-through">{c.original}</span>
              <span className="text-muted mx-1">&rarr;</span>
              <span className="text-green-400">{c.suggested}</span>
              <span className="text-muted text-xs ml-2">{c.explanation}</span>
            </div>
          ))}
        </div>
      )}

      {/* Row 3: Model translation (collapsible) */}
      {modelTranslation && (
        <button
          type="button"
          onClick={() => setModelExpanded((prev) => !prev)}
          className="text-muted text-xs text-left hover:text-primary transition-colors"
        >
          {modelExpanded ? `Model: ${modelTranslation}` : "Show model translation..."}
        </button>
      )}

      {/* Row 4: Encouragement */}
      <p className="text-brand italic text-sm">{encouragement}</p>

      {/* Row 5: Improvement hint */}
      {improvementHint && <p className="text-muted text-xs">{improvementHint}</p>}

      {/* Row 6: Coaching nudge */}
      {coachingNudge && (
        <p className="text-amber-400 text-xs bg-amber-500/10 rounded-md px-2 py-1">
          {coachingNudge}
        </p>
      )}

      {/* Row 7: Confidence tap */}
      <div className="flex items-center gap-2">
        <span className="text-muted text-xs">Confidence:</span>
        <ConfidenceTap value={confidence} onChange={setConfidence} />
      </div>

      {/* Row 8: Action buttons */}
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onEdit}
          className="border border-border text-muted hover:text-primary rounded-lg px-4 py-2 text-sm"
        >
          Edit my translation
        </button>
        <button
          type="button"
          onClick={() => onConfirm(lastTranslation, confidence, false)}
          className="bg-brand text-white rounded-lg px-4 py-2 text-sm font-medium"
        >
          Got it &mdash; Next &crarr;
        </button>
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────

export function PracticeBottomBar({
  state,
  currentSegmentText,
  evaluation,
  loading,
  error,
  initialText,
  onSubmit,
  onConfirm,
  onEdit,
}: PracticeBottomBarProps) {
  // Track the last submitted translation for confirm callback
  const lastTranslationRef = useRef(initialText ?? "");
  useEffect(() => {
    if (initialText) {
      lastTranslationRef.current = initialText;
    }
  }, [initialText]);

  const handleSubmit = (text: string) => {
    lastTranslationRef.current = text;
    onSubmit(text);
  };

  return (
    <div className="border-t border-border px-4 py-2 shrink-0">
      {state === "input" ? (
        <InputBar
          currentSegmentText={currentSegmentText}
          loading={loading}
          error={error}
          initialText={initialText}
          onSubmit={handleSubmit}
        />
      ) : evaluation ? (
        <EvalBar
          evaluation={evaluation}
          onConfirm={onConfirm}
          onEdit={onEdit}
          lastTranslation={lastTranslationRef.current}
        />
      ) : null}
    </div>
  );
}
