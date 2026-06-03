import { getThreadStatusClass, type ThreadStatusById } from "@utils/threadStatus";
import type { CSSProperties, MouseEvent } from "react";
import { memo } from "react";
import { cn } from "@/utils/cn";
import type { ThreadSummary } from "@/types";

function hashString(value: string) {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash * 31 + value.charCodeAt(index)) >>> 0;
  }
  return hash;
}

function getSubagentPillToneStyle(
  workspaceId: string,
  nickname: string | null | undefined,
  role: string | null | undefined,
  threadId: string,
) {
  const identity = [workspaceId, nickname ?? role ?? threadId].join(":");
  const hash = hashString(identity);
  const hue = hash % 360;
  const saturation = 68 + (hash % 12);
  const accent = 52 + ((hash >> 3) % 10);
  return {
    "--thread-subagent-pill-hue": `${hue}`,
    "--thread-subagent-pill-saturation": `${saturation}%`,
    "--thread-subagent-pill-accent": `${accent}%`,
  } as CSSProperties;
}

function formatSubagentRoleLabel(role: string | null | undefined) {
  const normalized = (role ?? "").trim();
  if (!normalized) {
    return null;
  }
  return normalized
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ")
    .replace(/\b\w/g, (match) => match.toUpperCase());
}

type ThreadRowProps = {
  thread: ThreadSummary;
  depth: number;
  workspaceId: string;
  indentUnit: number;
  activeWorkspaceId: string | null;
  activeThreadId: string | null;
  threadStatusById: ThreadStatusById;
  pendingUserInputKeys?: Set<string>;
  workspaceLabel?: string | null;
  getThreadTime: (thread: ThreadSummary) => string | null;
  getThreadArgsBadge?: (workspaceId: string, threadId: string) => string | null;
  isThreadPinned: (workspaceId: string, threadId: string) => boolean;
  onSelectThread: (workspaceId: string, threadId: string) => void;
  onShowThreadMenu: (
    event: MouseEvent,
    workspaceId: string,
    threadId: string,
    canPin: boolean,
  ) => void;
  hasSubagentChildren?: boolean;
  subagentsExpanded?: boolean;
  onToggleSubagents?: (workspaceId: string, threadId: string) => void;
  showPinnedLabel?: boolean;
  dataStatus?: "running" | "recent";
  dataActive?: boolean;
};

