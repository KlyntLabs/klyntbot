import { useState } from "react";
import type { KnowledgeAtomResponse } from "../hooks/useKnowledgeAtoms";
import { InlineReview } from "./InlineReview";

interface AtomCardProps {
  atom: KnowledgeAtomResponse;
  onAccept?: (atomId: string) => void;
  onDismiss?: (atomId: string) => void;
  onReviewDone?: () => void;
}

function retentionColor(pct: number): string {
  if (pct >= 0.8) return "text-green-400";
  if (pct >= 0.5) return "text-amber-400";
  return "text-red-400";
}

function retentionBg(pct: number): string {
  if (pct >= 0.8) return "bg-green-500/15";
  if (pct >= 0.5) return "bg-amber-500/15";
  return "bg-red-500/15";
}

export function AtomCard({ atom, onAccept, onDismiss, onReviewDone }: AtomCardProps) {
  const [isReviewing, setIsReviewing] = useState(false);

  if (isReviewing) {
    return (
      <InlineReview
        atomId={atom.id}
        onDone={() => {
          setIsReviewing(false);
          onReviewDone?.();
        }}
      />
    );
  }

  const isSuggested = atom.status === "suggested";

  return (
    <div
      className={`rounded-lg border px-3 py-2 transition-colors ${
        isSuggested ? "border-border/50 opacity-70" : "border-border bg-surface-base"
      }`}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0 flex-1">
          <span className="text-xs font-medium text-primary truncate block">{atom.subject}</span>
          {atom.sourceContext && (
            <span className="text-[10px] text-muted truncate block mt-0.5">
              {atom.sourceContext.slice(0, 60)}
            </span>
          )}
        </div>

        <div className="flex items-center gap-1.5 shrink-0">
          {/* Retention badge */}
          <span
            className={`rounded-full px-1.5 py-0.5 text-[9px] font-medium ${retentionBg(atom.retentionPct)} ${retentionColor(atom.retentionPct)}`}
          >
            {Math.round(atom.retentionPct * 100)}%
          </span>

          {isSuggested ? (
            <>
              <button
                type="button"
                onClick={() => onAccept?.(atom.id)}
                className="rounded bg-brand/15 px-1.5 py-0.5 text-[10px] font-medium text-brand hover:bg-brand/25 transition-colors"
              >
                Accept
              </button>
              <button
                type="button"
                onClick={() => onDismiss?.(atom.id)}
                className="text-muted hover:text-primary text-[10px] transition-colors"
              >
                ✕
              </button>
            </>
          ) : (
            <button
              type="button"
              onClick={() => setIsReviewing(true)}
              className="rounded bg-purple-500/15 px-1.5 py-0.5 text-[10px] font-medium text-purple-400 hover:bg-purple-500/25 transition-colors"
            >
              Review
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
