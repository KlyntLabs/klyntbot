import { PlanReadyFollowupMessage } from "@app/components/PlanReadyFollowupMessage";
import { RequestUserInputMessage } from "@app/components/RequestUserInputMessage";
import { memo, useCallback, useMemo } from "react";
import type {
  ConversationItem,
  OpenAppTarget,
  RequestUserInputRequest,
  RequestUserInputResponse,
} from "@/types";
import { useFileLinkOpener } from "../hooks/useFileLinkOpener";
import { groupBursts, type BurstGroup } from "../utils/groupBursts";
import { parseReasoning } from "../utils/messageRenderUtils";
import { BurstRow } from "./BurstRow";
import {
  DiffRow,
  ExploreRow,
  MessageRow,
  ReasoningRow,
  ReviewRow,
  ToolRow,
  UserInputRow,
  WorkingIndicator,
} from "./MessageRows";
import { useMessagesViewState } from "./useMessagesViewState";
import { VirtualizedMessageList } from "./VirtualizedMessageList";
import "./Messages.css";

type MessagesProps = {
  items: ConversationItem[];
  threadId: string | null;
  workspaceId?: string | null;
  isThinking: boolean;
  isLoadingMessages?: boolean;
  processingStartedAt?: number | null;
  lastDurationMs?: number | null;
  showPollingFetchStatus?: boolean;
  pollingIntervalMs?: number;
  workspacePath?: string | null;
  openTargets: OpenAppTarget[];
  selectedOpenAppId: string;
  codeBlockCopyUseModifier?: boolean;
  showMessageFilePath?: boolean;
  userInputRequests?: RequestUserInputRequest[];
  onUserInputSubmit?: (
    request: RequestUserInputRequest,
    response: RequestUserInputResponse,
  ) => void;
  onPlanAccept?: () => void;
  onPlanSubmitChanges?: (changes: string) => void;
  onOpenThreadLink?: (threadId: string, workspaceId?: string | null) => void;
  onQuoteMessage?: (text: string) => void;
};

