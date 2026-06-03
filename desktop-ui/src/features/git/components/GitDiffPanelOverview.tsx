import ArrowLeftRight from "lucide-react/dist/esm/icons/arrow-left-right";
import RotateCw from "lucide-react/dist/esm/icons/rotate-cw";
import type { GitPanelMode } from "../types";

type GitMode = GitPanelMode;

type GitPanelModeStatusProps = {
  mode: GitMode;
  diffStatusLabel: string;
  perFileDiffStatusLabel: string;
  logCountLabel: string;
  logSyncLabel: string;
  logUpstreamLabel: string;
  issuesLoading: boolean;
  issuesTotal: number;
  pullRequestsLoading: boolean;
  pullRequestsTotal: number;
};

export function GitPanelModeStatus({
  mode,
  diffStatusLabel,
  perFileDiffStatusLabel,
  logCountLabel,
  logSyncLabel,
  logUpstreamLabel,
  issuesLoading,
  issuesTotal,
  pullRequestsLoading,
  pullRequestsTotal,
}: GitPanelModeStatusProps) {
  if (mode === "diff") {
    return (
      <div className="text-ui-xs font-medium text-text-muted">{diffStatusLabel}</div>
    );
  }

  if (mode === "perFile") {
    return (
      <div className="text-ui-xs font-medium text-text-muted">{perFileDiffStatusLabel}</div>
    );
  }

  if (mode === "log") {
    return (
      <>
        <div className="text-ui-xs font-medium text-text-muted">{logCountLabel}</div>
        <div className="git-log-sync inline-flex flex-wrap gap-[6px] text-ui-xs text-text-faint">
          <span>{logSyncLabel}</span>
          {logUpstreamLabel && (
            <>
              <span className="text-text-dim">·</span>
              <span>{logUpstreamLabel}</span>
            </>
          )}
        </div>
      </>
    );
  }

  if (mode === "issues") {
    return (
      <>
        <div className="diff-status-issues inline-flex items-center gap-[6px] text-ui-xs font-medium text-text-muted">
          <span>GitHub issues</span>
          {issuesLoading && <span className="git-panel-spinner" aria-hidden />}
        </div>
        <div className="git-log-sync inline-flex flex-wrap gap-[6px] text-ui-xs text-text-faint">
          <span>{issuesTotal} open</span>
        </div>
      </>
    );
  }

  return (
    <>
      <div className="diff-status-issues inline-flex items-center gap-[6px] text-ui-xs font-medium text-text-muted">
        <span>GitHub pull requests</span>
        {pullRequestsLoading && <span className="git-panel-spinner" aria-hidden />}
      </div>
      <div className="git-log-sync inline-flex flex-wrap gap-[6px] text-ui-xs text-text-faint">
        <span>{pullRequestsTotal} open</span>
      </div>
    </>
  );
}

type GitBranchRowProps = {
  mode: GitMode;
  branchName: string;
  onFetch?: () => void | Promise<void>;
  fetchLoading: boolean;
};

export function GitBranchRow({ mode, branchName, onFetch, fetchLoading }: GitBranchRowProps) {
  if (mode !== "diff" && mode !== "perFile" && mode !== "log") {
    return null;
  }

  return (
    <div className="diff-branch-row flex items-center justify-between gap-[10px] min-w-0 pt-2 border-t border-border-subtle">
      <div className="diff-branch-meta flex flex-col gap-[2px] min-w-0">
        <span className="text-ui-2xs font-semibold tracking-[0.08em] uppercase text-text-faint">
          Branch
        </span>
        <div className="text-ui-sm font-semibold text-text-strong">
          {branchName || "unknown"}
        </div>
      </div>
      <button
        type="button"
        className="diff-branch-refresh inline-flex items-center justify-center w-7 h-7 rounded-full border border-border-default bg-surface-control text-text-muted cursor-pointer p-0 shrink-0 shadow-none transition-[background,border-color,color] duration-ui-fast"
        onClick={() => void onFetch?.()}
        disabled={!onFetch || fetchLoading}
        title={fetchLoading ? "Fetching remote..." : "Fetch remote"}
        aria-label={fetchLoading ? "Fetching remote" : "Fetch remote"}
      >
        {fetchLoading ? (
          <span className="git-panel-spinner" aria-hidden />
        ) : (
          <RotateCw size={12} aria-hidden />
        )}
      </button>
    </div>
  );
}

type GitRootCurrentPathProps = {
  mode: GitMode;
  hasGitRoot: boolean;
  gitRoot: string | null;
  onScanGitRoots?: () => void;
  gitRootScanLoading: boolean;
};

export function GitRootCurrentPath({
  mode,
  hasGitRoot,
  gitRoot,
  onScanGitRoots,
  gitRootScanLoading,
}: GitRootCurrentPathProps) {
  if (mode === "issues" || !hasGitRoot) {
    return null;
  }

  return (
    <div className="git-root-current flex items-center justify-between gap-[10px] min-w-0 pt-2 border-t border-border-subtle text-ui-xs text-text-faint">
      <div className="git-root-current-main flex flex-col gap-[3px] min-w-0">
        <span className="git-root-label text-ui-2xs font-semibold tracking-[0.08em] uppercase text-text-faint">
          Repository root
        </span>
        <span className="git-root-path min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-text-muted" title={gitRoot ?? ""}>
          {gitRoot}
        </span>
      </div>
      {onScanGitRoots && (
        <button
          type="button"
          className="ghost git-root-button inline-flex items-center gap-[6px] px-[11px] py-[7px] text-ui-sm rounded-full border border-border-default bg-surface-control text-text-emphasis shadow-none transition-[background,border-color,color] duration-ui-fast"
          onClick={onScanGitRoots}
          disabled={gitRootScanLoading}
        >
          <ArrowLeftRight className="git-root-button-icon w-3 h-3" aria-hidden />
          Change
        </button>
      )}
    </div>
  );
}
