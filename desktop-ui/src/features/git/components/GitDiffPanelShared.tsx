import { formatRelativeTime } from "@utils/time";
import Check from "lucide-react/dist/esm/icons/check";
import Minus from "lucide-react/dist/esm/icons/minus";
import Plus from "lucide-react/dist/esm/icons/plus";
import RotateCcw from "lucide-react/dist/esm/icons/rotate-ccw";
import Upload from "lucide-react/dist/esm/icons/upload";
import X from "lucide-react/dist/esm/icons/x";
import type { MouseEvent as ReactMouseEvent } from "react";
import { cn } from "@/utils/cn";
import { MagicSparkleIcon } from "@/features/shared/components/MagicSparkleIcon";
import type { GitLogEntry } from "@/types";
import {
  getStatusClass,
  getStatusSymbol,
  splitNameAndExtension,
  splitPath,
} from "./GitDiffPanel.utils";

export type DiffFile = {
  path: string;
  status: string;
  additions: number;
  deletions: number;
};

export type SidebarErrorAction = {
  label: string;
  onAction: () => void | Promise<void>;
  disabled?: boolean;
  loading?: boolean;
};

type CommitButtonProps = {
  commitMessage: string;
  hasStagedFiles: boolean;
  hasUnstagedFiles: boolean;
  commitLoading: boolean;
  onCommit?: () => void | Promise<void>;
};

export function CommitButton({
  commitMessage,
  hasStagedFiles,
  hasUnstagedFiles,
  commitLoading,
  onCommit,
}: CommitButtonProps) {
  const hasMessage = commitMessage.trim().length > 0;
  const hasChanges = hasStagedFiles || hasUnstagedFiles;
  const canCommit = hasMessage && hasChanges && !commitLoading;

  const handleCommit = () => {
    if (canCommit) {
      void onCommit?.();
    }
  };

  return (
    <div className="commit-button-container mt-0">
      <button
        type="button"
        className="commit-button w-full flex items-center justify-center gap-[6px] px-[14px] py-[10px] text-ui-sm font-semibold text-text-emphasis bg-surface-control border border-border-default rounded-[14px] cursor-pointer shadow-none transition-[background,border-color,color] duration-ui-fast"
        onClick={handleCommit}
        disabled={!canCommit}
        title={
          !hasMessage
            ? "Enter a commit message"
            : !hasChanges
              ? "No changes to commit"
              : hasStagedFiles
                ? "Commit staged changes"
                : "Commit all unstaged changes"
        }
      >
        {commitLoading ? (
          <span className="commit-button-spinner w-[14px] h-[14px] rounded-full border-2 border-border-subtle border-t-text-emphasis animate-spin" aria-hidden />
        ) : (
          <svg
            width={14}
            height={14}
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden
          >
            <title>Commit</title>
            <path d="M20 6 9 17l-5-5" />
          </svg>
        )}
        <span>{commitLoading ? "Committing..." : "Commit"}</span>
      </button>
    </div>
  );
}

type SidebarErrorProps = {
  variant?: "diff" | "commit";
  message: string;
  action?: SidebarErrorAction | null;
  onDismiss: () => void;
};

export function SidebarError({ variant = "diff", message, action, onDismiss }: SidebarErrorProps) {
  return (
    <div className="sidebar-error flex items-start gap-[6px]">
      <div className="sidebar-error-body flex flex-col gap-[6px] min-w-0 flex-1">
        <div
          className={cn(
            "text-ui-sm",
            variant === "commit"
              ? "commit-message-error text-[rgba(255,160,160,0.9)] py-[2px]"
              : "diff-error text-[rgba(255,160,160,0.9)] whitespace-pre-wrap",
          )}
        >
          {message}
        </div>
        {action && (
          <button
            type="button"
            className="ghost sidebar-error-action self-start inline-flex items-center gap-[6px] px-[10px] py-1 text-ui-sm rounded-lg"
            onClick={() => void action.onAction()}
            disabled={action.disabled || action.loading}
          >
            {action.loading && (
              <span className="commit-button-spinner w-[14px] h-[14px] rounded-full border-2 border-border-subtle border-t-text-emphasis animate-spin" aria-hidden />
            )}
            <span>{action.label}</span>
          </button>
        )}
      </div>
      <button
        type="button"
        className="ghost icon-button sidebar-error-dismiss shrink-0 w-[18px] h-[18px] p-0 rounded text-text-faint hover:text-text-emphasis transition-colors"
        onClick={onDismiss}
        aria-label="Dismiss error"
        title="Dismiss error"
      >
        <X size={12} aria-hidden />
      </button>
    </div>
  );
}

