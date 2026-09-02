import { ThinkingDots } from "@shared/ui/ThinkingDots";
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
  isApproved,
  onToggle,
  onEdit,
}: {
  card: GeneratedCardPreview;
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
        : "bg-control-hover text-fg-secondary";

  return (
    <div className={`island p-3 transition-all ${isApproved ? "opacity-100" : "opacity-40"}`}>
      <div className="flex items-start gap-2">
        <button
          type="button"
          onClick={onToggle}
          className={`mt-0.5 size-5 rounded flex items-center justify-center flex-shrink-0 transition-colors ${
            isApproved ? "bg-brand text-white" : "bg-control-hover text-fg-secondary hover:bg-control-hover"
          }`}
        >
          {isApproved && <Check size={12} />}
        </button>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span className={`text-ui-xs px-1.5 py-0.5 rounded-md ${typeBg}`}>{typeLabel}</span>
            {card.tags.length > 0 && (
              <span className="text-ui-xs text-fg-secondary">{card.tags.join(", ")}</span>
            )}
            <button
              type="button"
              onClick={() => setExpanded(!expanded)}
              className="ml-auto text-fg-secondary hover:text-fg"
            >
              {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            </button>
          </div>

          <p className="text-sm text-fg leading-snug">{card.front}</p>

          {expanded && (
            <div className="mt-2 space-y-2">
              <label className="block">
                <span className="text-ui-xs text-fg-secondary block mb-0.5">Front</span>
                <textarea
                  value={card.front}
                  onChange={(e) => onEdit("front", e.target.value)}
                  className="w-full bg-control-hover/50 rounded-md px-2 py-1.5 text-sm text-fg resize-none"
                  rows={2}
                />
              </label>
              <label className="block">
                <span className="text-ui-xs text-fg-secondary block mb-0.5">Back</span>
                <textarea
                  value={card.back}
                  onChange={(e) => onEdit("back", e.target.value)}
                  className="w-full bg-control-hover/50 rounded-md px-2 py-1.5 text-sm text-fg resize-none"
                  rows={2}
                />
              </label>
              {card.sourceContext && (
                <p className="text-ui-xs text-fg-secondary italic">
                  Source: {card.sourceContext}
                </p>
              )}
            </div>
          )}

          {!expanded && (
            <p className="text-ui-sm text-fg-secondary mt-0.5 truncate">{card.back}</p>
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
      {/* biome-ignore lint/a11y/noStaticElementInteractions: backdrop overlay with role=presentation */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
        onKeyDown={() => {}}
        role="presentation"
      />

      <div className="relative glass-panel rounded-2xl w-full max-w-lg max-h-[80vh] flex flex-col mx-4">
        <div className="flex items-center justify-between px-5 py-4 border-b border-separator">
          <div className="flex items-center gap-2">
            <Sparkles size={18} className="text-brand" strokeWidth={1.5} />
            <h2 className="text-sm font-semibold text-fg">Generate Flashcards</h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-fg-secondary hover:text-fg transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-2">
          {generating && (
            <div className="flex flex-col items-center justify-center py-12 gap-3">
              <ThinkingDots />
              <p className="text-sm text-fg-secondary">Generating cards</p>
            </div>
          )}

          {error && (
            <div className="island p-3 border border-red-500/20">
              <p className="text-sm text-red-400">{error}</p>
            </div>
          )}

          {!generating &&
            previews.map((card, cardIndex) => (
              <CardPreviewRow
                key={`${card.cardType}-${card.front.slice(0, 40)}-${card.back.slice(0, 20)}`}
                card={card}
                isApproved={approved.has(cardIndex)}
                onToggle={() => onToggleCard(cardIndex)}
                onEdit={(field, value) => onEditCard(cardIndex, field, value)}
              />
            ))}
        </div>

        {!generating && previews.length > 0 && (
          <div className="px-5 py-4 border-t border-separator space-y-3">
            <label className="flex items-center gap-2">
              <span className="text-ui-sm text-fg-secondary whitespace-nowrap">Deck:</span>
              <input
                type="text"
                value={deck}
                onChange={(e) => setDeck(e.target.value)}
                placeholder="Enter deck name..."
                className="flex-1 bg-control-hover/50 rounded-lg px-3 py-1.5 text-sm text-fg placeholder:text-fg-dim"
              />
            </label>

            <div className="flex items-center justify-between">
              <span className="text-ui-sm text-fg-secondary">
                {approvedCount} of {previews.length} cards selected
              </span>
              <button
                type="button"
                onClick={() => onSave(noteId, deck)}
                disabled={approvedCount === 0 || !deck.trim() || saving}
                className="glass-button px-4 py-2 text-sm text-fg disabled:opacity-40 disabled:cursor-not-allowed inline-flex items-center gap-1.5"
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
