import { Layers, Play } from "lucide-react";
import type { DeckSummary } from "../../notes/hooks/useFlashcards";

interface DeckListProps {
  decks: DeckSummary[];
  onReviewDeck: (deck: string) => void;
}

export function DeckList({ decks, onReviewDeck }: DeckListProps) {
  if (decks.length === 0) {
    return (
      <div className="text-center py-8">
        <Layers size={28} className="mx-auto text-muted-foreground mb-2" strokeWidth={1.5} />
        <p className="text-sm text-muted-foreground">No decks yet</p>
        <p className="text-[11px] text-muted-foreground mt-0.5">Create flashcards to get started</p>
      </div>
    );
  }

  return (
    <div className="space-y-1">
      <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider px-1 mb-2">
        Decks
      </h3>
      {decks.map((deck) => (
        <div
          key={deck.name}
          className="flex items-center justify-between px-3 py-2.5 rounded-lg bg-white/[0.03] hover:bg-white/[0.06] transition-all duration-200 group"
        >
          <div className="flex items-center gap-2.5 min-w-0">
            <Layers size={16} className="text-muted-foreground shrink-0" strokeWidth={1.5} />
            <div className="min-w-0">
              <p className="text-sm text-foreground truncate">{deck.name}</p>
              <p className="text-[11px] text-muted-foreground">
                {deck.cardCount} card{deck.cardCount !== 1 ? "s" : ""}
                {deck.dueCount > 0 && (
                  <span className="text-brand ml-1.5">{deck.dueCount} due</span>
                )}
              </p>
            </div>
          </div>
          {deck.dueCount > 0 && (
            <button
              type="button"
              onClick={() => onReviewDeck(deck.name)}
              className="glass-button px-2.5 py-1 text-xs text-foreground flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-200"
            >
              <Play size={12} strokeWidth={1.5} />
              Review
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