type DiffFileRowProps = {
  file: DiffFile;
  isSelected: boolean;
  isActive: boolean;
  section: "staged" | "unstaged";
  onClick: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  onKeySelect: () => void;
  onContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  onStageFile?: (path: string) => Promise<void> | void;
  onUnstageFile?: (path: string) => Promise<void> | void;
  onDiscardFile?: (path: string) => Promise<void> | void;
};

function DiffFileRow({
  file,
  isSelected,
  isActive,
  section,
  onClick,
  onKeySelect,
  onContextMenu,
  onStageFile,
  onUnstageFile,
  onDiscardFile,
}: DiffFileRowProps) {
  const { name, dir } = splitPath(file.path);
  const { base, extension } = splitNameAndExtension(name);
  const statusSymbol = getStatusSymbol(file.status);
  const statusClass = getStatusClass(file.status);
  const showStage = section === "unstaged" && Boolean(onStageFile);
  const showUnstage = section === "staged" && Boolean(onUnstageFile);
  const showDiscard = section === "unstaged" && Boolean(onDiscardFile);

  return (
    <button
      type="button"
      className={cn(
        "diff-row grid items-center gap-x-2 py-2 px-[10px] rounded-xl cursor-pointer border-0 bg-transparent transition-[background,box-shadow] duration-ui-fast",
        isActive && "active",
        isSelected && "selected",
      )}
      style={{ gridTemplateColumns: "16px minmax(0, 1fr) auto" }}
      onClick={onClick}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onKeySelect();
        }
      }}
      onContextMenu={onContextMenu}
    >
      <span className={cn("diff-icon w-4 h-4 grid place-items-center rounded-full text-ui-2xs font-bold border border-transparent leading-none pb-[1px]", statusClass)} aria-hidden>
        {statusSymbol}
      </span>
      <div className="diff-file flex flex-col gap-[2px] min-w-0">
        <div className="diff-path flex items-baseline gap-[6px] text-ui-xs font-semibold text-text-strong min-w-0">
          <span className="diff-name flex min-w-0 flex-1">
            <span className="diff-name-base min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">
              {base}
            </span>
            {extension && (
              <span className="diff-name-ext shrink-0 whitespace-nowrap">.{extension}</span>
            )}
          </span>
        </div>
        {dir && <div className="diff-dir text-ui-2xs text-text-faint whitespace-nowrap overflow-hidden text-ellipsis">{dir}</div>}
      </div>
      <div className="diff-row-meta inline-flex items-center justify-end justify-self-end min-w-0">
        <span
          className="diff-counts-inline text-[11px] font-code whitespace-nowrap inline-flex items-center gap-[3px] px-[7px] py-[2px] rounded-full border border-border-subtle bg-surface-control tabular-nums text-text-muted"
          role="img"
          aria-label={`+${file.additions} -${file.deletions}`}
        >
          <span className="diff-add text-[#47d488]">+{file.additions}</span>
          <span className="diff-sep text-text-dim">/</span>
          <span className="diff-del text-[#ff6b6b]">-{file.deletions}</span>
        </span>
        <fieldset className="diff-row-actions inline-flex items-center gap-[3px]" aria-label="File actions">
          {showStage && (
            <button
              type="button"
              className="diff-row-action ds-tooltip-trigger w-[22px] h-[22px] rounded-full p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer transition-[background,border-color,color] duration-ui-fast relative hover:!bg-[rgba(71,212,136,0.14)] hover:!border-[rgba(71,212,136,0.35)] hover:!text-[#47d488]"
              onClick={(event) => {
                event.stopPropagation();
                void onStageFile?.(file.path);
              }}
              data-tooltip="Stage Changes"
              data-tooltip-align="end"
              aria-label="Stage file"
            >
              <Plus size={12} aria-hidden />
            </button>
          )}
          {showUnstage && (
            <button
              type="button"
              className="diff-row-action ds-tooltip-trigger w-[22px] h-[22px] rounded-full p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer transition-[background,border-color,color] duration-ui-fast relative hover:!bg-[rgba(245,195,99,0.14)] hover:!border-[rgba(245,195,99,0.35)] hover:!text-[#f5c363]"
              onClick={(event) => {
                event.stopPropagation();
                void onUnstageFile?.(file.path);
              }}
              data-tooltip="Unstage Changes"
              data-tooltip-align="end"
              aria-label="Unstage file"
            >
              <Minus size={12} aria-hidden />
            </button>
          )}
          {showDiscard && (
            <button
              type="button"
              className="diff-row-action ds-tooltip-trigger w-[22px] h-[22px] rounded-full p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer transition-[background,border-color,color] duration-ui-fast relative hover:!bg-[rgba(255,107,107,0.14)] hover:!border-[rgba(255,107,107,0.35)] hover:!text-[#ff6b6b]"
              onClick={(event) => {
                event.stopPropagation();
                void onDiscardFile?.(file.path);
              }}
              data-tooltip="Discard Changes"
              data-tooltip-align="end"
              aria-label="Discard changes"
            >
              <RotateCcw size={12} aria-hidden />
            </button>
          )}
        </fieldset>
      </div>
    </button>
  );
}

