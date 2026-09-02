import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import type { ChatMessage, PersonaSegment } from "@shared/types";
import { MessageCircle, Send, Users } from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface InsightChatInputProps {
  messages: ChatMessage[];
  isStreaming: boolean;
  streamingContent: string;
  error: string | null;
  input: string;
  setInput: (value: string) => void;
  send: () => Promise<void>;
  placeholder?: string;
  speakerLabel?: string;
  // Squad/debate live stream
  personaMessages?: PersonaSegment[];
  statusPhase?: string | null;
}

// Tone colors for persona bubbles
const PERSONA_COLORS = [
  { bg: "bg-purple-500/8", label: "text-purple-400/80" },
  { bg: "bg-blue-500/8", label: "text-blue-400/80" },
  { bg: "bg-emerald-500/8", label: "text-emerald-400/80" },
  { bg: "bg-amber-500/8", label: "text-amber-400/80" },
  { bg: "bg-rose-500/8", label: "text-rose-400/80" },
  { bg: "bg-cyan-500/8", label: "text-cyan-400/80" },
];

function getPersonaColor(id: string): (typeof PERSONA_COLORS)[0] {
  let hash = 0;
  for (let i = 0; i < id.length; i++) {
    hash = (hash * 31 + id.charCodeAt(i)) | 0;
  }
  return PERSONA_COLORS[Math.abs(hash) % PERSONA_COLORS.length];
}

export function InsightChatInput({
  messages,
  isStreaming,
  streamingContent,
  error,
  input,
  setInput,
  send,
  placeholder = "Ask a follow-up question...",
  speakerLabel = "AI",
  personaMessages = [],
  statusPhase,
}: InsightChatInputProps) {
  const [expanded, setExpanded] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const hasMessages = messages.length > 0;
  const hasSquad = messages.some((m) => m.personaName) || personaMessages.length > 0;

  useEffect(() => {
    if (hasMessages) setExpanded(true);
  }, [hasMessages]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: scroll on content changes
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages, streamingContent, personaMessages.length]);

  if (!expanded && !hasMessages) {
    return (
      <div className="border-t border-separator pt-3 mt-4">
        <button
          type="button"
          onClick={() => setExpanded(true)}
          className="flex items-center gap-1.5 text-ui-xs text-fg-secondary hover:text-fg transition-colors"
        >
          <MessageCircle size={12} />
          Ask a follow-up question
        </button>
      </div>
    );
  }

  return (
    <div className="border-t border-separator pt-3 mt-4 space-y-3">
      {(hasMessages || isStreaming) && (
        <div className="space-y-2.5">
          {/* Persisted messages */}
          {messages.map((msg) => (
            <ChatBubble
              key={msg.id}
              role={msg.role}
              content={msg.content}
              speakerLabel={speakerLabel}
              personaId={msg.personaId}
              personaName={msg.personaName}
            />
          ))}

          {isStreaming &&
            personaMessages.map((pm) => {
              const color = getPersonaColor(pm.personaId);
              return (
                <div key={pm.personaId} className={`rounded-lg px-3 py-2 ${color.bg}`}>
                  <div className={`text-[9px] font-medium mb-1 ${color.label}`}>
                    {pm.personaName}
                  </div>
                  <div className="text-ui-xs leading-relaxed text-fg-secondary">
                    <MarkdownContent content={pm.content} />
                  </div>
                </div>
              );
            })}

          {/* Streaming indicator */}
          {isStreaming && personaMessages.length === 0 && (
            <div className="rounded-lg bg-bg-elevated/50 px-3 py-2">
              <div className="text-ui-xs leading-relaxed text-fg-secondary">
                {streamingContent ? (
                  <>
                    <MarkdownContent content={streamingContent} />
                    <span className="inline-block w-1.5 h-3 bg-purple animate-pulse ml-0.5 align-text-bottom rounded-sm" />
                  </>
                ) : (
                  <span className="text-fg-dim italic animate-pulse">
                    {statusPhase || (hasSquad ? "Panel is discussing..." : "Thinking...")}
                  </span>
                )}
              </div>
            </div>
          )}

          {/* Squad thinking indicator when persona messages are streaming */}
          {isStreaming && personaMessages.length > 0 && statusPhase && (
            <div className="flex items-center gap-1.5 text-[9px] text-fg-dim animate-pulse px-1">
              <Users size={9} />
              {statusPhase}
            </div>
          )}
        </div>
      )}

      {error && <div className="text-ui-xs text-status-danger">{error}</div>}

      <div className="flex gap-1.5">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              if (!isStreaming && input.trim()) {
                send();
              }
            }
          }}
          placeholder={placeholder}
          disabled={isStreaming}
          className="flex-1 text-ui-xs px-2.5 py-1.5 rounded-lg bg-input border border-separator text-fg placeholder:text-fg-dim focus:outline-none focus:ring-1 focus:ring-purple/30"
        />
        <button
          type="button"
          onClick={() => send()}
          disabled={isStreaming || !input.trim()}
          className="px-2 py-1.5 rounded-lg text-purple hover:bg-purple/10 transition-colors disabled:text-fg-dim"
        >
          <Send size={12} />
        </button>
      </div>

      <div ref={bottomRef} />
    </div>
  );
}

function ChatBubble({
  role,
  content,
  speakerLabel,
  personaId,
  personaName,
}: {
  role: string;
  content: string;
  speakerLabel: string;
  personaId?: string;
  personaName?: string;
}) {
  const isUser = role === "user";
  const isPersona = !isUser && !!personaName;
  const color = isPersona && personaId ? getPersonaColor(personaId) : null;

  const bgClass = isUser ? "bg-purple/8" : color ? color.bg : "bg-bg-elevated/50";
  const labelClass = isUser ? "text-purple/70" : color ? color.label : "text-fg-secondary/70";
  const label = isUser ? "You" : personaName || speakerLabel;

  return (
    <div className={`rounded-lg px-3 py-2 ${bgClass}`}>
      <div className={`text-[9px] font-medium mb-1 ${labelClass}`}>{label}</div>
      <div
        className={`text-ui-xs leading-relaxed ${isUser ? "text-fg" : "text-fg-secondary"}`}
      >
        {isUser ? content : <MarkdownContent content={content} />}
      </div>
    </div>
  );
}
