import { ipc } from "@shared/hooks/useIpc";
import { X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface QuickAddProps {
  open: boolean;
  onClose: () => void;
  onCreated: () => void;
}

type CardType = "basic" | "cloze" | "vocabulary";

const cardTypeOptions: { value: CardType; label: string }[] = [
  { value: "basic", label: "Basic" },
  { value: "cloze", label: "Cloze" },
  { value: "vocabulary", label: "Vocabulary" },
];

export function QuickAdd({ open, onClose, onCreated }: QuickAddProps) {
  const [front, setFront] = useState("");
  const [back, setBack] = useState("");
  const [deck, setDeck] = useState("general");
  const [cardType, setCardType] = useState<CardType>("basic");
  const [creating, setCreating] = useState(false);
  const frontRef = useRef<HTMLTextAreaElement>(null);

  // Reset form when opening
  useEffect(() => {
    if (!open) return;
    setFront("");
    setBack("");
    setDeck("general");
    setCardType("basic");
    setCreating(false);
    // Focus after a tick so the element is rendered
    const id = setTimeout(() => frontRef.current?.focus(), 50);
    return () => clearTimeout(id);
  }, [open]);

  // Escape to close
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);

  const handleCreate = useCallback(async () => {
    if (!front.trim()) return;
    setCreating(true);
    try {
      await ipc("flashcard_create", {
        deck: deck.trim() || "general",
        front: front.trim(),
        back: back.trim(),
        cardType,
      });
      onCreated();
    } catch {
      // silently fail
      setCreating(false);
    }
  }, [front, back, deck, cardType, onCreated]);

  const handleBackKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleCreate();
      }
    },
    [handleCreate],
  );

  const handleBackdropClick = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === e.currentTarget) onClose();
    },
    [onClose],
  );

  if (!open) return null;

  return createPortal(
    // biome-ignore lint/a11y/useKeyWithClickEvents lint/a11y/noStaticElementInteractions: modal backdrop - dismiss via click or Escape key
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      onClick={handleBackdropClick}
    >
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40" />

      {/* Modal */}
      <div
        role="dialog"
        aria-label="Quick Add Card"
        className="relative glass-panel p-5 w-full max-w-md animate-[glass-appear_0.2s_ease-out] space-y-4"
      >
        {/* Header */}
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-fg">Quick Add Card</h3>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-md text-fg-secondary hover:text-fg transition-colors"
          >
            <X size={16} strokeWidth={1.5} />
          </button>
        </div>

        {/* Front */}
        <label className="block">
          <span className="block text-ui-xs text-fg-secondary mb-1">Front</span>
          <textarea
            ref={frontRef}
            value={front}
            onChange={(e) => setFront(e.target.value)}
            rows={3}
            placeholder="Question or prompt..."
            className="glass-input w-full px-3 py-2 text-sm text-fg resize-none placeholder:text-fg-secondary"
          />
        </label>

        {/* Back */}
        <label className="block">
          <span className="block text-ui-xs text-fg-secondary mb-1">Back</span>
          <textarea
            value={back}
            onChange={(e) => setBack(e.target.value)}
            onKeyDown={handleBackKeyDown}
            rows={3}
            placeholder="Answer..."
            className="glass-input w-full px-3 py-2 text-sm text-fg resize-none placeholder:text-fg-secondary"
          />
        </label>

        {/* Deck + Card Type row */}
        <div className="flex gap-3">
          <label className="flex-1 block">
            <span className="block text-ui-xs text-fg-secondary mb-1">Deck</span>
            <input
              type="text"
              value={deck}
              onChange={(e) => setDeck(e.target.value)}
              placeholder="general"
              className="glass-input w-full px-3 py-1.5 text-sm text-fg placeholder:text-fg-secondary"
            />
          </label>
          <label className="flex-1 block">
            <span className="block text-ui-xs text-fg-secondary mb-1">Type</span>
            <select
              value={cardType}
              onChange={(e) => setCardType(e.target.value as CardType)}
              className="glass-input w-full px-3 py-1.5 text-sm text-fg bg-transparent"
            >
              {cardTypeOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </label>
        </div>

        {/* Actions */}
        <div className="flex items-center justify-end gap-2 pt-1">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-fg-secondary hover:text-fg transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleCreate}
            disabled={!front.trim() || creating}
            className="glass-button px-4 py-1.5 text-sm text-fg disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {creating ? "Creating..." : "Create"}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