export const Messages = memo(function Messages({
  items,
  threadId,
  workspaceId = null,
  isThinking,
  isLoadingMessages = false,
  processingStartedAt = null,
  lastDurationMs = null,
  showPollingFetchStatus = false,
  pollingIntervalMs = 12000,
  workspacePath = null,
  openTargets,
  selectedOpenAppId,
  codeBlockCopyUseModifier = false,
  showMessageFilePath = true,
  userInputRequests = [],
  onUserInputSubmit,
  onPlanAccept,
  onPlanSubmitChanges,
  onOpenThreadLink,
  onQuoteMessage,
}: MessagesProps) {
  const activeUserInputRequestId =
    threadId && userInputRequests.length
      ? (userInputRequests.find(
          (request) =>
            request.params.thread_id === threadId &&
            (!workspaceId || request.workspace_id === workspaceId),
        )?.request_id ?? null)
      : null;
  const { openFileLink, showFileLinkMenu } = useFileLinkOpener(
    workspacePath,
    openTargets,
    selectedOpenAppId,
  );
  const handleOpenThreadLink = useCallback(
    (threadId: string) => {
      onOpenThreadLink?.(threadId, workspaceId ?? null);
    },
    [onOpenThreadLink, workspaceId],
  );

  const hasActiveUserInputRequest = activeUserInputRequestId !== null;
  const hasVisibleUserInputRequest = hasActiveUserInputRequest && Boolean(onUserInputSubmit);
  const userInputNode =
    hasActiveUserInputRequest && onUserInputSubmit ? (
      <RequestUserInputMessage
        requests={userInputRequests}
        activeThreadId={threadId}
        activeWorkspaceId={workspaceId}
        onSubmit={onUserInputSubmit}
      />
    ) : null;
  const {
    bottomRef,
    containerRef,
    updateAutoScroll,
    requestAutoScroll,
    expandedItems,
    toggleExpanded,
    copiedMessageId,
    handleCopyMessage,
    handleQuoteMessage,
    reasoningMetaById,
    latestReasoningLabel,
    visibleItems,
    planFollowup,
    dismissPlanFollowup,
  } = useMessagesViewState({
    items,
    threadId,
    isThinking,
    activeUserInputRequestId,
    hasVisibleUserInputRequest,
    onPlanAccept,
    onPlanSubmitChanges,
    onQuoteMessage,
  });

  const planFollowupNode =
    planFollowup.shouldShow && onPlanAccept && onPlanSubmitChanges ? (
      <PlanReadyFollowupMessage
        onAccept={() => {
          dismissPlanFollowup();
          onPlanAccept();
        }}
        onSubmitChanges={(changes) => {
          dismissPlanFollowup();
          onPlanSubmitChanges(changes);
        }}
      />
    ) : null;

  const renderItem = (item: ConversationItem) => {
    if (item.kind === "message") {
      const isCopied = copiedMessageId === item.id;
      return (
        <MessageRow
          item={item}
          isCopied={isCopied}
          onCopy={handleCopyMessage}
          onQuote={onQuoteMessage ? handleQuoteMessage : undefined}
          codeBlockCopyUseModifier={codeBlockCopyUseModifier}
          showMessageFilePath={showMessageFilePath}
          workspacePath={workspacePath}
          onOpenFileLink={openFileLink}
          onOpenFileLinkMenu={showFileLinkMenu}
          onOpenThreadLink={handleOpenThreadLink}
        />
      );
    }
    if (item.kind === "reasoning") {
      const parsed = reasoningMetaById.get(item.id) ?? parseReasoning(item);
      return (
        <ReasoningRow
          item={item}
          parsed={parsed}
          showMessageFilePath={showMessageFilePath}
          workspacePath={workspacePath}
          onOpenFileLink={openFileLink}
          onOpenFileLinkMenu={showFileLinkMenu}
          onOpenThreadLink={handleOpenThreadLink}
        />
      );
    }
    if (item.kind === "review") {
      return (
        <ReviewRow
          item={item}
          showMessageFilePath={showMessageFilePath}
          workspacePath={workspacePath}
          onOpenFileLink={openFileLink}
          onOpenFileLinkMenu={showFileLinkMenu}
          onOpenThreadLink={handleOpenThreadLink}
        />
      );
    }
    if (item.kind === "userInput") {
      const isExpanded = expandedItems.has(item.id);
      return (
        <UserInputRow item={item} isExpanded={isExpanded} onToggle={toggleExpanded} />
      );
    }
    if (item.kind === "diff") {
      return <DiffRow item={item} />;
    }
    if (item.kind === "tool") {
      const isExpanded = expandedItems.has(item.id);
      return (
        <ToolRow
          item={item}
          isExpanded={isExpanded}
          onToggle={toggleExpanded}
          showMessageFilePath={showMessageFilePath}
          workspacePath={workspacePath}
          onOpenFileLink={openFileLink}
          onOpenFileLinkMenu={showFileLinkMenu}
          onOpenThreadLink={handleOpenThreadLink}
          onRequestAutoScroll={requestAutoScroll}
        />
      );
    }
    if (item.kind === "explore") {
      return <ExploreRow item={item} />;
    }
    return null;
  };

  const grouped = useMemo(() => groupBursts(visibleItems), [visibleItems]);

  type GroupedEntry = ConversationItem | BurstGroup;

  const renderGroupedEntry = useCallback(
    (entry: GroupedEntry) => {
      if ("kind" in entry && entry.kind === "burst") {
        return (
          <BurstRow
            group={entry}
            expandedItems={expandedItems}
            onToggle={toggleExpanded}
          />
        );
      }
      return renderItem(entry);
    },
    [expandedItems, toggleExpanded],
  );

  const getEntryKey = useCallback(
    (entry: GroupedEntry, index: number) => {
      if ("kind" in entry && entry.kind === "burst") {
        return `burst-${entry.id}`;
      }
      return entry.id ?? `entry-${index}`;
    },
    [],
  );

  return (
    <div className="overflow-y-auto [-webkit-app-region:no-drag] flex-1 min-h-0 min-w-0 messages-full" data-testid="messages-container" ref={containerRef} onScroll={updateAutoScroll}>
      <div className="messages-inner w-full m-0 flex flex-col gap-3.5">
        <VirtualizedMessageList
          items={grouped}
          renderItem={renderGroupedEntry}
          getItemKey={getEntryKey}
          scrollContainerRef={containerRef}
          estimateSize={120}
          trailingContent={
            <>
              {planFollowupNode}
              {userInputNode}
              <WorkingIndicator
                isThinking={isThinking}
                processingStartedAt={processingStartedAt}
                lastDurationMs={lastDurationMs}
                hasItems={items.length > 0}
                reasoningLabel={latestReasoningLabel}
                showPollingFetchStatus={showPollingFetchStatus}
                pollingIntervalMs={pollingIntervalMs}
              />
              {!items.length && !userInputNode && !isThinking && !isLoadingMessages && (
                <div className="empty p-[20px_22px] rounded-[18px] border border-[var(--cm-border-strong)] bg-[var(--cm-surface-panel)] text-text-muted">
                  {threadId ? "Send a prompt to the agent." : "Send a prompt to start a new agent."}
                </div>
              )}
              {!items.length && !userInputNode && !isThinking && isLoadingMessages && (
                <div className="empty p-[20px_22px] rounded-[18px] border border-[var(--cm-border-strong)] bg-[var(--cm-surface-panel)] text-text-muted">
                  <div className="inline-flex items-center gap-[10px] p-0" role="status" aria-live="polite">
                    <span className="working-spinner w-3.5 h-3.5 rounded-full border-2 border-[rgba(255,255,255,0.2)] border-t-text-stronger" aria-hidden />
                    <span className="text-text-muted text-ui-sm tracking-[0.02em]">Loading…</span>
                  </div>
                </div>
              )}
              <div ref={bottomRef} />
            </>
          }
        />
      </div>
    </div>
  );
});
