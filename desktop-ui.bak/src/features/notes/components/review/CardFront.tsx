import { BookOpen } from "lucide-react";
import type { Flashcard } from "../../hooks/useFlashcards";

interface CardFrontProps {
  card: Flashcard;
}

export function CardFront({ card }: CardFrontProps) {
  return (
    <div className="rounded-lg bg-white/[0.03] p-3 flex flex-col gap-2">
      {/* Header: deck name + card type badge */}
      <div className="flex items-center gap-2">
        <BookOpen size={11} className="text-muted-foreground shrink-0" />
        <span className="text-2xs text-dim truncate flex-1">{card.deck}</span>
        <span className="text-[9px] px-1.5 py-0.5 rounded-full bg-white/[0.06] text-muted-foreground capitalize shrink-0">
          {card.cardType}
        </span>
      </div>

      {/* Question */}
      <p className="text-xs text-foreground whitespace-pre-wrap leading-relaxed">{card.front}</p>
    </div>
  );
}
