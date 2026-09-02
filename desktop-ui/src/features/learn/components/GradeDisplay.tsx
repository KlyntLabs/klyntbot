import { retentionTextColor } from "@shared/lib/retention";
import { Check, Info, Lightbulb, X } from "lucide-react";
import type { GradeResult } from "../hooks/useReviewSession";

interface GradeDisplayProps {
  result: GradeResult;
  userAnswer: string;
}

function gradingMethodLabel(method: string): string {
  switch (method) {
    case "exact_match":
      return "Exact match";
    case "semantic":
      return "Semantic";
    case "llm":
      return "AI graded";
    case "semantic_fallback":
      return "Semantic";
    default:
      return method;
  }
}

function scoreBorderColor(score: number): string {
  if (score >= 0.8) return "border-green-400/50";
  if (score >= 0.5) return "border-amber-400/50";
  return "border-red-400/50";
}

export function GradeDisplay({ result, userAnswer }: GradeDisplayProps) {
  const score = result.score ?? 0;
  const pct = Math.round(score * 100);

  return (
    <div className="w-full max-w-md mx-auto space-y-3 animate-[fade-in-up_0.25s_ease-out]">
      {/* Score + method row */}
      <div className="flex items-center justify-center gap-3">
        <div
          className={`w-14 h-14 rounded-full border-2 ${scoreBorderColor(score)} flex items-center justify-center`}
        >
          <span className={`text-lg font-semibold tabular-nums ${retentionTextColor(score)}`}>
            {pct}%
          </span>
        </div>
        <span className="text-ui-xs text-fg-secondary glass-badge px-2 py-0.5">
          {gradingMethodLabel(result.gradingMethod)}
        </span>
      </div>

      {/* Your answer vs expected */}
      <div className="glass-card p-3 space-y-2 text-sm">
        <div>
          <span className="text-ui-xs text-fg-secondary uppercase tracking-wider">
            Your answer
          </span>
          <p className="text-fg mt-0.5">{userAnswer}</p>
        </div>
        <div className="glass-divider" />
        <div>
          <span className="text-ui-xs text-fg-secondary uppercase tracking-wider">
            Expected
          </span>
          <p className="text-fg mt-0.5">{result.expectedAnswer}</p>
        </div>
      </div>

      {/* Key concepts */}
      {(result.keyConceptsPresent.length > 0 || result.keyConceptsMissing.length > 0) && (
        <div className="flex flex-wrap gap-1.5 justify-center">
          {result.keyConceptsPresent.map((concept) => (
            <span
              key={concept}
              className="inline-flex items-center gap-1 text-ui-xs text-green-400 glass-badge px-2 py-0.5"
            >
              <Check size={10} strokeWidth={2} />
              {concept}
            </span>
          ))}
          {result.keyConceptsMissing.map((concept) => (
            <span
              key={concept}
              className="inline-flex items-center gap-1 text-ui-xs text-red-400 glass-badge px-2 py-0.5"
            >
              <X size={10} strokeWidth={2} />
              {concept}
            </span>
          ))}
        </div>
      )}

      {/* Explanation */}
      {result.explanation && (
        <div className="glass-card p-3 text-sm text-fg">
          <div className="flex items-start gap-2">
            <Info size={14} className="text-fg-secondary mt-0.5 shrink-0" strokeWidth={1.5} />
            <p className="whitespace-pre-wrap">{result.explanation}</p>
          </div>
        </div>
      )}

      {/* Coaching nudge */}
      {result.coachingNudge && (
        <div className="flex items-start gap-2 px-1 text-ui-sm text-fg-secondary">
          <Lightbulb size={13} className="mt-0.5 shrink-0" strokeWidth={1.5} />
          <p>{result.coachingNudge}</p>
        </div>
      )}
    </div>
  );
}
