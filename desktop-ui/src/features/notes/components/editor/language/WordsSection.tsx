import { useCallback, useEffect, useState } from "react";
import type { WordBreakdown } from "../../../hooks/useLanguageBreakdown";

interface WordsSectionProps {
  words: WordBreakdown[];
  onSaveWords: (words: WordBreakdown[]) => void;
  saving: boolean;
  saved: boolean;
}

export function WordsSection({ words, onSaveWords, saving, saved }: WordsSectionProps) {
  const [selected, setSelected] = useState<Set<string>>(() => {
    return new Set(words.filter((w) => w.isNew).map((w) => w.word));
  });

  // Clear selection when save completes
  useEffect(() => {
    if (saved) setSelected(new Set());
  }, [saved]);

  const toggle = useCallback((word: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(word)) next.delete(word);
      else next.add(word);
      return next;
    });
  }, []);

  const selectAll = useCallback(() => {
    setSelected(new Set(words.map((w) => w.word)));
  }, [words]);

  const deselectAll = useCallback(() => {
    setSelected(new Set());
  }, []);

  const handleSave = useCallback(() => {
    const toSave = words.filter((w) => selected.has(w.word));
    if (toSave.length > 0) onSaveWords(toSave);
  }, [words, selected, onSaveWords]);

  const buttonLabel = saving
    ? "Saving..."
    : saved
      ? "Saved!"
      : `Save ${selected.size} word${selected.size !== 1 ? "s" : ""}`;

  const buttonClass = saving
    ? "rounded-md bg-brand/50 px-2.5 py-1 text-ui-xs font-semibold text-black cursor-wait"
    : saved
      ? "rounded-md bg-green-500 px-2.5 py-1 text-ui-xs font-semibold text-black"
      : "rounded-md bg-brand px-2.5 py-1 text-ui-xs font-semibold text-black hover:bg-brand/90 active:scale-95 transition-all";

  return (
    <div className="border-b border-separator px-3 py-3">
      {/* Header */}
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-ui-xs text-fg-secondary uppercase tracking-wider">
          Words ({words.length})
        </span>
        <div className="flex items-center gap-1.5">
          {!saved && (
            <button
              type="button"
              onClick={selected.size === words.length ? deselectAll : selectAll}
              className="text-ui-xs text-fg-secondary hover:text-brand transition-colors"
            >
              {selected.size === words.length ? "Deselect all" : "Select all"}
            </button>
          )}
          {(selected.size > 0 || saved) && (
            <button
              type="button"
              onClick={handleSave}
              disabled={saving || saved}
              className={buttonClass}
            >
              {buttonLabel}
            </button>
          )}
        </div>
      </div>

      {/* Word list */}
      <div className="space-y-0.5">
        {words.map((word) => (
          <WordRow
            key={word.word}
            word={word}
            isSelected={selected.has(word.word)}
            onToggle={() => toggle(word.word)}
            disabled={saving}
          />
        ))}
      </div>
    </div>
  );
}

function WordRow({
  word,
  isSelected,
  onToggle,
  disabled,
}: {
  word: WordBreakdown;
  isSelected: boolean;
  onToggle: () => void;
  disabled: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      disabled={disabled}
      className={`flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left transition-colors disabled:opacity-50 ${
        isSelected ? "bg-brand/8 ring-1 ring-brand/20" : "hover:bg-control-hover"
      }`}
    >
      <div className="flex items-center gap-2 min-w-0">
        <div
          className={`h-3.5 w-3.5 shrink-0 rounded border transition-colors flex items-center justify-center ${
            isSelected ? "bg-brand border-brand" : "border-separator"
          }`}
        >
          {isSelected && (
            <svg
              viewBox="0 0 12 12"
              className="h-2.5 w-2.5 text-black"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              aria-hidden="true"
            >
              <path d="M2 6l3 3 5-5" />
            </svg>
          )}
        </div>
        <span className="text-ui-sm text-brand font-medium truncate">{word.word}</span>
        {word.reading && <span className="text-ui-xs text-fg-secondary shrink-0">{word.reading}</span>}
        {word.isNew && <span className="text-[9px] text-brand font-medium shrink-0">new</span>}
      </div>
      <div className="flex items-center gap-2 shrink-0 ml-2">
        {word.proficiencyLevel && (
          <span className="rounded-full bg-purple-500/15 px-1.5 py-0.5 text-[9px] text-purple-400">
            {word.proficiencyLevel}
          </span>
        )}
        <span className="text-ui-sm text-fg-secondary">{word.meaning}</span>
      </div>
    </button>
  );
}
