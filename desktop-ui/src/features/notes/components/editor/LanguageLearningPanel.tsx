import { useEffect } from "react";
import type { WordBreakdown } from "../../hooks/useLanguageBreakdown";
import { useLanguageBreakdown } from "../../hooks/useLanguageBreakdown";
import { useVocabularySave } from "../../hooks/useVocabularySave";
import { CollapsibleSection } from "./language/CollapsibleSection";
import { ConfusableSection } from "./language/ConfusableSection";
import { PracticeSection } from "./language/PracticeSection";
import { TranslationSection } from "./language/TranslationSection";
import { WordsSection } from "./language/WordsSection";

interface LanguageLearningPanelProps {
  noteId: string;
  noteTitle: string;
  sourceText: string;
  sourceLang: string;
  targetLang: string;
}

export function LanguageLearningPanel({
  noteId,
  noteTitle,
  sourceText,
  sourceLang,
  targetLang,
}: LanguageLearningPanelProps) {
  const { result, loading, error, translate } = useLanguageBreakdown();
  const { saving, savedCount, saveWords, dismissSaved } = useVocabularySave();

  // Auto-translate on mount or when source text changes
  useEffect(() => {
    if (sourceText.trim().length > 5) {
      translate(sourceText, sourceLang, targetLang);
    }
  }, [sourceText, sourceLang, targetLang, translate]);

  const handleSaveWords = (words: WordBreakdown[]) => {
    saveWords(words, noteId, noteTitle);
  };

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      {/* Save feedback snackbar */}
      {savedCount !== null && (
        <div className="mx-3 mt-2 flex items-center justify-between rounded-md bg-green-500/10 px-3 py-2 text-xs text-green-400">
          <span>
            Saved {savedCount} word{savedCount !== 1 ? "s" : ""} to &ldquo;
            {noteTitle}&rdquo;
          </span>
          <button
            type="button"
            onClick={dismissSaved}
            className="text-green-300 hover:text-green-200"
          >
            &times;
          </button>
        </div>
      )}

      {/* Section 1: Translation (always expanded) */}
      <TranslationSection
        translation={result?.translation ?? null}
        loading={loading}
        error={error}
        onRetry={() => translate(sourceText, sourceLang, targetLang)}
      />

      {/* Section 2: Words (always expanded) */}
      {result && (
        <WordsSection words={result.words} onSaveWords={handleSaveWords} saving={saving} />
      )}

      {/* Section 3: Grammar (collapsed by default) */}
      {result && result.grammarPatterns.length > 0 && (
        <CollapsibleSection title="Grammar Patterns">
          <div className="space-y-2">
            {result.grammarPatterns.map((gp, i) => (
              <div
                key={`${gp.pattern}-${i}`}
                className="rounded-md border border-blue-500/20 bg-blue-500/5 p-2"
              >
                <p className="text-xs font-mono text-blue-300">{gp.pattern}</p>
                <p className="mt-1 text-xs text-muted">{gp.explanation}</p>
                {gp.patternType && (
                  <span className="mt-1 inline-block rounded-full bg-blue-500/15 px-1.5 py-0.5 text-[9px] text-blue-400">
                    {gp.patternType}
                  </span>
                )}
              </div>
            ))}
          </div>
        </CollapsibleSection>
      )}

      {/* Section 4: Practice (collapsed by default) */}
      {result && (
        <CollapsibleSection title="Practice">
          <PracticeSection
            sourceText={sourceText}
            sourceLang={sourceLang}
            targetLang={targetLang}
          />
        </CollapsibleSection>
      )}

      {/* Section 5: Confusables (conditional — only shows when matches found) */}
      {result && result.words.some((w) => w.isNew) && (
        <CollapsibleSection title="Confusable Words">
          <ConfusableSection words={result.words} sourceLang={sourceLang} />
        </CollapsibleSection>
      )}
    </div>
  );
}
