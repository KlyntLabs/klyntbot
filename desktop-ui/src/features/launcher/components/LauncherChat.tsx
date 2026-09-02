import { MessageList } from "@features/chat/components/MessageList";
import { useAutoResizeTextarea } from "@shared/hooks/useAutoResizeTextarea";
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
  const { ref: inputRef, handleInput } = useAutoResizeTextarea(chat.input);
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

  // Focus input after streaming completes
  // biome-ignore lint/correctness/useExhaustiveDependencies: inputRef is a stable ref object
  useEffect(() => {
    if (!chat.isStreaming && !chat.activeInteraction) {
      inputRef.current?.focus();
    }
  }, [chat.isStreaming, chat.activeInteraction]);

  // Re-focus input when launcher window regains focus (after hide/show)
  // biome-ignore lint/correctness/useExhaustiveDependencies: inputRef is a stable ref object
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

  // Handle Escape to go back and Cmd+/ to expand
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onBack();
      } else if ((e.metaKey || e.ctrlKey) && e.key === "/") {
        e.preventDefault();
        onExpand();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onBack, onExpand]);

  return (
    <div className="flex flex-col" style={{ height: 568 }}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-separator">
        <button
          type="button"
          onClick={onBack}
          className="flex items-center gap-1.5 text-ui-sm font-light text-fg-secondary hover:text-fg transition-colors"
        >
          <ArrowLeft className="size-3.5" strokeWidth={1.5} />
          Back
        </button>
        <span className="text-ui font-light text-fg">Klynt AI</span>
        <button
          type="button"
          onClick={onExpand}
          className="flex items-center gap-1.5 text-ui-xs font-light text-fg-secondary hover:text-fg transition-colors"
        >
          <ArrowUpRight className="size-3.5" strokeWidth={1.5} />
          Expand
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        <MessageList
          messages={chat.messages}
          segments={chat.segments}
          isStreaming={chat.isStreaming}
          activeTools={chat.activeTools}
          error={chat.error}
          activeInteraction={chat.activeInteraction}
          sessionKey={sessionKey}
          onInteractionSubmitted={chat.clearInteraction}
          liveTransparency={null}
          activeDelegateAgent={chat.activeDelegateAgent}
          statusPhase={chat.statusPhase}
        />
      </div>

      {/* Input */}
      <div className="px-4 pb-3">
        <div className="flex items-center gap-3 glass-input px-4 py-2.5">
          <Sparkles className="w-[16px] h-[16px] text-brand shrink-0" strokeWidth={1.5} />
          <textarea
            ref={inputRef}
            value={chat.input}
            onChange={(e) => chat.setInput(e.target.value)}
            onInput={handleInput}
            onKeyDown={handleKeyDown}
            placeholder="Follow up\u2026"
            aria-label="Message Klynt"
            rows={1}
            className="flex-1 bg-transparent text-fg text-ui placeholder:text-fg-secondary outline-none font-light resize-none max-h-[80px]"
          />
          <button
            type="button"
            onClick={() => chat.send()}
            disabled={!chat.input.trim() || chat.isStreaming}
            className="text-brand hover:text-brand/80 disabled:text-fg-secondary transition-colors shrink-0"
          >
            <Send className="size-4" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      {/* Footer */}
      <div className="px-5 py-2.5 border-t border-separator">
        <div className="flex items-center justify-between text-ui-xs text-fg-secondary">
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
