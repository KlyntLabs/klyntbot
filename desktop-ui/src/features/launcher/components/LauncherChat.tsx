import { InteractionCard } from "@features/chat/components/InteractionCard";
import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import { ActiveToolIndicator } from "@features/chat/components/SegmentedMessage";
import { useChatSession } from "@shared/hooks/useChatSession";
import { isTauri } from "@shared/lib/utils";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowLeft, ArrowUpRight, Send, Sparkles } from "lucide-react";
import { useCallback, useEffect, useRef } from "react";

interface LauncherChatProps {
  sessionKey: string;
  initialQuery: string | null;
  onBack: () => void;
  onExpand: () => void;
}

export function LauncherChat({ sessionKey, initialQuery, onBack, onExpand }: LauncherChatProps) {
  const chat = useChatSession(sessionKey);
  const endRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const sentInitial = useRef(false);
  const needsInitialSend = useRef(false);

  // Set initial query input once
  useEffect(() => {
    if (initialQuery && !sentInitial.current) {
      sentInitial.current = true;
      needsInitialSend.current = true;
      chat.setInput(initialQuery);
    }
  }, [initialQuery, chat.setInput]); // eslint-disable-line react-hooks/exhaustive-deps

  // Fire send once when input state has committed with the initial query
  useEffect(() => {
    if (needsInitialSend.current && chat.input) {
      needsInitialSend.current = false;
      chat.send();
    }
  }, [chat.input, chat.send]); // eslint-disable-line react-hooks/exhaustive-deps

  // Auto-scroll on new content
  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  // Focus input after streaming completes
  useEffect(() => {
    if (!chat.isStreaming && !chat.activeInteraction) {
      inputRef.current?.focus();
    }
  }, [chat.isStreaming, chat.activeInteraction]);

  // Re-focus input when launcher window regains focus (after hide/show)
  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) inputRef.current?.focus();
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        chat.send();
      }
    },
    [chat.send],
  );

  // Handle Cmd+/ to expand
  useEffect(() => {
    const handleExpand = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "/") {
        e.preventDefault();
        onExpand();
      }
    };
    window.addEventListener("keydown", handleExpand);
    return () => window.removeEventListener("keydown", handleExpand);
  }, [onExpand]);

  return (
    <div className="flex flex-col" style={{ height: 568 }}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle">
        <button
          type="button"
          onClick={onBack}
          className="flex items-center gap-1.5 text-[12px] font-light text-muted-foreground hover:text-foreground transition-colors"
        >
          <ArrowLeft className="w-3.5 h-3.5" strokeWidth={1.5} />
          Back
        </button>
        <span className="text-[13px] font-light text-foreground">Klynt AI</span>
        <button
          type="button"
          onClick={onExpand}
          className="flex items-center gap-1.5 text-[11px] font-light text-muted-foreground hover:text-foreground transition-colors"
        >
          <ArrowUpRight className="w-3.5 h-3.5" strokeWidth={1.5} />
          Expand
        </button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-4">
        {chat.messages.map((msg) => {
          if (msg.role === "interaction") return null;
          return (
            <div key={msg.id}>
              {msg.role === "user" ? (
                <div className="flex justify-end">
                  <div className="max-w-[85%] glass-bubble-user">
                    <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-foreground">
                      {msg.content}
                    </p>
                  </div>
                </div>
              ) : (
                <div className="max-w-full">
                  <MarkdownContent content={msg.content} />
                </div>
              )}
            </div>
          );
        })}

        {/* Streaming content */}
        {chat.segments.length > 0 && (
          <div className="max-w-full">
            {chat.segments.map((seg, idx) =>
              seg.type === "text" ? (
                <div
                  key={`text-${seg.content.slice(0, 32)}`}
                  className={
                    chat.isStreaming && idx === chat.segments.length - 1 ? "streaming-cursor" : ""
                  }
                >
                  <MarkdownContent content={seg.content} />
                </div>
              ) : null,
            )}
          </div>
        )}

        {/* Active tool spinners */}
        {chat.activeTools.map((name) => (
          <ActiveToolIndicator key={name} name={name} />
        ))}

        {/* Thinking indicator */}
        {chat.isStreaming && chat.segments.length === 0 && chat.activeTools.length === 0 && (
          <div className="flex gap-1">
            <div
              className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce"
              style={{ animationDelay: "0ms" }}
            />
            <div
              className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce"
              style={{ animationDelay: "150ms" }}
            />
            <div
              className="w-1.5 h-1.5 bg-muted rounded-full animate-bounce"
              style={{ animationDelay: "300ms" }}
            />
          </div>
        )}

        {/* Error */}
        {chat.error && (
          <div className="rounded-xl px-4 py-3 bg-destructive/10 border border-destructive/20">
            <p className="text-[12px] font-light text-destructive">{chat.error}</p>
          </div>
        )}

        {/* Interaction card */}
        {chat.activeInteraction && (
          <InteractionCard
            sessionKey={sessionKey}
            requestId={chat.activeInteraction.requestId}
            request={chat.activeInteraction.request}
            onSubmitted={chat.clearInteraction}
          />
        )}

        <div ref={endRef} />
      </div>

      {/* Input */}
      <div className="px-4 pb-3">
        <div className="flex items-center gap-3 glass-input px-4 py-2.5">
          <Sparkles className="w-[16px] h-[16px] text-brand shrink-0" strokeWidth={1.5} />
          <textarea
            ref={inputRef}
            value={chat.input}
            onChange={(e) => {
              chat.setInput(e.target.value);
              e.target.style.height = "auto";
              e.target.style.height = `${Math.min(e.target.scrollHeight, 80)}px`;
            }}
            onKeyDown={handleKeyDown}
            placeholder="Follow up\u2026"
            aria-label="Message Klynt"
            rows={1}
            className="flex-1 bg-transparent text-foreground text-[13px] placeholder:text-muted-foreground outline-none font-light resize-none max-h-[80px]"
          />
          <button
            type="button"
            onClick={() => chat.send()}
            disabled={!chat.input.trim() || chat.isStreaming}
            className="text-brand hover:text-brand/80 disabled:text-muted-foreground transition-colors shrink-0"
          >
            <Send className="w-4 h-4" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      {/* Footer */}
      <div className="px-5 py-2.5 border-t border-border-subtle">
        <div className="flex items-center justify-between text-[11px] text-muted-foreground">
          <span className="flex items-center gap-1.5 font-light">
            <kbd className="px-1.5 py-0.5 glass-badge">Esc</kbd>
            Back to commands
          </span>
          <span className="flex items-center gap-1.5 font-light">
            <kbd className="px-1.5 py-0.5 glass-badge">{"\u2318/"}</kbd>
            Open full chat
          </span>
        </div>
      </div>
    </div>
  );
}
