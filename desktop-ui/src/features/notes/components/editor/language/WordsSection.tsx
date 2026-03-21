import { useCallback, useState } from "react";
import type { WordBreakdown } from "../../../hooks/useLanguageBreakdown";

interface WordsSectionProps {
  words: WordBreakdown[];
  onSaveWords: (words: WordBreakdown[]) => void;
  saving: boolean;
}

export function WordsSection({ words, onSaveWords, saving }: WordsSectionProps) {
  const [selected, setSelected] = useState<Set<string>>(() => {
    // Pre-select new words
    return new Set(words.filter((w) => w.isNew).map((w) => w.word));
  });

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

  return (
    <div className="border-b border-border px-3 py-3">
      {/* Header */}
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
          Words ({words.length})
        </span>
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            onClick={selected.size === words.length ? deselectAll : selectAll}
            className="text-[10px] text-muted-foreground hover:text-primary transition-colors"
          >
            {selected.size === words.length ? "Deselect all" : "Select all"}
          </button>
          {selected.size > 0 && (
            <button
              type="button"
              onClick={handleSave}
              disabled={saving}
              className="rounded-md bg-brand px-2.5 py-1 text-[10px] font-semibold text-black hover:bg-brand/90 disabled:opacity-50"
            >
              {saving ? "Saving..." : `Save ${selected.size} word${selected.size !== 1 ? "s" : ""}`}
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
}: {
  word: WordBreakdown;
  isSelected: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className={`flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left transition-colors ${
        isSelected ? "bg-brand/8 ring-1 ring-brand/20" : "hover:bg-surface-hover"
      }`}
    >
      <div className="flex items-center gap-2 min-w-0">
        <div
          className={`h-3.5 w-3.5 shrink-0 rounded border transition-colors flex items-center justify-center ${
            isSelected ? "bg-brand border-brand" : "border-border"
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
        <span className="text-xs text-primary font-medium truncate">{word.word}</span>
        {word.reading && <span className="text-[10px] text-muted shrink-0">{word.reading}</span>}
        {word.isNew && <span className="text-[9px] text-brand font-medium shrink-0">new</span>}
      </div>
      <div className="flex items-center gap-2 shrink-0 ml-2">
        {word.proficiencyLevel && (
          <span className="rounded-full bg-purple-500/15 px-1.5 py-0.5 text-[9px] text-purple-400">
            {word.proficiencyLevel}
          </span>
        )}
        <span className="text-xs text-muted">{word.meaning}</span>
      </div>
    </button>
  );
}
