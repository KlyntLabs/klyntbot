import { retentionTextColor } from "@shared/lib/retention";
import { useState } from "react";
import type { KnowledgeAtomResponse } from "../hooks/useKnowledgeAtoms";
import { InlineReview } from "./InlineReview";
import { WhyThisPopover } from "./WhyThisPopover";

interface AtomCardProps {
  atom: KnowledgeAtomResponse;
  onAccept?: (atomId: string) => void;
  onDismiss?: (atomId: string) => void;
  onReviewDone?: () => void;
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
  const metadata = atom.metadata
    ? (() => {
        try {
          return JSON.parse(atom.metadata);
        } catch {
          return null;
        }
      })()
    : null;
  const isFromGaps = metadata?.source === "gap_analysis";

  const cardContent = (
    <div
      className={`flex items-center gap-2 rounded-md px-2 py-1 transition-colors ${
        isSuggested ? "opacity-60" : "hover:bg-surface-hover"
      }`}
    >
      {/* Subject + meaning on one line */}
      <div className="flex items-center gap-1.5 min-w-0 flex-1 truncate">
        <span className="text-xs font-medium text-primary shrink-0">{atom.subject}</span>
        {isFromGaps && (
          <span className="text-[8px] px-1 py-0.5 rounded bg-amber-500/15 text-amber-400 shrink-0">
            From gaps
          </span>
        )}
        {atom.sourceContext && (
          <span className="text-2xs text-muted truncate">{atom.sourceContext}</span>
        )}
      </div>

      {/* Right side: retention + action */}
      <div className="flex items-center gap-1 shrink-0">
        {atom.retentionPct < 0.5 && (
          <span className="h-1.5 w-1.5 rounded-full bg-red-500 animate-pulse shrink-0" />
        )}
        <span
          className={`text-[9px] font-medium tabular-nums ${retentionTextColor(atom.retentionPct)}`}
        >
          {Math.round(atom.retentionPct * 100)}%
        </span>

        {isSuggested ? (
          <>
            <button
              type="button"
              onClick={() => onAccept?.(atom.id)}
              aria-label="Accept suggestion"
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

  if (isSuggested) {
    return (
      <WhyThisPopover sourceContext={atom.sourceContext} domain={atom.domain}>
        {cardContent}
      </WhyThisPopover>
    );
  }

  return cardContent;
}
