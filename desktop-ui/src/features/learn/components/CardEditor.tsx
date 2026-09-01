import { ipc } from "@shared/hooks/useIpc";
import { Check, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Flashcard } from "../../notes/hooks/useFlashcards";

interface CardEditorProps {
  card: Flashcard;
  onSaved: (updated: { front: string; back: string; deck: string }) => void;
  onCancel: () => void;
}

export function CardEditor({ card, onSaved, onCancel }: CardEditorProps) {
  const [front, setFront] = useState(card.front);
  const [back, setBack] = useState(card.back);
  const [deck, setDeck] = useState(card.deck);
  const [saving, setSaving] = useState(false);
  const frontRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    frontRef.current?.focus();
  }, []);

  const handleSave = useCallback(async () => {
    if (!front.trim()) return;
    setSaving(true);
    try {
      await ipc("flashcard_update", {
        id: card.id,
        front: front.trim(),
        back: back.trim(),
        deck: deck.trim() || card.deck,
      });
      onSaved({
        front: front.trim(),
        back: back.trim(),
        deck: deck.trim() || card.deck,
      });
    } catch {
      setSaving(false);
    }
  }, [card.id, card.deck, front, back, deck, onSaved]);

  // Escape to cancel, Cmd+Enter to save
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onCancel();
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        handleSave();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onCancel, handleSave]);

  return (
    <div className="w-full max-w-lg space-y-4">
      <div className="glass-card p-6 space-y-4">
        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Front</span>
          <textarea
            ref={frontRef}
            value={front}
            onChange={(e) => setFront(e.target.value)}
            rows={3}
            className="glass-input w-full px-3 py-2 text-sm text-foreground resize-none"
          />
        </label>
        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Back</span>
          <textarea
            value={back}
            onChange={(e) => setBack(e.target.value)}
            rows={3}
            className="glass-input w-full px-3 py-2 text-sm text-foreground resize-none"
          />
        </label>
        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Deck</span>
          <input
            type="text"
            value={deck}
            onChange={(e) => setDeck(e.target.value)}
            className="glass-input w-full px-3 py-1.5 text-sm text-foreground"
          />
        </label>
      </div>
      <div className="flex items-center justify-center gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="glass-button px-3 py-1.5 text-sm text-muted-foreground flex items-center gap-1"
        >
          <X size={14} strokeWidth={1.5} />
          Cancel
          <span className="text-2xs text-muted-foreground ml-1">Esc</span>
        </button>
        <button
          type="button"
          onClick={handleSave}
          disabled={!front.trim() || saving}
          className="glass-button px-4 py-1.5 text-sm text-foreground flex items-center gap-1 disabled:opacity-40"
        >
          <Check size={14} strokeWidth={1.5} />
          {saving ? "Saving..." : "Save"}
          <span className="text-2xs text-muted-foreground ml-1">{"\u2318"}↩</span>
        </button>
      </div>
    </div>
  );
}
