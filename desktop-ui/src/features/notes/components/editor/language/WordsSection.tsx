import type { WordBreakdown } from "../../../hooks/useLanguageBreakdown";

interface WordsSectionProps {
  words: WordBreakdown[];
  onSaveWords: (words: WordBreakdown[]) => void;
  saving: boolean;
}

export function WordsSection({ words, onSaveWords, saving }: WordsSectionProps) {
  const newWords = words.filter((w) => w.isNew);

  return (
    <div className="border-b border-border px-3 py-3">
      <div className="flex items-center justify-between mb-2">
        <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
          Words ({words.length})
        </span>
        {newWords.length > 0 && (
          <button
            type="button"
            onClick={() => onSaveWords(newWords)}
            disabled={saving}
            className="rounded-md bg-brand px-2.5 py-1 text-[10px] font-semibold text-black hover:bg-brand/90 disabled:opacity-50"
          >
            {saving
              ? "Saving..."
              : `Save ${newWords.length} new word${newWords.length !== 1 ? "s" : ""}`}
          </button>
        )}
      </div>
      <div className="space-y-1">
        {words.map((word) => (
          <WordRow key={word.word} word={word} />
        ))}
      </div>
    </div>
  );
}

function WordRow({ word }: { word: WordBreakdown }) {
  return (
    <div className="flex items-center justify-between py-1.5 border-b border-border/50 last:border-0">
      <div className="flex items-center gap-2">
        <span className="text-xs text-primary font-medium">{word.word}</span>
        {word.reading && <span className="text-[10px] text-muted">{word.reading}</span>}
        {word.isNew && <span className="text-[9px] text-brand font-medium">new</span>}
      </div>
      <div className="flex items-center gap-2">
        {word.proficiencyLevel && (
          <span className="rounded-full bg-purple-500/15 px-1.5 py-0.5 text-[9px] text-purple-400">
            {word.proficiencyLevel}
          </span>
        )}
        <span className="text-xs text-muted">{word.meaning}</span>
      </div>
    </div>
  );
}
