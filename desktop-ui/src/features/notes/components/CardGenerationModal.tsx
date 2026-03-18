import { Check, ChevronDown, ChevronUp, Loader2, Sparkles, X } from "lucide-react";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import type { GeneratedCardPreview } from "../hooks/useCardGeneration";

interface CardGenerationModalProps {
  open: boolean;
  generating: boolean;
  previews: GeneratedCardPreview[];
  deckSuggestion: string;
  approved: Set<number>;
  error: string | null;
  saving: boolean;
  onToggleCard: (index: number) => void;
  onEditCard: (index: number, field: "front" | "back", value: string) => void;
  onSave: (noteId: string | null, deck: string) => void;
  onClose: () => void;
  noteId: string | null;
}

function CardPreviewRow({
  card,
  index,
  isApproved,
  onToggle,
  onEdit,
}: {
  card: GeneratedCardPreview;
  index: number;
  isApproved: boolean;
  onToggle: () => void;
  onEdit: (field: "front" | "back", value: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);

  const typeLabel =
    card.cardType === "cloze" ? "Cloze" : card.cardType === "vocabulary" ? "Vocab" : "Basic";

  const typeBg =
    card.cardType === "cloze"
      ? "bg-purple/10 text-purple"
      : card.cardType === "vocabulary"
        ? "bg-blue-400/10 text-blue-400"
        : "bg-muted text-muted-foreground";

  return (
    <div className={`glass-card p-3 transition-all ${isApproved ? "opacity-100" : "opacity-40"}`}>
      <div className="flex items-start gap-2">
        <button
          type="button"
          onClick={onToggle}
          className={`mt-0.5 w-5 h-5 rounded flex items-center justify-center flex-shrink-0 transition-colors ${
            isApproved ? "bg-brand text-white" : "bg-muted text-muted-foreground hover:bg-accent"
          }`}
        >
          {isApproved && <Check size={12} />}
        </button>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span className={`text-[10px] px-1.5 py-0.5 rounded-md ${typeBg}`}>{typeLabel}</span>
            {card.tags.length > 0 && (
              <span className="text-[10px] text-muted-foreground">{card.tags.join(", ")}</span>
            )}
            <button
              type="button"
              onClick={() => setExpanded(!expanded)}
              className="ml-auto text-muted-foreground hover:text-foreground"
            >
              {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            </button>
          </div>

          <p className="text-sm text-foreground leading-snug">{card.front}</p>

          {expanded && (
            <div className="mt-2 space-y-2">
              <div>
                <label className="text-[10px] text-muted-foreground block mb-0.5">Front</label>
                <textarea
                  value={card.front}
                  onChange={(e) => onEdit("front", e.target.value)}
                  className="w-full bg-muted/50 rounded-md px-2 py-1.5 text-sm text-foreground resize-none"
                  rows={2}
                />
              </div>
              <div>
                <label className="text-[10px] text-muted-foreground block mb-0.5">Back</label>
                <textarea
                  value={card.back}
                  onChange={(e) => onEdit("back", e.target.value)}
                  className="w-full bg-muted/50 rounded-md px-2 py-1.5 text-sm text-foreground resize-none"
                  rows={2}
                />
              </div>
              {card.sourceContext && (
                <p className="text-[11px] text-muted-foreground italic">
                  Source: {card.sourceContext}
                </p>
              )}
            </div>
          )}

          {!expanded && (
            <p className="text-[12px] text-muted-foreground mt-0.5 truncate">{card.back}</p>
          )}
        </div>
      </div>
    </div>
  );
}

export function CardGenerationModal({
  open,
  generating,
  previews,
  deckSuggestion,
  approved,
  error,
  saving,
  onToggleCard,
  onEditCard,
  onSave,
  onClose,
  noteId,
}: CardGenerationModalProps) {
  const [deck, setDeck] = useState(deckSuggestion);

  useEffect(() => {
    if (deckSuggestion) setDeck(deckSuggestion);
  }, [deckSuggestion]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  const approvedCount = approved.size;

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
        onKeyDown={() => {}}
        role="presentation"
      />

      <div className="relative glass-panel rounded-2xl w-full max-w-lg max-h-[80vh] flex flex-col mx-4">
        <div className="flex items-center justify-between px-5 py-4 border-b border-border">
          <div className="flex items-center gap-2">
            <Sparkles size={18} className="text-brand" strokeWidth={1.5} />
            <h2 className="text-sm font-semibold text-foreground">Generate Flashcards</h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-2">
          {generating && (
            <div className="flex flex-col items-center justify-center py-12 gap-3">
              <Loader2 size={24} className="text-brand animate-spin" strokeWidth={1.5} />
              <p className="text-sm text-muted-foreground">Generating cards...</p>
            </div>
          )}

          {error && (
            <div className="glass-card p-3 border border-red-500/20">
              <p className="text-sm text-red-400">{error}</p>
            </div>
          )}

          {!generating &&
            previews.map((card, i) => (
              <CardPreviewRow
                key={i}
                card={card}
                index={i}
                isApproved={approved.has(i)}
                onToggle={() => onToggleCard(i)}
                onEdit={(field, value) => onEditCard(i, field, value)}
              />
            ))}
        </div>

        {!generating && previews.length > 0 && (
          <div className="px-5 py-4 border-t border-border space-y-3">
            <div className="flex items-center gap-2">
              <label className="text-[12px] text-muted-foreground whitespace-nowrap">Deck:</label>
              <input
                type="text"
                value={deck}
                onChange={(e) => setDeck(e.target.value)}
                placeholder="Enter deck name..."
                className="flex-1 bg-muted/50 rounded-lg px-3 py-1.5 text-sm text-foreground placeholder:text-dim"
              />
            </div>

            <div className="flex items-center justify-between">
              <span className="text-[12px] text-muted-foreground">
                {approvedCount} of {previews.length} cards selected
              </span>
              <button
                type="button"
                onClick={() => onSave(noteId, deck)}
                disabled={approvedCount === 0 || !deck.trim() || saving}
                className="glass-button px-4 py-2 text-sm text-foreground disabled:opacity-40 disabled:cursor-not-allowed inline-flex items-center gap-1.5"
              >
                {saving ? <Loader2 size={14} className="animate-spin" /> : <Check size={14} />}
                Save {approvedCount} Card{approvedCount !== 1 ? "s" : ""}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
