import { useQuery } from "@shared/hooks/useQuery";
import { useCallback, useState } from "react";

interface StudyModeViewProps {
  noteId: string;
  sectionId: string;
}

interface FlashcardResponse {
  id: string;
  deck: string;
  front: string;
  back: string;
  cardType: string;
  dueAt: string | null;
  state: string;
}

const EMPTY_CARDS: FlashcardResponse[] = [];

export function StudyModeView({ noteId, sectionId: _ }: StudyModeViewProps) {
  const { data: cards } = useQuery<FlashcardResponse[]>(
    "flashcard_list_cards",
    { deck: noteId, limit: 20, offset: 0 },
    EMPTY_CARDS,
  );

  const [revealedIndex, setRevealedIndex] = useState<number | null>(null);

  const toggleReveal = useCallback((index: number) => {
    setRevealedIndex((prev) => (prev === index ? null : index));
  }, []);

  if (!cards || cards.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-ui-sm text-fg-secondary">
        No flashcards for this note. Create some with ⌥F.
      </div>
    );
  }

  const now = new Date();
  const dueCards = cards.filter((c) => c.dueAt && new Date(c.dueAt) <= now);

  return (
    <div className="flex h-full flex-col gap-3 overflow-y-auto p-4">
      <div className="flex items-center justify-between">
        <h3 className="text-ui-sm font-medium text-fg-secondary">
          {cards.length} card{cards.length !== 1 ? "s" : ""}
        </h3>
        {dueCards.length > 0 && (
          <span className="rounded-full bg-amber-500/15 px-2 py-0.5 text-ui-xs text-amber-400">
            {dueCards.length} due
          </span>
        )}
      </div>

      {cards.map((card, index) => {
        const isDue = card.dueAt && new Date(card.dueAt) <= now;
        const revealed = revealedIndex === index;

        return (
          <button
            type="button"
            key={card.id}
            onClick={() => toggleReveal(index)}
            className={`rounded-lg border p-3 text-left transition-all ${
              isDue ? "border-amber-500/30 bg-amber-500/5" : "border-separator bg-bg-elevated"
            }`}
          >
            <p className="text-ui-sm font-medium text-brand">{card.front}</p>
            {revealed && (
              <div className="mt-2 border-t border-separator pt-2">
                <p className="text-ui-sm text-fg-secondary">{card.back}</p>
              </div>
            )}
            {!revealed && <p className="mt-1 text-ui-xs text-fg-secondary">Click to reveal</p>}
          </button>
        );
      })}
    </div>
  );
}
