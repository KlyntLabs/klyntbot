import { useEffect, useRef } from "react";
import { useChatSession } from "../hooks/useChatSession";
import type { ChatMessage, MessageSegment } from "../types";
import { ChatInput } from "./ChatInput";
import { MessageBubble } from "./MessageBubble";

type ChatPanelProps = {
  sessionKey: string;
  onThreadsChanged: () => void;
};

function segmentsToContent(segments: MessageSegment[]): string {
  return segments
    .filter((s): s is { type: "text"; content: string } => s.type === "text")
    .map((s) => s.content)
    .join("");
}

export function ChatPanel({ sessionKey, onThreadsChanged }: ChatPanelProps) {
  const chat = useChatSession(sessionKey, onThreadsChanged);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  // Auto-scroll to bottom on new messages or streaming segments.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [chat.messages, chat.segments]);

  const showEmpty = chat.messages.length === 0 && !chat.isStreaming && chat.segments.length === 0;
  const streamingText = segmentsToContent(chat.segments);

  return (
    <div className="chat-panel">
      <header className="chat-panel__header">
        <span className="chat-panel__title">Chat</span>
      </header>

      <div ref={scrollRef} className="chat-panel__scroll">
        <div className="chat-panel__list">
          {showEmpty && (
            <div className="chat-panel__empty">
              <p>Start a conversation</p>
              <p className="chat-panel__empty-hint">
                Ask Klynt anything about your tasks, projects, or schedule.
              </p>
            </div>
          )}

          {chat.messages.map((m: ChatMessage) => (
            <MessageBubble key={m.id} message={m} />
          ))}

          {chat.isStreaming && streamingText && (
            <MessageBubble
              message={{
                id: "streaming",
                role: "assistant",
                content: streamingText,
              }}
            />
          )}
        </div>
      </div>

      {chat.error && (
        <div className="chat-panel__error" role="alert">
          {chat.error}
        </div>
      )}

      <ChatInput
        value={chat.input}
        onChange={chat.setInput}
        onSend={() => chat.send()}
        isStreaming={chat.isStreaming}
      />
    </div>
  );
}