type DiffSectionProps = {
  title: string;
  files: DiffFile[];
  section: "staged" | "unstaged";
  selectedFiles: Set<string>;
  selectedPath: string | null;
  onSelectFile?: (path: string) => void;
  onStageAllChanges?: () => Promise<void> | void;
  onStageFile?: (path: string) => Promise<void> | void;
  onUnstageFile?: (path: string) => Promise<void> | void;
  onDiscardFile?: (path: string) => Promise<void> | void;
  onDiscardFiles?: (paths: string[]) => Promise<void> | void;
  onReviewUncommittedChanges?: () => Promise<void> | void;
  showWorktreeApplyAction?: boolean;
  worktreeApplyTitle?: string | null;
  worktreeApplyLoading?: boolean;
  worktreeApplySuccess?: boolean;
  onApplyWorktreeChanges?: () => Promise<void> | void;
  onFileClick: (
    event: ReactMouseEvent<HTMLButtonElement>,
    path: string,
    section: "staged" | "unstaged",
  ) => void;
  onShowFileMenu: (
    event: ReactMouseEvent<HTMLButtonElement>,
    path: string,
    section: "staged" | "unstaged",
  ) => void;
};

export function DiffSection({
  title,
  files,
  section,
  selectedFiles,
  selectedPath,
  onSelectFile,
  onStageAllChanges,
  onStageFile,
  onUnstageFile,
  onDiscardFile,
  onDiscardFiles,
  onReviewUncommittedChanges,
  showWorktreeApplyAction = false,
  worktreeApplyTitle = null,
  worktreeApplyLoading = false,
  worktreeApplySuccess = false,
  onApplyWorktreeChanges,
  onFileClick,
  onShowFileMenu,
}: DiffSectionProps) {
  const filePaths = files.map((file) => file.path);
  const canStageAll =
    section === "unstaged" &&
    (Boolean(onStageAllChanges) || Boolean(onStageFile)) &&
    filePaths.length > 0;
  const canUnstageAll = section === "staged" && Boolean(onUnstageFile) && filePaths.length > 0;
  const canDiscardAll = section === "unstaged" && Boolean(onDiscardFiles) && filePaths.length > 0;
  const canReviewUncommitted =
    section === "unstaged" && Boolean(onReviewUncommittedChanges) && filePaths.length > 0;
  const canApplyWorktree =
    showWorktreeApplyAction && Boolean(onApplyWorktreeChanges) && filePaths.length > 0;
  const showSectionActions =
    canApplyWorktree || canStageAll || canUnstageAll || canDiscardAll || canReviewUncommitted;

  return (
    <div className="diff-section flex flex-col gap-2 p-0 rounded-0 bg-transparent border-0">
      <div className="diff-section-title flex items-center justify-between gap-[10px]">
        <div className="diff-section-heading inline-flex items-center gap-2 min-w-0">
          <span className="diff-section-label text-ui-xs font-bold tracking-[0.08em] uppercase text-text-faint">
            {title}
          </span>
          <span className="diff-section-count inline-flex items-center justify-center min-w-[24px] h-6 px-2 rounded-full border border-border-subtle bg-surface-control font-code text-[11px] tabular-nums text-text-muted">
            {files.length}
          </span>
        </div>
        {showSectionActions && (
          <fieldset className="diff-section-actions inline-flex items-center gap-[6px] ml-2" aria-label={`${title} actions`}>
            {canApplyWorktree && (
              <button
                type="button"
                className="diff-row-action ds-tooltip-trigger w-[22px] h-[22px] rounded-full p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer transition-[background,border-color,color] duration-ui-fast relative hover:!bg-[rgba(90,169,255,0.14)] hover:!border-[rgba(90,169,255,0.35)] hover:!text-[#5aa9ff]"
                onClick={() => {
                  void onApplyWorktreeChanges?.();
                }}
                disabled={worktreeApplyLoading || worktreeApplySuccess}
                data-tooltip={worktreeApplyTitle ?? "Apply changes to parent workspace"}
                data-tooltip-align="end"
                aria-label="Apply worktree changes"
              >
                <WorktreeApplyIcon success={worktreeApplySuccess} />
              </button>
            )}
            {canReviewUncommitted && (
              <button
                type="button"
                className="diff-row-action ds-tooltip-trigger w-[22px] h-[22px] rounded-full p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer transition-[background,border-color,color] duration-ui-fast relative hover:!bg-[rgba(90,169,255,0.14)] hover:!border-[rgba(90,169,255,0.35)] hover:!text-[#5aa9ff]"
                onClick={() => {
                  void onReviewUncommittedChanges?.();
                }}
                data-tooltip="Review Uncommitted Changes"
                data-tooltip-align="end"
                aria-label="Review uncommitted changes"
              >
                <MagicSparkleIcon size={12} />
              </button>
            )}
            {canStageAll && (
              <button
                type="button"
                className="diff-row-action ds-tooltip-trigger w-[22px] h-[22px] rounded-full p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer transition-[background,border-color,color] duration-ui-fast relative hover:!bg-[rgba(71,212,136,0.14)] hover:!border-[rgba(71,212,136,0.35)] hover:!text-[#47d488]"
                onClick={() => {
                  if (onStageAllChanges) {
                    void onStageAllChanges();
                    return;
                  }
                  void Promise.all(filePaths.map((path) => onStageFile?.(path)));
                }}
                data-tooltip="Stage All Changes"
                data-tooltip-align="end"
                aria-label="Stage all changes"
              >
                <Plus size={12} aria-hidden />
              </button>
            )}
            {canUnstageAll && (
              <button
                type="button"
                className="diff-row-action ds-tooltip-trigger w-[22px] h-[22px] rounded-full p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer transition-[background,border-color,color] duration-ui-fast relative hover:!bg-[rgba(245,195,99,0.14)] hover:!border-[rgba(245,195,99,0.35)] hover:!text-[#f5c363]"
                onClick={() => {
                  void Promise.all(filePaths.map((path) => onUnstageFile?.(path)));
                }}
                data-tooltip="Unstage All Changes"
                data-tooltip-align="end"
                aria-label="Unstage all changes"
              >
                <Minus size={12} aria-hidden />
              </button>
            )}
            {canDiscardAll && (
              <button
                type="button"
                className="diff-row-action ds-tooltip-trigger w-[22px] h-[22px] rounded-full p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer transition-[background,border-color,color] duration-ui-fast relative hover:!bg-[rgba(255,107,107,0.14)] hover:!border-[rgba(255,107,107,0.35)] hover:!text-[#ff6b6b]"
                onClick={() => {
                  void onDiscardFiles?.(filePaths);
                }}
                data-tooltip="Discard All Changes"
                data-tooltip-align="end"
                aria-label="Discard all changes"
              >
                <RotateCcw size={12} aria-hidden />
              </button>
            )}
          </fieldset>
        )}
      </div>
      <div className="diff-section-list flex flex-col gap-[2px]">
        {files.map((file) => {
          const isSelected = selectedFiles.size > 1 && selectedFiles.has(file.path);
          const isActive = selectedPath === file.path;
          return (
            <DiffFileRow
              key={`${section}-${file.path}`}
              file={file}
              isSelected={isSelected}
              isActive={isActive}
              section={section}
              onClick={(event) => onFileClick(event, file.path, section)}
              onKeySelect={() => onSelectFile?.(file.path)}
              onContextMenu={(event) => onShowFileMenu(event, file.path, section)}
              onStageFile={onStageFile}
              onUnstageFile={onUnstageFile}
              onDiscardFile={onDiscardFile}
            />
          );
        })}
      </div>
    </div>
  );
}

