import type {
  ActiveInteraction,
  ChatMessage,
  MessageSegment,
  PersonaSegment,
  TransparencyData,
} from "@shared/types";
import type { CommunityCardData, TreePathRef } from "@shared/types/tree-path";
import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { Fragment, useEffect, useRef, useState } from "react";
import { CollapsedInteraction } from "./CollapsedInteraction";
import { CommunityCard } from "./CommunityCard";
import { InteractionCard } from "./InteractionCard";
import { MarkdownContent } from "./MarkdownContent";
import { SegmentedMessage } from "./SegmentedMessage";
import { TokenBadge } from "./TokenBadge";
import { TreePathBreadcrumb } from "./TreePathBreadcrumb";
import { VirtualizedMessageList } from "./VirtualizedMessageList";

interface MessageListProps {
  messages: ChatMessage[];
  segments: MessageSegment[];
  isStreaming: boolean;
  activeTools: string[];
  error: string | null;
  activeInteraction: ActiveInteraction | null;
  sessionKey: string;
  onInteractionSubmitted: () => void;
  liveTransparency: TransparencyData | null;
  activeDelegateAgent?: string | null;
  personaMessages?: PersonaSegment[];
  statusPhase?: string | null;
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
  liveTransparency,
  activeDelegateAgent,
  personaMessages,
  statusPhase,
}: MessageListProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerHeight, setContainerHeight] = useState(600);
  const endRef = useRef<HTMLDivElement>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const scrollParentRef = useRef<HTMLElement | null>(null);
  const isVirtualized = messages.length > 20;

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const obs = new ResizeObserver((entries) => {
      for (const entry of entries) setContainerHeight(entry.contentRect.height);
    });
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

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

  if (isVirtualized) {
    return (
      <div ref={containerRef} className="h-full">
        <VirtualizedMessageList
          messages={messages}
          segments={segments}
          isStreaming={isStreaming}
          activeTools={activeTools}
          error={error}
          activeInteraction={activeInteraction}
          sessionKey={sessionKey}
          onInteractionSubmitted={onInteractionSubmitted}
          liveTransparency={liveTransparency}
          activeDelegateAgent={activeDelegateAgent}
          personaMessages={personaMessages}
          statusPhase={statusPhase}
          height={containerHeight}
        />
      </div>
    );
  }

  return (
    <div className="space-y-6" aria-live="polite">
      {messages.map((msg, idx) => {
        // Insert persona bubbles right after the last user message
        const isLastUser =
          msg.role === "user" && !messages.slice(idx + 1).some((m) => m.role === "user");
        const showPersonas = isLastUser && personaMessages && personaMessages.length > 0;

        return (
          <Fragment key={msg.id}>
            <div
              className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}
              style={{ animation: "fade-in 0.3s ease-out" }}
            >
              {msg.role === "user" ? (
                <div className="max-w-[85%] glass-bubble-user px-5 py-3.5">
                  <p className="text-ui font-light whitespace-pre-wrap leading-relaxed text-fg">
                    {msg.content}
                  </p>
                </div>
              ) : msg.role === "interaction" ? (
                <CollapsedInteraction content={msg.content} />
              ) : (
                <div className="w-full">
                  {msg.segments && msg.segments.length > 0 ? (
                    <SegmentedMessage segments={msg.segments} plan={msg.transparency?.plan} />
                  ) : (
                    <MarkdownContent content={msg.content} />
                  )}
                  {(() => {
                    const paths = (
                      msg as ChatMessage & { metadata?: { treePaths?: TreePathRef[] } }
                    ).metadata?.treePaths;
                    return paths ? <TreePathBreadcrumb treePaths={paths} /> : null;
                  })()}
                  {(() => {
                    const community = (
                      msg as ChatMessage & {
                        metadata?: { communityCard?: CommunityCardData };
                      }
                    ).metadata?.communityCard;
                    return community ? <CommunityCard community={community} /> : null;
                  })()}
                  {msg.transparency && <TokenBadge transparency={msg.transparency} />}
                </div>
              )}
            </div>

            {/* Persona perspectives — inserted after last user message */}
            {showPersonas &&
              personaMessages.map((pm) => (
                <div
                  key={pm.personaId}
                  className="flex justify-start gap-2.5"
                  style={{ animation: "fade-in 0.3s ease-out" }}
                >
                  <div className="shrink-0 size-7 rounded-full bg-purple-500/15 flex items-center justify-center text-sm mt-1">
                    {pm.personaIcon || "🤖"}
                  </div>
                  <div className="max-w-[80%]">
                    <div className="flex items-baseline gap-1.5 mb-1">
                      <span className="text-ui-xs font-medium text-fg">{pm.personaName}</span>
                      {pm.personaRole && (
                        <span className="text-[9px] text-fg-dim">{pm.personaRole}</span>
                      )}
                    </div>
                    <div className="glass-bubble px-4 py-3">
                      <div className="text-ui font-light text-fg-secondary leading-relaxed whitespace-pre-wrap">
                        {pm.content}
                      </div>
                    </div>
                  </div>
                </div>
              ))}
          </Fragment>
        );
      })}

      {/* Streaming segments (live) — includes inline tool spinners + cursor */}
      {(segments.length > 0 || activeTools.length > 0) && (
        <div className="flex justify-start">
          <div className="w-full">
            <SegmentedMessage
              segments={segments}
              activeTools={activeTools}
              isStreaming={isStreaming}
              activeDelegateAgent={activeDelegateAgent}
              plan={liveTransparency?.plan}
            />
            {liveTransparency && <TokenBadge transparency={liveTransparency} />}
          </div>
        </div>
      )}

      {/* Thinking indicator (streaming but no segments yet and no tools running) */}
      {isStreaming && segments.length === 0 && activeTools.length === 0 && (
        <div className="flex justify-start">
          <div className="glass-bubble px-4 py-3 flex items-center gap-2">
            <ThinkingDots size="sm" />
            {statusPhase && <span className="text-ui-sm text-fg-secondary">{statusPhase}</span>}
          </div>
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className="flex justify-start">
          <div
            className="rounded-xl px-4 py-3"
            style={{
              background: "color-mix(in srgb, var(--ds-status-danger) 6%, transparent)",
              border: "1px solid rgba(244, 63, 94, 0.15)",
            }}
          >
            <p className="text-ui-sm font-light text-status-danger">{error}</p>
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
            className="glass-badge px-4 py-2 text-ui-xs text-fg-secondary font-light hover:text-fg hover:bg-control-hover transition-all"
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
