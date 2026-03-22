import { useEffect, useMemo, useState } from "react";
import {
  gradeBgClass,
  gradeColorClass,
  gradeToNumber,
  isStrongGrade,
  numberToGrade,
} from "../../lib/gradeUtils";

// ── Types ────────────────────────────────────────────────

interface UnitResult {
  index: number;
  finalTranslation: string;
  grade: string;
  scores: { meaning: string; grammar: string; naturalness: string; wordChoice: string };
}

interface PracticeSessionCompleteProps {
  results: UnitResult[];
  totalSegments: number;
  startedAt: Date;
  onSaveToSR: () => void;
  onSaveAsNote: () => void;
  onExit: () => void;
}

type DimensionKey = "meaning" | "grammar" | "naturalness" | "wordChoice";

const DIMENSION_LABELS: Record<DimensionKey, string> = {
  meaning: "Meaning",
  grammar: "Grammar",
  naturalness: "Natural",
  wordChoice: "Choice",
};

// ── Component ────────────────────────────────────────────

export function PracticeSessionComplete({
  results,
  totalSegments,
  startedAt,
  onSaveToSR,
  onSaveAsNote,
  onExit,
}: PracticeSessionCompleteProps) {
  const [showScores, setShowScores] = useState(false);
  const [showTranslation, setShowTranslation] = useState(false);

  // "I did this" moment: show scores after 3 seconds
  useEffect(() => {
    const timer = setTimeout(() => setShowScores(true), 3000);
    return () => clearTimeout(timer);
  }, []);

  // Compute overall percentage
  const overallPercent = useMemo(() => {
    if (results.length === 0) return 0;
    const total = results.reduce((sum, r) => sum + gradeToNumber(r.grade), 0);
    return Math.round(total / results.length);
  }, [results]);

  // Compute per-dimension averages
  const dimensionAverages = useMemo(() => {
    if (results.length === 0) return {} as Record<DimensionKey, string>;
    const dims: DimensionKey[] = ["meaning", "grammar", "naturalness", "wordChoice"];
    const avgs: Record<string, string> = {};
    for (const dim of dims) {
      const total = results.reduce((sum, r) => sum + gradeToNumber(r.scores[dim]), 0);
      avgs[dim] = numberToGrade(Math.round(total / results.length));
    }
    return avgs as Record<DimensionKey, string>;
  }, [results]);

  // Weak units: grade <= A-
  const weakUnits = useMemo(() => {
    return results.filter((r) => gradeToNumber(r.grade) < 95);
  }, [results]);

  // Weakest dimension per weak unit
  const weakUnitDetails = useMemo(() => {
    return weakUnits.map((r) => {
      const dims: DimensionKey[] = ["meaning", "grammar", "naturalness", "wordChoice"];
      let weakest: DimensionKey = "meaning";
      let lowest = 100;
      for (const dim of dims) {
        const val = gradeToNumber(r.scores[dim]);
        if (val < lowest) {
          lowest = val;
          weakest = dim;
        }
      }
      return { index: r.index, weakestDimension: DIMENSION_LABELS[weakest] };
    });
  }, [weakUnits]);

  // Count of units eligible for spaced repetition (grade <= A-)
  const srCardCount = weakUnits.length;

  // Duration in minutes
  const durationMins = Math.max(1, Math.round((Date.now() - startedAt.getTime()) / 60_000));

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8 overflow-auto">
      {/* "I did this" moment */}
      {!showScores && (
        <div className="w-full max-w-2xl space-y-6">
          <p className="text-center text-muted-foreground text-sm animate-fade-in">
            You translated this.
          </p>
          <div className="space-y-3 animate-fade-in">
            {results.map((r) => (
              <p key={r.index} className="text-sm leading-relaxed text-primary">
                {r.finalTranslation}
              </p>
            ))}
          </div>
        </div>
      )}

      {/* Score overlay */}
      {showScores && (
        <div className="w-full max-w-lg space-y-6 animate-fade-in">
          {/* Overall score */}
          <div className="text-center space-y-1">
            <p className="text-4xl font-bold text-brand">{overallPercent}%</p>
            <p className="text-muted-foreground text-sm">
              {results.length}/{totalSegments} units &middot; {durationMins} minute
              {durationMins !== 1 ? "s" : ""}
            </p>
          </div>

          {/* Per-dimension grade cards */}
          <div className="grid grid-cols-4 gap-3">
            {(["meaning", "grammar", "naturalness", "wordChoice"] as DimensionKey[]).map((dim) => {
              const grade = dimensionAverages[dim] ?? "?";
              return (
                <div
                  key={dim}
                  className={`flex flex-col items-center gap-1 rounded-lg border px-3 py-2 ${gradeBgClass(grade)}`}
                >
                  <span className={`text-lg font-bold ${gradeColorClass(grade)}`}>{grade}</span>
                  <span className="text-xs text-muted-foreground">{DIMENSION_LABELS[dim]}</span>
                </div>
              );
            })}
          </div>

          {/* Weak units summary */}
          {weakUnitDetails.length > 0 && (
            <div className="rounded-lg bg-yellow-400/5 border border-yellow-400/15 px-4 py-3 space-y-1">
              <p className="text-sm font-medium text-yellow-300/90">
                {weakUnitDetails.length} unit{weakUnitDetails.length !== 1 ? "s" : ""} need review
              </p>
              <p className="text-xs text-muted-foreground">
                {weakUnitDetails.map((u) => `#${u.index + 1} (${u.weakestDimension})`).join(", ")}
              </p>
            </div>
          )}

          {/* Action buttons */}
          <div className="flex flex-col gap-2 pt-2">
            <button
              type="button"
              onClick={() => setShowTranslation((v) => !v)}
              className="w-full px-4 py-2 rounded-lg border border-border text-sm text-foreground hover:bg-white/5 transition-colors"
            >
              {showTranslation ? "Hide My Full Translation" : "View My Full Translation"}
            </button>

            {showTranslation && (
              <div className="rounded-lg border border-border bg-surface/50 p-4 space-y-2 animate-fade-in">
                {results.map((r) => (
                  <p key={r.index} className="text-sm leading-relaxed text-primary">
                    {r.finalTranslation}
                  </p>
                ))}
              </div>
            )}

            {srCardCount > 0 && (
              <button
                type="button"
                onClick={onSaveToSR}
                className="w-full px-4 py-2.5 rounded-lg bg-brand text-white text-sm font-medium hover:bg-brand-hover transition-colors"
              >
                Save to Spaced Repetition ({srCardCount} card{srCardCount !== 1 ? "s" : ""})
              </button>
            )}

            <button
              type="button"
              onClick={onSaveAsNote}
              className="w-full px-4 py-2 rounded-lg border border-border text-sm text-foreground hover:bg-white/5 transition-colors"
            >
              Save as new note
            </button>

            <button
              type="button"
              onClick={onExit}
              className="text-sm text-muted-foreground hover:text-foreground transition-colors pt-1"
            >
              Close
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
