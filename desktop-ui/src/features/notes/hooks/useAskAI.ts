import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useRef, useState } from "react";

export interface AskAIPosition {
  top: number;
  left: number;
}

/**
 * Hook for the inline "Ask AI" feature in the note editor.
 * Sends selected text + user prompt to the agent, displays the response.
 */
export function useAskAI(noteId: string | null) {
  const [selectedText, setSelectedText] = useState<string | null>(null);
  const [position, setPosition] = useState<AskAIPosition | null>(null);
  const [response, setResponse] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const requestIdRef = useRef(0);

  const trigger = useCallback((text: string, rect?: { top: number; left: number }) => {
    if (!text.trim()) return;
    setSelectedText(text.trim());
    setResponse(null);
    setLoading(false);
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
  }, []);

  const submit = useCallback(
    (prompt: string) => {
      if (!selectedText || !prompt.trim() || !noteId) return;
      setLoading(true);
      const id = ++requestIdRef.current;

      const content = `${prompt.trim()}\n\nContext:\n${selectedText}`;
      const sessionKey = `notes:inline:${noteId}`;

      ipc<{ content: string }>("chat_send", {
        content,
        sessionKey,
        context: {
          entityKind: "note",
          entityId: noteId,
          isEphemeral: true,
        },
      })
        .then((msg) => {
          if (id === requestIdRef.current) {
            setResponse(msg.content);
          }
        })
        .catch(() => {
          if (id === requestIdRef.current) {
            setResponse("Failed to get a response. Please try again.");
          }
        })
        .finally(() => {
          if (id === requestIdRef.current) {
            setLoading(false);
          }
        });
    },
    [selectedText, noteId],
  );

  const dismiss = useCallback(() => {
    requestIdRef.current++;
    setSelectedText(null);
    setResponse(null);
    setPosition(null);
    setLoading(false);
  }, []);

  return { selectedText, position, response, loading, trigger, submit, dismiss };
}
