import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ActiveInteraction,
  ChatMessage,
  MessageSegment,
  TransparencyData,
} from "../../lib/types";
import { CollapsedInteraction } from "./CollapsedInteraction";
import { InteractionCard } from "./InteractionCard";
import { MarkdownContent } from "./MarkdownContent";
import { SegmentedMessage } from "./SegmentedMessage";
import { TokenBadge } from "./TokenBadge";

interface MessageListProps {
  messages: ChatMessage[];
  segments: MessageSegment[];
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: ActiveInteraction | null;
  sessionKey: string;
  onInteractionSubmitted: () => void;
  showTransparency: boolean;
  liveTransparency: TransparencyData | null;
  activeDelegateAgent?: string | null;
}

export function MessageList({
  messages,
  segments,
  isStreaming,
  activeTools,
  error,
  activeInteraction,
  sessionKey,
  onInteractionSubmitted,
  showTransparency,
  liveTransparency,
  activeDelegateAgent,
}: MessageListProps) {
  const endRef = useRef<HTMLDivElement>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll on new messages/streaming unless user scrolled up
  const messageCount = messages.length;
  const segmentCount = segments.length;
  useEffect(() => {
    if (!userScrolledUp && (messageCount > 0 || segmentCount > 0 || isStreaming)) {
      endRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messageCount, segmentCount, isStreaming, userScrolledUp]);

  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const isNearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 100;
    setUserScrolledUp(!isNearBottom);
  }, []);

  return (
    <div ref={containerRef} onScroll={handleScroll} className="space-y-6" aria-live="polite">
      {messages.map((msg) => (
        <div
          key={msg.id}
          className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}
        >
          {msg.role === "user" ? (
            <div className="max-w-[85%] rounded-2xl px-5 py-3.5 bg-surface-raised backdrop-blur-sm">
              <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-primary">
                {msg.content}
              </p>
            </div>
          ) : msg.role === "interaction" ? (
            <CollapsedInteraction content={msg.content} />
          ) : (
            <div className="max-w-[85%]">
              {msg.segments && msg.segments.length > 0 ? (
                <SegmentedMessage segments={msg.segments} />
              ) : (
                <MarkdownContent content={msg.content} />
              )}
              {showTransparency && msg.transparency && (
                <TokenBadge transparency={msg.transparency} />
              )}
            </div>
          )}
        </div>
      ))}

      {/* Streaming segments (live) — includes inline tool spinners + cursor */}
      {(segments.length > 0 || activeTools.length > 0) && (
        <div className="flex justify-start">
          <div className="max-w-[85%]">
            <SegmentedMessage
              segments={segments}
              activeTools={activeTools}
              isStreaming={isStreaming}
              activeDelegateAgent={activeDelegateAgent}
            />
            {showTransparency && liveTransparency && (
              <TokenBadge transparency={liveTransparency} isStreaming={isStreaming} />
            )}
          </div>
        </div>
      )}

      {/* Thinking indicator (streaming but no segments yet and no tools running) */}
      {isStreaming && segments.length === 0 && activeTools.length === 0 && (
        <div className="flex justify-start">
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
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className="flex justify-start">
          <div className="rounded-xl px-4 py-3 bg-destructive/10 border border-destructive/20">
            <p className="text-[12px] font-light text-destructive">{error}</p>
          </div>
        </div>
      )}

      {/* Active interaction prompt */}
      {activeInteraction && (
        <InteractionCard
          sessionKey={sessionKey}
          requestId={activeInteraction.requestId}
          request={activeInteraction.request}
          onSubmitted={onInteractionSubmitted}
        />
      )}

      {userScrolledUp && (
        <div className="sticky bottom-2 flex justify-center">
          <button
            type="button"
            onClick={() => {
              endRef.current?.scrollIntoView({ behavior: "smooth" });
              setUserScrolledUp(false);
            }}
            className="px-3 py-1.5 rounded-full bg-surface-raised text-[11px] text-muted font-light hover:bg-surface-highest transition-colors"
            aria-label="Scroll to bottom"
          >
            Scroll to bottom
          </button>
        </div>
      )}

      <div ref={endRef} />
    </div>
  );
}
