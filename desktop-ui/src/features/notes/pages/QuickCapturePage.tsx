import { ipc } from "@shared/hooks/useIpc";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Inbox } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

export function QuickCapturePage() {
  const [content, setContent] = useState("");
  const [captured, setCaptured] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Auto-focus on mount and when window becomes visible
  useEffect(() => {
    inputRef.current?.focus();
    const handler = () => inputRef.current?.focus();
    window.addEventListener("focus", handler);
    return () => window.removeEventListener("focus", handler);
  }, []);

  const hideWindow = useCallback(() => {
    try {
      getCurrentWindow().hide();
    } catch {
      // In browser mode, just ignore
    }
  }, []);

  const handleSubmit = useCallback(async () => {
    const trimmed = content.trim();
    if (!trimmed) return;

    try {
      await ipc("inbox_create", { params: { content: trimmed } });
      setCaptured(true);
      setContent("");
      setTimeout(() => {
        setCaptured(false);
        hideWindow();
      }, 600);
    } catch {
      // Silently fail
    }
  }, [content, hideWindow]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setContent("");
        hideWindow();
      }
    },
    [handleSubmit, hideWindow],
  );

  return (
    <div className="h-screen w-screen flex items-center justify-center p-4" data-tauri-drag-region>
      <div className="w-full glass-panel rounded-xl p-4 flex flex-col gap-3">
        <div className="flex items-center gap-2 text-sm text-secondary">
          <Inbox className="w-4 h-4 text-brand" />
          <span className="font-medium">Quick Capture</span>
          <span className="ml-auto text-[10px] text-dim">Enter to save · Esc to close</span>
        </div>

        {captured ? (
          <div className="text-center py-4 text-sm text-brand">Captured to inbox!</div>
        ) : (
          <textarea
            ref={inputRef}
            value={content}
            onChange={(e) => setContent(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a quick thought..."
            className="w-full bg-transparent border border-white/[0.08] rounded-lg px-3 py-2 text-sm text-primary placeholder:text-dim focus:outline-none focus:border-brand/40 resize-none"
            rows={3}
          />
        )}
      </div>
    </div>
  );
}
