import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

interface QuickTranslatePopupProps {
  translation: string | null;
  words: Array<{
    word: string;
    reading: string | null;
    meaning: string;
    partOfSpeech: string;
    proficiencyLevel: string | null;
    isNew: boolean;
  }>;
  position: { top: number; left: number };
  loading?: boolean;
  onSaveWords: () => void;
  onPractice: () => void;
  onDismiss: () => void;
}

export function QuickTranslatePopup({
  translation,
  words,
  position,
  loading,
  onSaveWords,
  onPractice,
  onDismiss,
}: QuickTranslatePopupProps) {
  const popupRef = useRef<HTMLDivElement>(null);
  const hasData = !!translation;

  useEffect(() => {
    function handleMouseDown(e: MouseEvent) {
      if (popupRef.current && !popupRef.current.contains(e.target as Node)) {
        onDismiss();
      }
    }
    document.addEventListener("mousedown", handleMouseDown);
    return () => document.removeEventListener("mousedown", handleMouseDown);
  }, [onDismiss]);

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") onDismiss();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onDismiss]);

  return createPortal(
    <div
      ref={popupRef}
      className="fixed z-50 max-w-[320px] min-w-[200px] rounded-xl p-3 shadow-2xl backdrop-blur-[80px] backdrop-saturate-[1.6]"
      style={{
        top: position.top,
        left: position.left,
        background: "var(--surface-floating)",
        border: "1px solid var(--glass-border)",
      }}
    >
      {/* Loading state — bouncing dots */}
      {loading && !hasData && (
        <div className="flex items-center gap-2 py-1">
          <svg width="24" height="8" viewBox="0 0 24 8" className="text-brand">
            <circle cx="4" cy="4" r="3" fill="currentColor" opacity="0.3">
              <animate attributeName="opacity" values="0.3;1;0.3" dur="1s" repeatCount="indefinite" begin="0s" />
            </circle>
            <circle cx="12" cy="4" r="3" fill="currentColor" opacity="0.3">
              <animate attributeName="opacity" values="0.3;1;0.3" dur="1s" repeatCount="indefinite" begin="0.2s" />
            </circle>
            <circle cx="20" cy="4" r="3" fill="currentColor" opacity="0.3">
              <animate attributeName="opacity" values="0.3;1;0.3" dur="1s" repeatCount="indefinite" begin="0.4s" />
            </circle>
          </svg>
          <span className="text-xs text-muted">Translating...</span>
        </div>
      )}

      {/* Translation text */}
      {hasData && (
        <p className="text-sm text-primary leading-relaxed line-clamp-3">{translation}</p>
      )}

      {/* Vocabulary rows — word left, level+badge right-aligned */}
      {words.length > 0 && (
        <div className="mt-2 flex flex-col gap-1">
          {words.map((w) => (
            <div
              key={w.word}
              className="flex items-center gap-1 rounded-md px-2 py-1 text-xs"
              style={{ background: "var(--surface-glass-subtle)" }}
            >
              <span className="font-medium text-primary">{w.word}</span>
              {w.reading && <span className="text-muted">{w.reading}</span>}
              <span className="text-muted">&middot;</span>
              <span className="text-muted truncate">{w.meaning}</span>
              <span className="ml-auto flex items-center gap-1 shrink-0">
                {w.proficiencyLevel && (
                  <span className="text-brand/70 text-[10px]">{w.proficiencyLevel}</span>
                )}
                {w.isNew && (
                  <span className="rounded-full bg-brand/20 px-1.5 py-px text-[9px] font-medium text-brand">
                    new
                  </span>
                )}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* Action buttons — only when data is ready */}
      {hasData && (
        <div className="mt-3 flex items-center gap-2 border-t border-border pt-3">
          <button
            type="button"
            onClick={onSaveWords}
            className="rounded-lg px-3 py-1.5 text-xs font-medium text-brand hover:bg-surface-hover transition-colors"
          >
            Save words
          </button>
          <button
            type="button"
            onClick={onPractice}
            className="ml-auto rounded-lg bg-brand px-3 py-1.5 text-xs font-medium text-white shadow-[0_0_12px_rgba(139,92,246,0.4)] hover:brightness-110 transition-all"
          >
            Practice this note &rarr;
          </button>
        </div>
      )}
    </div>,
    document.body,
  );
}
