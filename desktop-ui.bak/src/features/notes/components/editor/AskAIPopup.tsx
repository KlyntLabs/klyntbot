import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { Send } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface AskAIPopupProps {
  selectedText: string;
  position: { top: number; left: number };
  response: string | null;
  loading: boolean;
  onSubmit: (prompt: string) => void;
  onDismiss: () => void;
}

export function AskAIPopup({
  selectedText,
  position,
  response,
  loading,
  onSubmit,
  onDismiss,
}: AskAIPopupProps) {
  const popupRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [prompt, setPrompt] = useState("");

  // Auto-focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Dismiss on click outside
  useEffect(() => {
    function handleMouseDown(e: MouseEvent) {
      if (popupRef.current && !popupRef.current.contains(e.target as Node)) {
        onDismiss();
      }
    }
    document.addEventListener("mousedown", handleMouseDown);
    return () => document.removeEventListener("mousedown", handleMouseDown);
  }, [onDismiss]);

  // Dismiss on Escape
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") onDismiss();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onDismiss]);

  const handleSubmit = () => {
    if (!prompt.trim() || loading) return;
    onSubmit(prompt);
  };

  return createPortal(
    <div
      ref={popupRef}
      className="glass-panel fixed z-50 w-[360px] rounded-xl shadow-2xl"
      style={{ top: position.top, left: position.left }}
    >
      {/* Selected text preview */}
      <div className="px-3 pt-3 pb-2 border-b border-border/50">
        <p className="text-[11px] text-muted-foreground line-clamp-2">{selectedText}</p>
      </div>

      {/* Input area */}
      {!response && (
        <div className="flex items-center gap-2 p-2">
          <input
            ref={inputRef}
            type="text"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSubmit();
            }}
            placeholder="Ask about this text..."
            className="flex-1 bg-transparent text-sm text-primary placeholder:text-muted outline-none"
            disabled={loading}
          />
          {loading ? (
            <ThinkingDots size="sm" />
          ) : (
            <button
              type="button"
              onClick={handleSubmit}
              disabled={!prompt.trim()}
              className="p-1 rounded text-muted hover:text-primary transition-colors disabled:opacity-30"
            >
              <Send size={14} />
            </button>
          )}
        </div>
      )}

      {/* Response area */}
      {response && (
        <div className="p-3 max-h-[200px] overflow-auto">
          <p className="text-sm text-primary leading-relaxed whitespace-pre-wrap">{response}</p>
        </div>
      )}
    </div>,
    document.body,
  );
}