export const ThreadRow = memo(function ThreadRow({
  thread,
  depth,
  workspaceId,
  indentUnit,
  activeWorkspaceId,
  activeThreadId,
  threadStatusById,
  pendingUserInputKeys,
  workspaceLabel,
  getThreadTime,
  getThreadArgsBadge,
  isThreadPinned,
  onSelectThread,
  onShowThreadMenu,
  hasSubagentChildren = false,
  subagentsExpanded = true,
  onToggleSubagents,
  showPinnedLabel = true,
  dataStatus,
  dataActive,
}: ThreadRowProps) {
  const relativeTime = getThreadTime(thread);
  const badge = getThreadArgsBadge?.(workspaceId, thread.id) ?? null;
  const modelBadge =
    thread.modelId && thread.modelId.trim().length > 0
      ? thread.effort && thread.effort.trim().length > 0
        ? `${thread.modelId} · ${thread.effort}`
        : thread.modelId
      : null;
  const indentStyle =
    depth > 0 ? ({ "--thread-indent": `${depth * indentUnit}px` } as CSSProperties) : undefined;
  const hasPendingUserInput = Boolean(pendingUserInputKeys?.has(`${workspaceId}:${thread.id}`));
  const statusClass = getThreadStatusClass(threadStatusById[thread.id], hasPendingUserInput);
  const statusLabel =
    statusClass === "reviewing" ? "Reviewing" : hasPendingUserInput ? "Waiting" : null;
  const subagentLabel =
    thread.isSubagent && (thread.subagentNickname || thread.subagentRole)
      ? (thread.subagentNickname ?? thread.subagentRole ?? null)
      : null;
  const subagentTitle =
    thread.subagentNickname && thread.subagentRole
      ? `${thread.subagentNickname} · ${thread.subagentRole}`
      : subagentLabel;
  const subagentRoleLabel =
    thread.subagentNickname && thread.subagentRole
      ? formatSubagentRoleLabel(thread.subagentRole)
      : null;
  const subagentPillStyle = subagentLabel
    ? getSubagentPillToneStyle(workspaceId, thread.subagentNickname, thread.subagentRole, thread.id)
    : undefined;
  const effectiveWorkspaceLabel = depth > 0 ? null : workspaceLabel;
  const contextLabel = badge ?? modelBadge;
  const canPin = depth === 0;
  const isPinned = canPin && isThreadPinned(workspaceId, thread.id);
  const canToggleSubagents = hasSubagentChildren && Boolean(onToggleSubagents);
  const hasDetails = Boolean(
    effectiveWorkspaceLabel || subagentLabel || contextLabel || statusLabel || isPinned,
  );

  return (
    <button
      type="button"
      className={cn(
        "thread-row flex items-center gap-[10px] py-[9px] pr-3 pb-[10px] pl-[calc(10px+var(--thread-indent,0px))] rounded-[14px] bg-[var(--cm-surface-row)] border border-transparent text-text-quiet text-ui-sm text-left cursor-pointer [webkit-app-region:no-drag] min-w-0 relative shadow-[inset_0_1px_0_rgba(255,255,255,0.02),inset_0_0_0_0_rgba(255,255,255,0)] transition-[background-color,border-color,box-shadow,transform] duration-[180ms] ease-out",
        workspaceId === activeWorkspaceId && thread.id === activeThreadId && "active",
        hasDetails && "has-details",
        hasDetails && "has-secondary-line",
        canToggleSubagents && "has-subagent-children",
        depth > 0 && "is-nested",
        isPinned && "is-pinned",
      )}
      style={indentStyle}
      data-status={dataStatus}
      data-active={dataActive ? "true" : undefined}
      onClick={() => onSelectThread(workspaceId, thread.id)}
      onContextMenu={(event) => onShowThreadMenu(event, workspaceId, thread.id, canPin)}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelectThread(workspaceId, thread.id);
        }
      }}
    >
      <span className={cn("thread-status", statusClass)} aria-hidden />
      <div className="thread-content flex-1 min-w-0 flex flex-col gap-1">
        <div className="thread-headline flex items-center gap-2 min-w-0">
          <span className="thread-name flex-1 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-ui-sm font-medium leading-tight">{thread.name}</span>
        </div>
        {hasDetails && (
          <div className="thread-details flex items-center gap-[6px] min-w-0 flex-wrap">
            {effectiveWorkspaceLabel && (
              <span className="thread-workspace-label text-text-muted text-ui-2xs max-w-[140px] overflow-hidden text-ellipsis whitespace-nowrap" title={effectiveWorkspaceLabel}>
                {effectiveWorkspaceLabel}
              </span>
            )}
            {subagentLabel && (
              <span
                className="thread-subagent-pill max-w-[140px] overflow-hidden text-ellipsis whitespace-nowrap"
                title={subagentTitle ?? undefined}
                style={subagentPillStyle}
              >
                {subagentLabel}
              </span>
            )}
            {subagentRoleLabel && (
              <span className="thread-subagent-role text-text-faint text-ui-2xs leading-tight font-mono tracking-[0.03em] uppercase" title={thread.subagentRole ?? undefined}>
                {subagentRoleLabel}
              </span>
            )}
            {statusLabel && (
              <span className={cn("thread-state-chip", statusClass)}>{statusLabel}</span>
            )}
            {contextLabel && (
              <span className="thread-context-label max-w-[140px] overflow-hidden text-ellipsis whitespace-nowrap px-[7px] py-0.5 border border-[var(--cm-border-elevated)] rounded-full bg-[var(--cm-surface-panel-solid)] text-text-muted text-ui-2xs leading-tight" title={contextLabel}>
                {contextLabel}
              </span>
            )}
            {showPinnedLabel && isPinned && <span className="thread-pinned-label max-w-[140px] overflow-hidden text-ellipsis whitespace-nowrap px-[7px] py-0.5 border border-[var(--cm-border-elevated)] rounded-full bg-[var(--cm-surface-panel-solid)] text-text-muted text-ui-2xs leading-tight">Pinned</span>}
          </div>
        )}
      </div>
      <div className="thread-meta ml-auto inline-flex items-center gap-[6px] shrink-0">
        {canToggleSubagents ? (
          <button
            type="button"
            className={cn(
              "thread-subagent-time-toggle border-0 bg-transparent text-text-faint inline-flex items-center justify-end text-ui-2xs leading-tight p-0 [webkit-app-region:no-drag] cursor-pointer min-w-[3ch] relative whitespace-nowrap transition-colors duration-[150ms] ease-out hover:text-text-strong",
              subagentsExpanded && "expanded",
            )}
            onClick={(event) => {
              event.stopPropagation();
              onToggleSubagents?.(workspaceId, thread.id);
            }}
            data-tauri-drag-region="false"
            aria-label={subagentsExpanded ? "Hide sub-agents" : "Show sub-agents"}
            aria-expanded={subagentsExpanded}
          >
            <span className="thread-subagent-time-label inline-block pt-0.5">{relativeTime ?? "Now"}</span>
            <span className="thread-subagent-toggle-icon absolute right-0 inline-flex items-center justify-center opacity-0 pointer-events-none transition-[transform,opacity] duration-[150ms] ease-out" aria-hidden>
              ›
            </span>
          </button>
        ) : (
          relativeTime && <span className="thread-time text-text-faint text-ui-2xs whitespace-nowrap pt-0.5">{relativeTime}</span>
        )}
      </div>
    </button>
  );
});
