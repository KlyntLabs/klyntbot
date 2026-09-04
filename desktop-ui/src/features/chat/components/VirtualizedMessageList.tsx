import type {
  ActiveInteraction,
  ChatMessage,
  MessageSegment,
  PersonaSegment,
  TransparencyData,
} from "@shared/types";
import type { CommunityCardData, TreePathRef } from "@shared/types/tree-path";
import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { useCallback, useEffect, useRef, useState } from "react";
import { VariableSizeList } from "react-window";
import { CollapsedInteraction } from "./CollapsedInteraction";
import { CommunityCard } from "./CommunityCard";
import { InteractionCard } from "./InteractionCard";
import { MarkdownContent } from "./MarkdownContent";
import { SegmentedMessage } from "./SegmentedMessage";
import { TokenBadge } from "./TokenBadge";
import { TreePathBreadcrumb } from "./TreePathBreadcrumb";

interface VirtualizedMessageListProps {
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
  height: number;
}

// Extra virtual items appended after message rows (streaming/error/interaction area)
const EXTRA_ITEMS = 1;

interface MessageRowProps {
  index: number;
  style: React.CSSProperties;
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
  onHeightChange: (index: number, height: number) => void;
}

function MessageRow({
  index,
  style,
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
  onHeightChange,
}: MessageRowProps) {
  const measureRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = measureRef.current;
    if (!el) return;
    const obs = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const h = entry.contentRect.height + 24; // + mb-6 (1.5rem = 24px)
        onHeightChange(index, h);
      }
    });
    obs.observe(el);
    return () => obs.disconnect();
  }, [index, onHeightChange]);

  // Last virtual item = streaming/error/interaction area
  if (index === messages.length) {
    return (
      <div style={style}>
        <div ref={measureRef}>
          {(segments.length > 0 || activeTools.length > 0) && (
            <div className="flex justify-start mb-6">
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

          {isStreaming && segments.length === 0 && activeTools.length === 0 && (
            <div className="flex justify-start mb-6">
              <div className="glass-bubble px-4 py-3 flex items-center gap-2">
                <ThinkingDots size="sm" />
                {statusPhase && <span className="text-ui-sm text-fg-secondary">{statusPhase}</span>}
              </div>
            </div>
          )}

          {error && (
            <div className="flex justify-start mb-6">
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

          {activeInteraction && (
            <div className="mb-6">
              <InteractionCard
                sessionKey={sessionKey}
                requestId={activeInteraction.requestId}
                request={activeInteraction.request}
                onSubmitted={onInteractionSubmitted}
              />
            </div>
          )}
        </div>
      </div>
    );
  }

  const msg = messages[index];
  const isLastUser =
    msg.role === "user" && !messages.slice(index + 1).some((m) => m.role === "user");
  const showPersonas = isLastUser && personaMessages && personaMessages.length > 0;

  return (
    <div style={style}>
      <div ref={measureRef}>
        <div
          className={`flex mb-6 ${msg.role === "user" ? "justify-end" : "justify-start"}`}
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
                const paths = (msg as ChatMessage & { metadata?: { treePaths?: TreePathRef[] } })
                  .metadata?.treePaths;
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

        {showPersonas &&
          personaMessages.map((pm) => (
            <div
              key={pm.personaId}
              className="flex justify-start gap-2.5 mb-6"
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
      </div>
    </div>
  );
}

export function VirtualizedMessageList({
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
  height,
}: VirtualizedMessageListProps) {
  const listRef = useRef<VariableSizeList>(null);
  const heightMapRef = useRef<Map<number, number>>(new Map());
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const outerRef = useRef<HTMLDivElement>(null);

  const totalItems = messages.length + EXTRA_ITEMS;

  const getItemSize = useCallback((index: number) => {
    return heightMapRef.current.get(index) ?? 80;
  }, []);

  const handleHeightChange = useCallback((index: number, h: number) => {
    const current = heightMapRef.current.get(index);
    if (current !== h) {
      heightMapRef.current.set(index, h);
      listRef.current?.resetAfterIndex(index, false);
    }
  }, []);

  // Auto-scroll to bottom when new content arrives
  const scrollToBottom = useCallback(() => {
    listRef.current?.scrollToItem(totalItems - 1, "end");
  }, [totalItems]);

  useEffect(() => {
    if (!userScrolledUp) {
      scrollToBottom();
    }
  }, [userScrolledUp, scrollToBottom]);

  // Continuously scroll during streaming
  useEffect(() => {
    if (!isStreaming || userScrolledUp) return;
    const raf = { id: 0 };
    const tick = () => {
      listRef.current?.scrollToItem(totalItems - 1, "end");
      raf.id = requestAnimationFrame(tick);
    };
    raf.id = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf.id);
  }, [isStreaming, userScrolledUp, totalItems]);

  // Detect scroll position
  const onScroll = useCallback(
    ({
      scrollOffset,
      scrollUpdateWasRequested,
    }: {
      scrollOffset: number;
      scrollUpdateWasRequested: boolean;
    }) => {
      if (scrollUpdateWasRequested) return;
      const outer = outerRef.current;
      if (!outer) return;
      const isNearBottom = outer.scrollHeight - scrollOffset - outer.clientHeight < 100;
      setUserScrolledUp(!isNearBottom);
    },
    [],
  );

  // react-window render prop
  const renderRow = useCallback(
    ({ index, style }: { index: number; style: React.CSSProperties }) => (
      <MessageRow
        index={index}
        style={style}
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
        onHeightChange={handleHeightChange}
      />
    ),
    [
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
      handleHeightChange,
    ],
  );

  return (
    <div className="relative h-full" aria-live="polite" data-render-path="virtualized">
      <VariableSizeList
        ref={listRef}
        outerRef={outerRef}
        height={height}
        itemCount={totalItems}
        itemSize={getItemSize}
        estimatedItemSize={80}
        overscanCount={5}
        onScroll={onScroll}
        width="100%"
      >
        {renderRow}
      </VariableSizeList>

      {userScrolledUp && (
        <div className="absolute bottom-2 left-0 right-0 flex justify-center">
          <button
            type="button"
            onClick={() => {
              scrollToBottom();
              setUserScrolledUp(false);
            }}
            className="glass-badge px-4 py-2 text-ui-xs text-fg-secondary font-light hover:text-fg hover:bg-control-hover transition-all"
            aria-label="Scroll to bottom"
          >
            Scroll to bottom
          </button>
        </div>
      )}
    </div>
  );
}