type GitLogEntryRowProps = {
  entry: GitLogEntry;
  isSelected: boolean;
  compact?: boolean;
  onSelect?: (entry: GitLogEntry) => void;
  onContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
};

export function GitLogEntryRow({
  entry,
  isSelected,
  compact = false,
  onSelect,
  onContextMenu,
}: GitLogEntryRowProps) {
  return (
    <button
      type="button"
      className={cn(
        "git-log-entry flex flex-col gap-[6px] px-[10px] py-[9px] border-0 rounded-xl bg-transparent text-inherit no-underline text-left cursor-pointer shadow-none outline-none transition-[background,box-shadow] duration-ui-fast min-w-0",
        compact && "git-log-entry-compact py-[9px]",
        isSelected && "active",
      )}
      onClick={() => onSelect?.(entry)}
      onContextMenu={onContextMenu}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect?.(entry);
        }
      }}
    >
      <div className="git-log-summary text-ui-sm font-semibold text-text-strong">
        {entry.summary || "No message"}
      </div>
      <div className="git-log-meta flex flex-wrap gap-[6px] text-ui-xs text-text-faint">
        <span className="git-log-sha font-code text-[11px]">{entry.sha.slice(0, 7)}</span>
        <span className="git-log-sep text-text-dim">·</span>
        <span>{entry.author || "Unknown"}</span>
        <span className="git-log-sep text-text-dim">·</span>
        <span>{formatRelativeTime(entry.timestamp * 1000)}</span>
      </div>
    </button>
  );
}

export function WorktreeApplyIcon({ success }: { success: boolean }) {
  if (success) {
    return <Check size={12} aria-hidden />;
  }
  return <Upload size={12} aria-hidden />;
}
