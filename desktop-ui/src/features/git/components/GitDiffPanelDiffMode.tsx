import Download from "lucide-react/dist/esm/icons/download";
import RotateCcw from "lucide-react/dist/esm/icons/rotate-ccw";
import Upload from "lucide-react/dist/esm/icons/upload";
import type { MouseEvent as ReactMouseEvent } from "react";
import { cn } from "@/utils/cn";
import {
  MagicSparkleIcon,
  MagicSparkleLoaderIcon,
} from "@/features/shared/components/MagicSparkleIcon";
import {
  DEPTH_OPTIONS,
  isGitRootNotFound,
  isMissingRepo,
  normalizeRootPath,
} from "./GitDiffPanel.utils";
import { CommitButton, type DiffFile, DiffSection } from "./GitDiffPanelShared";

type GitDiffModeContentProps = {
  error: string | null | undefined;
  showGitRootPanel: boolean;
  onScanGitRoots?: () => void;
  gitRootScanLoading: boolean;
  gitRootScanDepth: number;
  onGitRootScanDepthChange?: (depth: number) => void;
  onPickGitRoot?: () => void | Promise<void>;
  onInitGitRepo?: () => void | Promise<void>;
  initGitRepoLoading: boolean;
  hasGitRoot: boolean;
  onClearGitRoot?: () => void;
  gitRootScanError: string | null | undefined;
  gitRootScanHasScanned: boolean;
  gitRootCandidates: string[];
  gitRoot: string | null;
  onSelectGitRoot?: (path: string) => void;
  showGenerateCommitMessage: boolean;
  showApplyWorktree: boolean;
  commitMessage: string;
  onCommitMessageChange?: (value: string) => void;
  commitMessageLoading: boolean;
  canGenerateCommitMessage: boolean;
  onGenerateCommitMessage?: () => void | Promise<void>;
  worktreeApplyTitle: string | null;
  worktreeApplyLoading: boolean;
  worktreeApplySuccess: boolean;
  onApplyWorktreeChanges?: () => void | Promise<void>;
  stagedFiles: DiffFile[];
  unstagedFiles: DiffFile[];
  commitLoading: boolean;
  onCommit?: () => void | Promise<void>;
  commitsAhead: number;
  commitsBehind: number;
  onPull?: () => void | Promise<void>;
  pullLoading: boolean;
  onPush?: () => void | Promise<void>;
  pushLoading: boolean;
  onSync?: () => void | Promise<void>;
  syncLoading: boolean;
  onStageAllChanges?: () => void | Promise<void>;
  onStageFile?: (path: string) => Promise<void> | void;
  onUnstageFile?: (path: string) => Promise<void> | void;
  onDiscardFile?: (path: string) => Promise<void> | void;
  onDiscardFiles?: (paths: string[]) => Promise<void> | void;
  onReviewUncommittedChanges?: () => void | Promise<void>;
  selectedFiles: Set<string>;
  selectedPath: string | null;
  onSelectFile?: (path: string) => void;
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
  onDiffListClick: (event: ReactMouseEvent<HTMLDivElement>) => void;
};

