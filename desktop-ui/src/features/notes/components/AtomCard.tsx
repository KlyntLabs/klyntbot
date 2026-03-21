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
      className={`flex items-center gap-2 rounded-md px-2 py-1 transition-colors ${
        isSuggested ? "opacity-60" : "hover:bg-surface-hover"
      }`}
    >
      {/* Subject + meaning on one line */}
      <div className="flex items-center gap-1.5 min-w-0 flex-1 truncate">
        <span className="text-xs font-medium text-primary shrink-0">{atom.subject}</span>
        {atom.sourceContext && (
          <span className="text-[10px] text-muted truncate">{atom.sourceContext}</span>
        )}
      </div>

      {/* Right side: retention + action */}
      <div className="flex items-center gap-1 shrink-0">
        <span
          className={`text-[9px] font-medium tabular-nums ${retentionColor(atom.retentionPct)}`}
        >
          {Math.round(atom.retentionPct * 100)}%
        </span>

        {isSuggested ? (
          <>
            <button
              type="button"
              onClick={() => onAccept?.(atom.id)}
              className="rounded px-1.5 py-0.5 text-[9px] font-medium text-brand hover:bg-brand/15 transition-colors"
            >
              +
            </button>
            <button
              type="button"
              onClick={() => onDismiss?.(atom.id)}
              className="text-muted hover:text-primary text-[9px] transition-colors px-0.5"
            >
              ✕
            </button>
          </>
        ) : (
          <button
            type="button"
            onClick={() => setIsReviewing(true)}
            className="rounded px-1.5 py-0.5 text-[9px] font-medium text-purple-400 hover:bg-purple-500/15 transition-colors"
          >
            Review
          </button>
        )}
      </div>
    </div>
  );
}
