import type {
  ActiveInteraction,
  ChatMessage,
  MessageSegment,
  TransparencyData,
} from "@shared/types";
import { useEffect, useRef, useState } from "react";
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
  const scrollParentRef = useRef<HTMLElement | null>(null);

  // Find the actual scrollable parent (the div with overflow-y-auto)
  useEffect(() => {
    let el = endRef.current?.parentElement ?? null;
    while (el) {
      const overflow = getComputedStyle(el).overflowY;
      if (overflow === "auto" || overflow === "scroll") {
        scrollParentRef.current = el;
        break;
      }
      el = el.parentElement;
    }
  }, []);

  // Listen for scroll events on the actual scrollable container
  useEffect(() => {
    const sp = scrollParentRef.current;
    if (!sp) return;
    const onScroll = () => {
      const isNearBottom = sp.scrollHeight - sp.scrollTop - sp.clientHeight < 100;
      setUserScrolledUp(!isNearBottom);
    };
    sp.addEventListener("scroll", onScroll, { passive: true });
    return () => sp.removeEventListener("scroll", onScroll);
  }, []);

  // Auto-scroll on new messages/segments unless user scrolled up
  const messageCount = messages.length;
  const segmentCount = segments.length;
  useEffect(() => {
    if (!userScrolledUp && (messageCount > 0 || segmentCount > 0)) {
      endRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messageCount, segmentCount, userScrolledUp]);

  // Continuously scroll during streaming (content grows without count changes)
  useEffect(() => {
    if (!isStreaming || userScrolledUp) return;
    const raf = { id: 0 };
    const tick = () => {
      endRef.current?.scrollIntoView({ behavior: "instant" });
      raf.id = requestAnimationFrame(tick);
    };
    raf.id = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf.id);
  }, [isStreaming, userScrolledUp]);

  return (
    <div className="space-y-6" aria-live="polite">
      {messages.map((msg) => (
        <div
          key={msg.id}
          className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}
          style={{ animation: "fade-in 0.3s ease-out" }}
        >
          {msg.role === "user" ? (
            <div className="max-w-[85%] glass-bubble-user px-5 py-3.5">
              <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-primary">
                {msg.content}
              </p>
            </div>
          ) : msg.role === "interaction" ? (
            <CollapsedInteraction content={msg.content} />
          ) : (
            <div className="max-w-[85%]">
              {msg.segments && msg.segments.length > 0 ? (
                <SegmentedMessage segments={msg.segments} plan={msg.transparency?.plan} />
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
              plan={liveTransparency?.plan}
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
          <div className="glass-bubble px-4 py-3 flex gap-1.5">
            <div
              className="w-1.5 h-1.5 bg-brand/60 rounded-full animate-bounce"
              style={{ animationDelay: "0ms" }}
            />
            <div
              className="w-1.5 h-1.5 bg-brand/60 rounded-full animate-bounce"
              style={{ animationDelay: "150ms" }}
            />
            <div
              className="w-1.5 h-1.5 bg-brand/60 rounded-full animate-bounce"
              style={{ animationDelay: "300ms" }}
            />
          </div>
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className="flex justify-start">
          <div
            className="rounded-xl px-4 py-3"
            style={{
              background: "var(--glass-tint-destructive)",
              border: "1px solid rgba(244, 63, 94, 0.15)",
            }}
          >
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
            className="glass-badge px-4 py-2 text-[11px] text-muted font-light hover:text-secondary hover:bg-surface-raised transition-all"
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