export function GitDiffModeContent({
  error,
  showGitRootPanel,
  onScanGitRoots,
  gitRootScanLoading,
  gitRootScanDepth,
  onGitRootScanDepthChange,
  onPickGitRoot,
  onInitGitRepo,
  initGitRepoLoading,
  hasGitRoot,
  onClearGitRoot,
  gitRootScanError,
  gitRootScanHasScanned,
  gitRootCandidates,
  gitRoot,
  onSelectGitRoot,
  showGenerateCommitMessage,
  showApplyWorktree,
  commitMessage,
  onCommitMessageChange,
  commitMessageLoading,
  canGenerateCommitMessage,
  onGenerateCommitMessage,
  worktreeApplyTitle,
  worktreeApplyLoading,
  worktreeApplySuccess,
  onApplyWorktreeChanges,
  stagedFiles,
  unstagedFiles,
  commitLoading,
  onCommit,
  commitsAhead,
  commitsBehind,
  onPull,
  pullLoading,
  onPush,
  pushLoading,
  onSync,
  syncLoading,
  onStageAllChanges,
  onStageFile,
  onUnstageFile,
  onDiscardFile,
  onDiscardFiles,
  onReviewUncommittedChanges,
  selectedFiles,
  selectedPath,
  onSelectFile,
  onFileClick,
  onShowFileMenu,
  onDiffListClick,
}: GitDiffModeContentProps) {
  const normalizedGitRoot = normalizeRootPath(gitRoot);
  const missingRepo = isMissingRepo(error);
  const gitRootNotFound = isGitRootNotFound(error);
  const showInitGitRepo = Boolean(onInitGitRepo) && missingRepo && !gitRootNotFound;
  const gitRootTitle = gitRootNotFound
    ? "Git root folder not found."
    : missingRepo
      ? "This workspace isn't a Git repository yet."
      : "Choose a repo for this workspace.";
  const generateCommitMessageTooltip = "Generate commit message";
  const showWorktreeApplyInUnstaged = showApplyWorktree && unstagedFiles.length > 0;
  const showWorktreeApplyInStaged =
    showApplyWorktree && unstagedFiles.length === 0 && stagedFiles.length > 0;

  return (
    <div
      className="diff-list flex flex-col gap-3 overflow-y-auto flex-1 pr-[2px] min-h-0"
      role="application"
      onClick={onDiffListClick}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onDiffListClick(event as unknown as ReactMouseEvent<HTMLDivElement>);
        }
      }}
    >
      {showGitRootPanel && (
        <div className="git-root-panel flex flex-col gap-3 p-[14px] rounded-[18px] border border-border-default bg-surface-card-soft">
          <div className="git-root-title text-ui-sm font-semibold text-text-strong">
            {gitRootTitle}
          </div>
          {showInitGitRepo && (
            <div className="git-root-primary-action flex">
              <button
                type="button"
                className="primary git-root-button px-[11px] py-[7px] text-ui-sm rounded-full border border-border-default bg-surface-control text-text-emphasis shadow-none transition-[background,border-color,color] duration-ui-fast"
                onClick={() => {
                  void onInitGitRepo?.();
                }}
                disabled={initGitRepoLoading || gitRootScanLoading}
              >
                {initGitRepoLoading ? "Initializing..." : "Initialize Git"}
              </button>
            </div>
          )}
          <div className="git-root-actions flex flex-wrap gap-2 items-center">
            <button
              type="button"
              className="ghost git-root-button px-[11px] py-[7px] text-ui-sm rounded-full border border-border-default bg-surface-control text-text-emphasis shadow-none transition-[background,border-color,color] duration-ui-fast"
              onClick={onScanGitRoots}
              disabled={!onScanGitRoots || gitRootScanLoading || initGitRepoLoading}
            >
              Scan workspace
            </button>
            <label className="git-root-depth inline-flex items-center gap-[6px] text-ui-xs font-semibold text-text-faint">
              <span>Depth</span>
              <select
                className="git-root-select px-[10px] py-[6px] pr-6 rounded-full border border-border-default bg-surface-control text-text-strong text-ui-sm"
                value={gitRootScanDepth}
                onChange={(event) => {
                  const value = Number(event.target.value);
                  if (!Number.isNaN(value)) {
                    onGitRootScanDepthChange?.(value);
                  }
                }}
                disabled={gitRootScanLoading || initGitRepoLoading}
              >
                {DEPTH_OPTIONS.map((depth) => (
                  <option key={depth} value={depth}>
                    {depth}
                  </option>
                ))}
              </select>
            </label>
            {onPickGitRoot && (
              <button
                type="button"
                className="ghost git-root-button px-[11px] py-[7px] text-ui-sm rounded-full border border-border-default bg-surface-control text-text-emphasis shadow-none transition-[background,border-color,color] duration-ui-fast"
                onClick={() => {
                  void onPickGitRoot();
                }}
                disabled={gitRootScanLoading || initGitRepoLoading}
              >
                Pick folder
              </button>
            )}
            {hasGitRoot && onClearGitRoot && (
              <button
                type="button"
                className="ghost git-root-button px-[11px] py-[7px] text-ui-sm rounded-full border border-border-default bg-surface-control text-text-emphasis shadow-none transition-[background,border-color,color] duration-ui-fast"
                onClick={onClearGitRoot}
                disabled={gitRootScanLoading || initGitRepoLoading}
              >
                Use workspace root
              </button>
            )}
          </div>
          {gitRootScanLoading && (
            <div className="text-ui-sm text-text-faint">Scanning for repositories...</div>
          )}
          {!gitRootScanLoading &&
            !gitRootScanError &&
            gitRootScanHasScanned &&
            gitRootCandidates.length === 0 && (
              <div className="text-ui-sm text-text-faint">No repositories found.</div>
            )}
          {gitRootCandidates.length > 0 && (
            <div className="git-root-list flex flex-col gap-[2px]">
              {gitRootCandidates.map((path) => {
                const normalizedPath = normalizeRootPath(path);
                const isActive = normalizedGitRoot && normalizedGitRoot === normalizedPath;
                return (
                  <button
                    key={path}
                    type="button"
                    className={cn(
                      "git-root-item flex items-center gap-2 w-full text-left border-0 bg-transparent text-text-emphasis px-[10px] py-[9px] rounded-xl text-ui-sm shadow-none transition-[background,box-shadow,transform] duration-ui-fast",
                      isActive && "active",
                    )}
                    onClick={() => onSelectGitRoot?.(path)}
                  >
                    <span className="git-root-path min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-text-muted">
                      {path}
                    </span>
                    {isActive && (
                      <span className="git-root-tag text-ui-2xs text-text-muted uppercase tracking-[0.08em]">
                        Active
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}
      {showGenerateCommitMessage && (
        <div className="commit-message-section flex flex-col gap-[10px] p-3 rounded-[18px] border border-border-default bg-surface-card-soft">
          <div className="commit-message-input-wrapper relative">
            <textarea
              className="commit-message-input w-full font-code text-[11px] leading-relaxed text-text-emphasis bg-surface-control border border-border-default rounded-[14px] px-3 py-[11px] pr-10 resize-y min-h-14 max-h-[132px] transition-[background,border-color,color] duration-ui-fast"
              placeholder="Commit message..."
              value={commitMessage}
              onChange={(event) => onCommitMessageChange?.(event.target.value)}
              disabled={commitMessageLoading}
              rows={2}
            />
            <button
              type="button"
              className="commit-message-generate-button diff-row-action ds-tooltip-trigger absolute top-2 right-2 flex items-center justify-center w-[26px] h-[26px] rounded-full border border-border-subtle bg-surface-card-soft text-text-muted cursor-pointer transition-[background,border-color,color] duration-ui-fast"
              onClick={() => {
                if (!canGenerateCommitMessage) {
                  return;
                }
                void onGenerateCommitMessage?.();
              }}
              disabled={commitMessageLoading || !canGenerateCommitMessage}
              title={generateCommitMessageTooltip}
              data-tooltip={generateCommitMessageTooltip}
              data-tooltip-placement="bottom"
              data-tooltip-align="end"
              aria-label="Generate commit message"
            >
              {commitMessageLoading ? (
                <MagicSparkleLoaderIcon className="commit-message-loader w-[14px] h-[14px] animate-spin" />
              ) : (
                <MagicSparkleIcon />
              )}
            </button>
          </div>
          <CommitButton
            commitMessage={commitMessage}
            hasStagedFiles={stagedFiles.length > 0}
            hasUnstagedFiles={unstagedFiles.length > 0}
            commitLoading={commitLoading}
            onCommit={onCommit}
          />
        </div>
      )}
      {(commitsAhead > 0 || commitsBehind > 0) && !stagedFiles.length && (
        <div className="push-section flex flex-col gap-2">
          <div className="push-sync-buttons flex gap-2">
            {commitsBehind > 0 && (
              <button
                type="button"
                className="push-button-secondary flex-1 flex items-center justify-center gap-[6px] px-3 py-[10px] text-ui-sm font-semibold text-text-emphasis bg-surface-row border border-border-default rounded-[14px] cursor-pointer shadow-none transition-[background,border-color] duration-ui-fast"
                onClick={() => void onPull?.()}
                disabled={!onPull || pullLoading || syncLoading}
                title={`Pull ${commitsBehind} commit${commitsBehind > 1 ? "s" : ""}`}
              >
                {pullLoading ? (
                  <span className="commit-button-spinner w-[14px] h-[14px] rounded-full border-2 border-border-subtle border-t-text-emphasis animate-spin" aria-hidden />
                ) : (
                  <Download size={14} aria-hidden />
                )}
                <span>{pullLoading ? "Pulling..." : "Pull"}</span>
                <span className="push-count inline-flex items-center justify-center min-w-[18px] h-[18px] px-[5px] text-ui-2xs font-semibold text-text-emphasis bg-surface-control-hover rounded-[9px]">
                  {commitsBehind}
                </span>
              </button>
            )}
            {commitsAhead > 0 && (
              <button
                type="button"
                className="push-button flex-1 flex items-center justify-center gap-[6px] px-3 py-[10px] text-ui-sm font-semibold text-text-emphasis bg-surface-row border border-border-default rounded-[14px] cursor-pointer shadow-none transition-[background,border-color,color] duration-ui-fast"
                onClick={() => void onPush?.()}
                disabled={!onPush || pushLoading || commitsBehind > 0}
                title={
                  commitsBehind > 0
                    ? "Remote is ahead. Pull first, or use Sync."
                    : `Push ${commitsAhead} commit${commitsAhead > 1 ? "s" : ""}`
                }
              >
                {pushLoading ? (
                  <span className="commit-button-spinner w-[14px] h-[14px] rounded-full border-2 border-border-subtle border-t-text-emphasis animate-spin" aria-hidden />
                ) : (
                  <Upload size={14} aria-hidden />
                )}
                <span>Push</span>
                <span className="push-count inline-flex items-center justify-center min-w-[18px] h-[18px] px-[5px] text-ui-2xs font-semibold text-text-emphasis bg-surface-control-hover rounded-[9px]">
                  {commitsAhead}
                </span>
              </button>
            )}
          </div>
          {commitsAhead > 0 && commitsBehind > 0 && (
            <button
              type="button"
              className="push-button-secondary flex items-center justify-center gap-[6px] px-3 py-[10px] text-ui-sm font-semibold text-text-emphasis bg-surface-row border border-border-default rounded-[14px] cursor-pointer shadow-none transition-[background,border-color] duration-ui-fast"
              onClick={() => void onSync?.()}
              disabled={!onSync || syncLoading || pullLoading}
              title="Pull latest changes and push your local commits"
            >
              {syncLoading ? (
                <span className="commit-button-spinner w-[14px] h-[14px] rounded-full border-2 border-border-subtle border-t-text-emphasis animate-spin" aria-hidden />
              ) : (
                <RotateCcw size={14} aria-hidden />
              )}
              <span>{syncLoading ? "Syncing..." : "Sync (pull then push)"}</span>
            </button>
          )}
        </div>
      )}
      {!error &&
        !stagedFiles.length &&
        !unstagedFiles.length &&
        commitsAhead === 0 &&
        commitsBehind === 0 && (
          <div className="text-ui-sm text-text-faint">No changes detected.</div>
        )}
      {(stagedFiles.length > 0 || unstagedFiles.length > 0) && (
        <>
          {stagedFiles.length > 0 && (
            <DiffSection
              title="Staged"
              files={stagedFiles}
              section="staged"
              selectedFiles={selectedFiles}
              selectedPath={selectedPath}
              onSelectFile={onSelectFile}
              onUnstageFile={onUnstageFile}
              onDiscardFile={onDiscardFile}
              onDiscardFiles={onDiscardFiles}
              showWorktreeApplyAction={showWorktreeApplyInStaged}
              worktreeApplyTitle={worktreeApplyTitle}
              worktreeApplyLoading={worktreeApplyLoading}
              worktreeApplySuccess={worktreeApplySuccess}
              onApplyWorktreeChanges={onApplyWorktreeChanges}
              onFileClick={onFileClick}
              onShowFileMenu={onShowFileMenu}
            />
          )}
          {unstagedFiles.length > 0 && (
            <DiffSection
              title="Unstaged"
              files={unstagedFiles}
              section="unstaged"
              selectedFiles={selectedFiles}
              selectedPath={selectedPath}
              onSelectFile={onSelectFile}
              onStageAllChanges={onStageAllChanges}
              onStageFile={onStageFile}
              onDiscardFile={onDiscardFile}
              onDiscardFiles={onDiscardFiles}
              onReviewUncommittedChanges={onReviewUncommittedChanges}
              showWorktreeApplyAction={showWorktreeApplyInUnstaged}
              worktreeApplyTitle={worktreeApplyTitle}
              worktreeApplyLoading={worktreeApplyLoading}
              worktreeApplySuccess={worktreeApplySuccess}
              onApplyWorktreeChanges={onApplyWorktreeChanges}
              onFileClick={onFileClick}
              onShowFileMenu={onShowFileMenu}
            />
          )}
        </>
      )}
    </div>
  );
}
