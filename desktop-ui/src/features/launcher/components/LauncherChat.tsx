import { useEffect, useRef } from "react";
import { useChatSession } from "@/features/chat/hooks/useChatSession";

interface Props {
  initialQuery: string;
  sessionKey: string;
  onBack: () => void;
  onExpandToMain: (sessionKey: string) => void;
}

export function LauncherChat({ initialQuery, sessionKey, onBack, onExpandToMain }: Props) {
  const { messages, isStreaming, input, setInput, send } = useChatSession(sessionKey);
  const sentInitialRef = useRef(false);

  useEffect(() => {
    if (!sentInitialRef.current && initialQuery) {
      sentInitialRef.current = true;
      setInput(initialQuery);
      // Defer send to next tick so setInput is applied
      const id = setTimeout(() => send(), 0);
      return () => clearTimeout(id);
    }
  }, [initialQuery, send, setInput]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onBack();
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) onExpandToMain(sessionKey);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack, onExpandToMain, sessionKey]);

  return (
    <div className="flex flex-col h-full">
      <header className="lc-chat-header">
        <button type="button" className="w-8 h-8 rounded-lg bg-transparent flex items-center justify-center text-text-muted cursor-pointer hover:text-text-strong hover:bg-surface-hover" onClick={onBack} aria-label="Back">
          ←
        </button>
        <span className="text-ui-sm font-semibold text-text-strong px-4 py-3 border-b border-border-subtle">Ask</span>
        <button
          type="button"
          className="w-8 h-8 rounded-lg bg-transparent flex items-center justify-center text-text-muted cursor-pointer hover:text-text-strong hover:bg-surface-hover"
          onClick={() => onExpandToMain(sessionKey)}
          aria-label="Expand"
        >
          ↗
        </button>
      </header>
      <div className="flex-1 overflow-y-auto p-4" role="log" aria-live="polite">
        {messages.map((m) => (
          <div key={m.id} className={`lc-chat-msg lc-chat-msg--${m.role}`}>
            {m.content}
          </div>
        ))}
        {isStreaming && <div className="lc-chat-streaming">…</div>}
      </div>
      <form
        className="lc-chat-composer"
        onSubmit={(e) => {
          e.preventDefault();
          if (input.trim()) {
            void send();
          }
        }}
      >
        <input
          className="flex-1 bg-transparent border-none text-text-primary text-ui-md outline-none px-4 py-3"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Reply… (⌘↵ to expand)"
        />
      </form>
    </div>
  );
}
