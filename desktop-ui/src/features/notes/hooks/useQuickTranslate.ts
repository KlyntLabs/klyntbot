import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useRef, useState } from "react";

export interface QuickTranslateWord {
  word: string;
  reading: string | null;
  meaning: string;
  partOfSpeech: string;
  proficiencyLevel: string | null;
  exampleSentence: string | null;
  isNew: boolean;
}

export interface QuickTranslateResponse {
  translation: string;
  words: QuickTranslateWord[];
}

export interface QuickTranslatePosition {
  top: number;
  left: number;
}

/**
 * Quick Translate hook — triggered explicitly from context menu actions.
 * Does NOT auto-translate on text selection. Only fires when
 * `triggerTranslateText(text)` is called.
 */
export function useQuickTranslate(sourceLang: string, targetLang: string) {
  const [selection, setSelection] = useState<string | null>(null);
  const [translation, setTranslation] = useState<string | null>(null);
  const [words, setWords] = useState<QuickTranslateWord[]>([]);
  const [loading, setLoading] = useState(false);
  const [position, setPosition] = useState<QuickTranslatePosition | null>(null);
  const requestIdRef = useRef(0);

  const dismiss = useCallback(() => {
    requestIdRef.current++;
    setSelection(null);
    setTranslation(null);
    setWords([]);
    setPosition(null);
    setLoading(false);
  }, []);

  const triggerTranslateText = useCallback(
    (text: string, rect?: { top: number; left: number }) => {
      if (!text.trim()) return;

      // Use provided rect (captured before context menu cleared selection),
      // fall back to current selection, then to a default position
      if (rect) {
        setPosition(rect);
      } else {
        const sel = window.getSelection();
        if (sel && sel.rangeCount > 0) {
          const r = sel.getRangeAt(0).getBoundingClientRect();
          setPosition({ top: r.bottom + 8, left: r.left });
        } else {
          setPosition({ top: 200, left: 300 });
        }
      }

      setSelection(text.trim());
      setTranslation(null);
      setWords([]);
      setLoading(true);
      const id = ++requestIdRef.current;

      ipc<QuickTranslateResponse>("language_quick_translate", {
        params: { text: text.trim(), sourceLang, targetLang },
      })
        .then((response) => {
          if (id === requestIdRef.current) {
            setTranslation(response.translation);
            setWords(response.words);
          }
        })
        .catch(() => {
          if (id === requestIdRef.current) {
            setTranslation(null);
            setWords([]);
          }
        })
        .finally(() => {
          if (id === requestIdRef.current) {
            setLoading(false);
          }
        });
    },
    [sourceLang, targetLang],
  );

  return { selection, translation, words, loading, position, dismiss, triggerTranslateText };
}
