import type { GradeResult } from "@shared/types/notes";
import { PropagationRipple } from "./PropagationRipple";

interface GradeDisplayProps {
  result: GradeResult;
  propagationCount?: number;
}

function scoreBadgeClass(score: number | null): string {
  if (score == null) return "bg-white/[0.06] text-muted-foreground";
  if (score >= 0.85) return "bg-green-500/20 text-green-400";
  if (score >= 0.6) return "bg-yellow-500/20 text-yellow-400";
  if (score >= 0.3) return "bg-orange-500/20 text-orange-400";
  return "bg-red-500/20 text-red-400";
}

function scoreLabel(score: number | null): string {
  if (score == null) return "—";
  return `${Math.round(score * 100)}%`;
}

export function GradeDisplay({ result, propagationCount = 0 }: GradeDisplayProps) {
  const { score, diffHighlights, expectedAnswer, explanation, socraticSuggestion } = result;

  return (
    <div className="flex flex-col gap-2">
      {/* Score badge */}
      <div className="flex items-center gap-2">
        <span
          className={`text-[11px] font-semibold px-2 py-0.5 rounded-full ${scoreBadgeClass(score)}`}
        >
          {scoreLabel(score)}
        </span>
        {result.suggestedRating && (
          <span className="text-[9px] text-dim capitalize">
            Suggested: {result.suggestedRating}
          </span>
        )}
      </div>

      {/* Diff highlights */}
      {diffHighlights.length > 0 && (
        <div className="rounded-lg bg-white/[0.03] p-2.5 flex flex-wrap gap-1">
          {diffHighlights.map((chunk, i) => {
            let cls = "text-[11px]";
            if (chunk.status === "match") cls += " text-green-400";
            else if (chunk.status === "missing") cls += " text-red-400 line-through";
            else if (chunk.status === "partial") cls += " text-yellow-400";
            else cls += " text-dim";
            // biome-ignore lint/suspicious/noArrayIndexKey: diff highlights are positional, no stable id
            return (
              <span key={`diff-${i}`} className={cls}>
                {chunk.text}
              </span>
            );
          })}
        </div>
      )}

      {/* Expected answer */}
      <div className="rounded-lg bg-white/[0.03] p-2.5">
        <p className="text-[9px] text-dim mb-1">Expected answer</p>
        <p className="text-[11px] text-foreground whitespace-pre-wrap">{expectedAnswer}</p>
      </div>

      {/* Explanation */}
      {explanation && <p className="text-[10px] text-muted-foreground">{explanation}</p>}

      {/* Socratic suggestion (collapsible) */}
      {socraticSuggestion && (
        <details className="text-[10px]">
          <summary className="cursor-pointer text-accent hover:text-accent/80 select-none">
            Explore deeper…
          </summary>
          <p className="mt-1.5 text-muted-foreground leading-relaxed">{socraticSuggestion}</p>
        </details>
      )}

      {/* Propagation ripple */}
      <PropagationRipple count={propagationCount} />
    </div>
  );
}
