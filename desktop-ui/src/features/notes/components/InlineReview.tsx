import { useCallback, useEffect, useRef, useState } from "react";
import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import type { ReviewQuality } from "../hooks/useFlashcards";
import { fetchNextCard, type FlashcardForReview } from "../hooks/useKnowledgeAtoms";

interface InlineReviewProps {
  atomId: string;
  onDone: () => void;
}

export function InlineReview({ atomId, onDone }: InlineReviewProps) {
  const [card, setCard] = useState<FlashcardForReview | null>(null);
  const [loading, setLoading] = useState(true);
  const [revealed, setRevealed] = useState(false);
  const [isSubmitting, setRating] = useState(false);
  const startTimeRef = useRef(Date.now());

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    fetchNextCard(atomId).then((c) => {
      if (!cancelled) {
        setCard(c);
        setLoading(false);
        startTimeRef.current = Date.now();
      }
    });
    return () => {
      cancelled = true;
    };
  }, [atomId]);

  const handleRate = useCallback(
    async (quality: ReviewQuality) => {
      if (!card || isSubmitting) return;
      setRating(true);
      const recallSpeedMs = Date.now() - startTimeRef.current;
      try {
        await ipc("flashcard_record_review", {
          params: { cardId: card.id, quality, recallSpeedMs },
        });
        invalidateQueries("atoms_for_note");
      } finally {
        onDone();
      }
    },
    [card, isSubmitting, onDone],
  );

  if (loading) {
    return (
      <div className="rounded-lg border border-border px-3 py-4 text-center">
        <span className="text-[10px] text-muted animate-pulse">Loading card...</span>
      </div>
    );
  }

  if (!card) {
    return (
      <div className="rounded-lg border border-border px-3 py-3">
        <p className="text-[10px] text-muted text-center">No cards available</p>
        <button
          type="button"
          onClick={onDone}
          className="mt-1 text-[10px] text-brand hover:text-brand/80 block mx-auto"
        >
          Close
        </button>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-purple-500/20 bg-purple-500/5 px-3 py-3">
      {/* Front */}
      <div className="text-center mb-2">
        <span className="text-sm font-semibold text-primary">{card.front}</span>
      </div>

      {!revealed ? (
        <button
          type="button"
          onClick={() => setRevealed(true)}
          className="w-full rounded-md bg-surface-hover px-3 py-2 text-xs text-muted-foreground hover:text-primary transition-colors"
        >
          Show answer
        </button>
      ) : (
        <>
          {/* Back */}
          <div className="text-center mb-3 border-t border-border/50 pt-2">
            <span className="text-sm text-primary">{card.back}</span>
          </div>

          {/* Rating buttons (compact) */}
          <div className="flex items-center gap-1 justify-center">
            {(["again", "hard", "good", "easy"] as const).map((q) => (
              <button
                key={q}
                type="button"
                onClick={() => handleRate(q)}
                disabled={isSubmitting}
                className={`rounded-md px-2 py-1 text-[10px] font-medium transition-colors disabled:opacity-50 ${isSubmittingStyle(q)}`}
              >
                {q.charAt(0).toUpperCase() + q.slice(1)}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function isSubmittingStyle(quality: ReviewQuality): string {
  switch (quality) {
    case "again":
      return "bg-red-500/15 text-red-400 hover:bg-red-500/25";
    case "hard":
      return "bg-amber-500/15 text-amber-400 hover:bg-amber-500/25";
    case "good":
      return "bg-emerald-500/15 text-emerald-400 hover:bg-emerald-500/25";
    case "easy":
      return "bg-blue-500/15 text-blue-400 hover:bg-blue-500/25";
  }
}
