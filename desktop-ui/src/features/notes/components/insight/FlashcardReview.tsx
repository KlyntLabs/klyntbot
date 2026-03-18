import { BookOpen, ChevronRight, X } from "lucide-react";
import { useEffect } from "react";
import type { DeckSummary } from "../../hooks/useFlashcards";
import { useFlashcards } from "../../hooks/useFlashcards";

interface FlashcardReviewProps {
  onClose: () => void;
}

export function FlashcardReview({ onClose }: FlashcardReviewProps) {
  const { decks, current, remaining, done, revealed, fetchDecks, startReview, reveal, review } =
    useFlashcards();

  useEffect(() => {
    fetchDecks();
  }, [fetchDecks]);

  // Deck picker — shown when no review session is active
  if (!current && !done) {
    const dueDecks = decks.filter((d) => d.dueCount > 0);

    if (decks.length === 0) {
      return (
        <div className="flex flex-col items-center justify-center gap-3 py-8">
          <p className="text-[11px] text-dim">Loading decks...</p>
        </div>
      );
    }

    if (dueDecks.length === 0) {
      return (
        <div className="flex flex-col items-center justify-center gap-3 py-8">
          <BookOpen size={24} className="text-accent" />
          <p className="text-[12px] text-foreground font-medium">No cards due for review</p>
          <p className="text-[10px] text-dim">
            {decks.length} {decks.length === 1 ? "deck" : "decks"} saved, all caught up!
          </p>
          <button
            type="button"
            onClick={onClose}
            className="text-[10px] px-3 py-1 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground"
          >
            Done
          </button>
        </div>
      );
    }

    return (
      <div className="flex flex-col gap-3 p-3">
        <div className="flex items-center gap-2">
          <span className="text-[11px] text-foreground font-medium">Choose a deck to review</span>
          <div className="flex-1" />
          <button type="button" onClick={onClose} className="p-1 text-dim hover:text-foreground">
            <X size={12} />
          </button>
        </div>
        <div className="space-y-1.5">
          {dueDecks.map((d: DeckSummary) => (
            <button
              key={d.name}
              type="button"
              onClick={() => startReview(d.name)}
              className="w-full flex items-center gap-2 p-2 rounded-lg bg-white/[0.03] hover:bg-white/[0.06] text-left"
            >
              <BookOpen size={12} className="text-muted-foreground shrink-0" />
              <span className="text-[11px] text-foreground truncate flex-1">{d.name}</span>
              <span className="text-[10px] text-dim shrink-0">
                {d.dueCount}/{d.cardCount} due
              </span>
            </button>
          ))}
        </div>
      </div>
    );
  }

  if (done) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-8">
        <BookOpen size={24} className="text-accent" />
        <p className="text-[12px] text-foreground font-medium">Review complete!</p>
        <button
          type="button"
          onClick={onClose}
          className="text-[10px] px-3 py-1 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground"
        >
          Done
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3 p-3">
      {/* Header */}
      <div className="flex items-center gap-2">
        <span className="text-[10px] text-dim">{remaining} remaining</span>
        <div className="flex-1" />
        <button type="button" onClick={onClose} className="p-1 text-dim hover:text-foreground">
          <X size={12} />
        </button>
      </div>

      {/* Front */}
      <div className="rounded-lg bg-white/[0.03] p-3">
        <p className="text-[11px] text-foreground whitespace-pre-wrap">{current.front}</p>
      </div>

      {/* Back (revealed or button) */}
      {revealed ? (
        <>
          <div className="rounded-lg bg-white/[0.04] border border-border p-3">
            <p className="text-[11px] text-foreground whitespace-pre-wrap">{current.back}</p>
          </div>
          <div className="flex gap-2 justify-center">
            {(["again", "hard", "good", "easy"] as const).map((q) => (
              <button
                key={q}
                type="button"
                onClick={() => review(q)}
                className="text-[10px] px-3 py-1.5 rounded-md bg-white/[0.04] text-muted-foreground hover:text-foreground hover:bg-white/[0.08] capitalize"
              >
                {q}
              </button>
            ))}
          </div>
        </>
      ) : (
        <button
          type="button"
          onClick={reveal}
          className="flex items-center justify-center gap-1 text-[10px] px-3 py-2 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground hover:bg-white/[0.08]"
        >
          <ChevronRight size={10} />
          Show Answer
        </button>
      )}
    </div>
  );
}
