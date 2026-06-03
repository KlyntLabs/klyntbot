import { openUrl } from "@tauri-apps/plugin-opener";
import { formatRelativeTime } from "@utils/time";
import ChevronDown from "lucide-react/dist/esm/icons/chevron-down";
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import type { MouseEvent as ReactMouseEvent } from "react";
import { useCallback, useEffect, useState } from "react";
import { cn } from "@/utils/cn";
import type { GitHubIssue, GitHubPullRequest, GitLogEntry } from "@/types";
import type { PerFileDiffGroup } from "../utils/perFileThreadDiffs";
import { splitPath } from "./GitDiffPanel.utils";
import { GitLogEntryRow } from "./GitDiffPanelShared";

type GitPerFileModeContentProps = {
  groups: PerFileDiffGroup[];
  selectedPath: string | null;
  onSelectFile?: (path: string) => void;
};

export function GitPerFileModeContent({
  groups,
  selectedPath,
  onSelectFile,
}: GitPerFileModeContentProps) {
  const [collapsedPaths, setCollapsedPaths] = useState<Set<string>>(new Set());

  useEffect(() => {
    setCollapsedPaths((previous) => {
      if (previous.size === 0) {
        return previous;
      }

      const activePaths = new Set(groups.map((group) => group.path));
      let changed = false;
      const next = new Set<string>();

      for (const path of previous) {
        if (activePaths.has(path)) {
          next.add(path);
        } else {
          changed = true;
        }
      }

      if (!changed && next.size === previous.size) {
        return previous;
      }

      return next;
    });
  }, [groups]);

  const toggleGroup = useCallback((path: string) => {
    setCollapsedPaths((previous) => {
      const next = new Set(previous);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  if (groups.length === 0) {
    return <div className="text-ui-sm text-text-faint">No agent edits in this thread yet.</div>;
  }

  return (
    <div className="per-file-tree flex flex-col gap-3 overflow-y-auto overflow-x-hidden flex-1 h-0 min-h-0 pr-[2px] overscroll-contain">
      {groups.map((group) => {
        const isExpanded = !collapsedPaths.has(group.path);
        const { name: fileName } = splitPath(group.path);
        return (
          <div key={group.path} className="per-file-group flex flex-col gap-[6px]">
            <button
              type="button"
              className="per-file-group-row w-full grid items-center gap-2 border-0 bg-transparent text-text-emphasis px-[10px] py-[9px] rounded-xl cursor-pointer text-left shadow-none transition-[background,box-shadow,transform] duration-ui-fast"
              style={{ gridTemplateColumns: "14px minmax(0, 1fr) auto" }}
              onClick={() => toggleGroup(group.path)}
            >
              <span className="per-file-group-chevron inline-flex items-center justify-center text-text-faint" aria-hidden>
                {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
              </span>
              <span className="per-file-group-path text-ui-sm font-semibold min-w-0 overflow-hidden text-ellipsis whitespace-nowrap" title={group.path}>
                {fileName || group.path}
              </span>
              <span className="per-file-group-count text-ui-2xs text-text-faint uppercase tracking-[0.04em]">
                {group.edits.length} edit{group.edits.length === 1 ? "" : "s"}
              </span>
            </button>
            {isExpanded && (
              <div className="per-file-edit-list flex flex-col gap-[2px] pl-[22px]">
                {group.edits.map((edit) => {
                  const isActive = selectedPath === edit.id;
                  return (
                    <button
                      key={edit.id}
                      type="button"
                      className={cn(
                        "per-file-edit-row w-full border-0 rounded-xl bg-transparent text-text-emphasis grid items-center gap-2 px-[10px] py-2 cursor-pointer text-left shadow-none transition-[background,box-shadow,transform] duration-ui-fast",
                        isActive && "active",
                      )}
                      style={{ gridTemplateColumns: "16px minmax(0, 1fr) auto" }}
                      onClick={() => onSelectFile?.(edit.id)}
                    >
                      <span
                        className="per-file-edit-status w-4 h-4 inline-flex items-center justify-center rounded-[4px] text-ui-2xs font-bold border border-border-subtle text-text-faint"
                        data-status={edit.status}
                      >
                        {edit.status}
                      </span>
                      <span className="per-file-edit-label text-ui-sm font-semibold">{edit.label}</span>
                      <span className="per-file-edit-stats inline-flex items-center gap-[6px] text-ui-2xs tabular-nums">
                        {edit.additions > 0 && (
                          <span className="per-file-edit-stat-add text-[#47d488]">
                            +{edit.additions}
                          </span>
                        )}
                        {edit.deletions > 0 && (
                          <span className="per-file-edit-stat-del text-[#ff6b6b]">
                            -{edit.deletions}
                          </span>
                        )}
                        {edit.additions === 0 && edit.deletions === 0 && (
                          <span className="per-file-edit-stat text-text-faint">0</span>
                        )}
                      </span>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

type GitLogModeContentProps = {
  logError: string | null | undefined;
  logLoading: boolean;
  logEntries: GitLogEntry[];
  showAheadSection: boolean;
  showBehindSection: boolean;
  logAheadEntries: GitLogEntry[];
  logBehindEntries: GitLogEntry[];
  selectedCommitSha: string | null;
  onSelectCommit?: (entry: GitLogEntry) => void;
  onShowLogMenu: (event: ReactMouseEvent<HTMLButtonElement>, entry: GitLogEntry) => void;
};

export function GitLogModeContent({
  logError,
  logLoading,
  logEntries,
  showAheadSection,
  showBehindSection,
  logAheadEntries,
  logBehindEntries,
  selectedCommitSha,
  onSelectCommit,
  onShowLogMenu,
}: GitLogModeContentProps) {
  return (
    <div className="git-log-list flex flex-col gap-3 overflow-y-auto flex-1 pr-[2px] min-h-0">
      {!logError && logLoading && (
        <div className="text-ui-xs text-text-faint py-[6px]">Loading commits...</div>
      )}
      {!logError &&
        !logLoading &&
        !logEntries.length &&
        !showAheadSection &&
        !showBehindSection && <div className="text-ui-sm text-text-faint">No commits yet.</div>}
      {showAheadSection && (
        <div className="git-log-section flex flex-col gap-2">
          <div className="git-log-section-title text-ui-xs font-bold tracking-[0.08em] uppercase text-text-faint">
            To push
          </div>
          <div className="git-log-section-list flex flex-col gap-[2px]">
            {logAheadEntries.map((entry) => {
              const isSelected = selectedCommitSha === entry.sha;
              return (
                <GitLogEntryRow
                  key={entry.sha}
                  entry={entry}
                  isSelected={isSelected}
                  compact
                  onSelect={onSelectCommit}
                  onContextMenu={(event) => onShowLogMenu(event, entry)}
                />
              );
            })}
          </div>
        </div>
      )}
      {showBehindSection && (
        <div className="git-log-section flex flex-col gap-2">
          <div className="git-log-section-title text-ui-xs font-bold tracking-[0.08em] uppercase text-text-faint">
            To pull
          </div>
          <div className="git-log-section-list flex flex-col gap-[2px]">
            {logBehindEntries.map((entry) => {
              const isSelected = selectedCommitSha === entry.sha;
              return (
                <GitLogEntryRow
                  key={entry.sha}
                  entry={entry}
                  isSelected={isSelected}
                  compact
                  onSelect={onSelectCommit}
                  onContextMenu={(event) => onShowLogMenu(event, entry)}
                />
              );
            })}
          </div>
        </div>
      )}
      {(logEntries.length > 0 || logLoading) && (
        <div className="git-log-section flex flex-col gap-2">
          <div className="git-log-section-title text-ui-xs font-bold tracking-[0.08em] uppercase text-text-faint">
            Recent commits
          </div>
          <div className="git-log-section-list flex flex-col gap-[2px]">
            {logEntries.map((entry) => {
              const isSelected = selectedCommitSha === entry.sha;
              return (
                <GitLogEntryRow
                  key={entry.sha}
                  entry={entry}
                  isSelected={isSelected}
                  onSelect={onSelectCommit}
                  onContextMenu={(event) => onShowLogMenu(event, entry)}
                />
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

type GitIssuesModeContentProps = {
  issuesError: string | null | undefined;
  issuesLoading: boolean;
  issues: GitHubIssue[];
};

export function GitIssuesModeContent({
  issuesError,
  issuesLoading,
  issues,
}: GitIssuesModeContentProps) {
  return (
    <div className="git-issues-list flex flex-col gap-[10px] overflow-y-auto flex-1 pr-[2px] min-h-0">
      {!issuesError && !issuesLoading && !issues.length && (
        <div className="text-ui-sm text-text-faint">No open issues.</div>
      )}
      {issues.map((issue) => {
        const relativeTime = formatRelativeTime(new Date(issue.updatedAt).getTime());
        return (
          <a
            key={issue.number}
            className="git-issue-entry flex flex-col gap-[6px] px-[10px] py-[9px] border-0 rounded-xl bg-transparent text-inherit no-underline text-left cursor-pointer shadow-none outline-none transition-[background,box-shadow] duration-ui-fast min-w-0"
            href={issue.url}
            onClick={(event) => {
              event.preventDefault();
              void openUrl(issue.url);
            }}
          >
            <div
              className="git-issue-summary grid items-baseline gap-[10px] text-ui-sm text-text-emphasis"
              style={{ gridTemplateColumns: "auto minmax(0, 1fr) auto" }}
            >
              <span className="git-issue-number font-code text-[11px] text-text-faint">
                #{issue.number}
              </span>
              <span className="git-issue-title min-w-0 font-semibold text-text-strong whitespace-normal overflow-wrap-anywhere">
                {issue.title}
              </span>
              <span className="text-text-faint whitespace-nowrap">{relativeTime}</span>
            </div>
          </a>
        );
      })}
    </div>
  );
}

type GitPullRequestsModeContentProps = {
  pullRequestsError: string | null | undefined;
  pullRequestsLoading: boolean;
  pullRequests: GitHubPullRequest[];
  selectedPullRequest: number | null;
  onSelectPullRequest?: (pullRequest: GitHubPullRequest) => void;
  onShowPullRequestMenu: (
    event: ReactMouseEvent<HTMLButtonElement>,
    pullRequest: GitHubPullRequest,
  ) => void;
};

export function GitPullRequestsModeContent({
  pullRequestsError,
  pullRequestsLoading,
  pullRequests,
  selectedPullRequest,
  onSelectPullRequest,
  onShowPullRequestMenu,
}: GitPullRequestsModeContentProps) {
  return (
    <div className="git-pr-list flex flex-col gap-[10px] overflow-y-auto overflow-x-hidden flex-1 pr-[2px] min-h-0">
      {!pullRequestsError && !pullRequestsLoading && !pullRequests.length && (
        <div className="text-ui-sm text-text-faint">No open pull requests.</div>
      )}
      {pullRequests.map((pullRequest) => {
        const relativeTime = formatRelativeTime(new Date(pullRequest.updatedAt).getTime());
        const author = pullRequest.author?.login ?? "unknown";
        const isSelected = selectedPullRequest === pullRequest.number;

        return (
          <button
            type="button"
            key={pullRequest.number}
            className={cn(
              "git-pr-entry flex flex-col gap-[6px] px-[10px] py-[9px] border-0 rounded-xl bg-transparent text-inherit text-left cursor-pointer shadow-none outline-none transition-[background,box-shadow] duration-ui-fast min-w-0",
              isSelected && "active",
            )}
            onClick={() => onSelectPullRequest?.(pullRequest)}
            onContextMenu={(event) => onShowPullRequestMenu(event, pullRequest)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onSelectPullRequest?.(pullRequest);
              }
            }}
          >
            <div className="git-pr-header flex items-start justify-between gap-[10px] min-w-0">
              <span className="git-pr-title inline-flex items-start gap-[6px] min-w-0 text-ui-sm text-text-strong">
                <span className="git-pr-number font-code text-[11px] text-text-faint">
                  #{pullRequest.number}
                </span>
                <span className="git-pr-title-text min-w-0 font-semibold whitespace-normal leading-snug overflow-wrap-anywhere">
                  {pullRequest.title}
                </span>
              </span>
              <span className="git-pr-time text-ui-xs text-text-faint whitespace-nowrap">
                {relativeTime}
              </span>
            </div>
            <div className="git-pr-meta flex flex-wrap gap-[6px] text-ui-xs text-text-faint">
              <span className="git-pr-author-inline font-code text-[11px] text-text-faint">
                @{author}
              </span>
              {pullRequest.isDraft && (
                <span className="git-pr-pill text-ui-2xs text-text-muted px-[7px] py-[3px] rounded-full border border-border-subtle bg-surface-control max-w-full">
                  <span className="git-pr-draft text-text-muted uppercase tracking-[0.08em] text-[9px]">
                    Draft
                  </span>
                </span>
              )}
            </div>
          </button>
        );
      })}
    </div>
  );
}
