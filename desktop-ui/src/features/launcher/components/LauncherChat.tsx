import { useEffect, useRef, useState } from "react";
import { useChatSession } from "@/features/chat/hooks/useChatSession";

interface Props {
  initialQuery: string;
  sessionKey: string;
  onBack: () => void;
  onExpandToMain: (sessionKey: string) => void;
}

export function LauncherChat({ initialQuery, sessionKey, onBack, onExpandToMain }: Props) {
  const {
    messages,
    isStreaming,
    input,
    setInput,
    send,
  } = useChatSession(sessionKey);
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
    <div className="lc-chat">
      <header className="lc-chat-header">
        <button className="lc-icon-btn" onClick={onBack} aria-label="Back">←</button>
        <span className="lc-chat-title">Ask</span>
        <button className="lc-icon-btn" onClick={() => onExpandToMain(sessionKey)} aria-label="Expand">↗</button>
      </header>
      <div className="lc-chat-thread" role="log" aria-live="polite">
        {messages.map((m) => (
          <div key={m.id} className={`lc-chat-msg lc-chat-msg--${m.role}`}>
            {m.content}
          </div>
        ))}
        {isStreaming && <div className="lc-chat-streaming">…</div>}
      </div>
      <form className="lc-chat-composer" onSubmit={(e) => {
        e.preventDefault();
        if (input.trim()) { void send(); }
      }}>
        <input
          className="lc-chat-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Reply… (⌘↵ to expand)"
          autoFocus
        />
      </form>
    </div>
  );
}
