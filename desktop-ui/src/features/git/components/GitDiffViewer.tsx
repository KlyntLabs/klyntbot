import type { SelectedLineRange } from "@pierre/diffs";
import { WorkerPoolContextProvider } from "@pierre/diffs/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ask } from "@tauri-apps/plugin-dialog";
import type { ParsedDiffLine } from "@utils/diff";
import { workerFactory } from "@utils/diffsWorker";
import GitCommitHorizontal from "lucide-react/dist/esm/icons/git-commit-horizontal";
import RotateCcw from "lucide-react/dist/esm/icons/rotate-ccw";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/utils/cn";
import { DIFF_VIEWER_HIGHLIGHTER_OPTIONS } from "@/features/design-system/diff/diffViewerTheme";
import type { PullRequestReviewIntent, PullRequestSelectionRange } from "@/types";
import { splitPath } from "./GitDiffPanel.utils";
import type { GitDiffViewerItem, GitDiffViewerProps } from "./GitDiffViewer.types";
import { calculateDiffStats } from "./GitDiffViewer.utils";
import { DiffCard } from "./GitDiffViewerDiffCard";
import { PullRequestSummary } from "./GitDiffViewerPullRequestSummary";
import { ImageDiffCard } from "./ImageDiffCard";

function isSelectableLine(
  line: ParsedDiffLine,
): line is ParsedDiffLine & { type: "add" | "del" | "context" } {
  return line.type === "add" || line.type === "del" || line.type === "context";
}

function findSelectionLineIndex(
  parsedLines: ParsedDiffLine[],
  lineNumber: number,
  side: "additions" | "deletions",
  fromEnd = false,
) {
  const indices = fromEnd ? [...parsedLines.keys()].reverse() : [...parsedLines.keys()];

  for (const index of indices) {
    const line = parsedLines[index];
    if (!line || !isSelectableLine(line)) {
      continue;
    }
    if (side === "deletions" && line.oldLine === lineNumber) {
      return index;
    }
    if (side === "additions" && line.newLine === lineNumber) {
      return index;
    }
  }

  for (const index of indices) {
    const line = parsedLines[index];
    if (!line || !isSelectableLine(line)) {
      continue;
    }
    if (line.oldLine === lineNumber || line.newLine === lineNumber) {
      return index;
    }
  }

  return null;
}

function buildSelectionRangeFromLineSelection({
  path,
  status,
  parsedLines,
  selectedLines,
}: {
  path: string;
  status: string;
  parsedLines: ParsedDiffLine[];
  selectedLines: SelectedLineRange | null;
}): PullRequestSelectionRange | null {
  if (!selectedLines) {
    return null;
  }

  const startSide = selectedLines.side ?? "additions";
  const endSide = selectedLines.endSide ?? startSide;
  const startIndex = findSelectionLineIndex(parsedLines, selectedLines.start, startSide, false);
  const endIndex = findSelectionLineIndex(parsedLines, selectedLines.end, endSide, true);
  if (startIndex === null || endIndex === null) {
    return null;
  }

  const start = Math.min(startIndex, endIndex);
  const end = Math.max(startIndex, endIndex);
  const lines = parsedLines
    .slice(start, end + 1)
    .filter(isSelectableLine)
    .map((line) => ({
      type: line.type,
      oldLine: line.oldLine,
      newLine: line.newLine,
      text: line.text,
    }));

  if (lines.length === 0) {
    return null;
  }

  return {
    path,
    status,
    start,
    end,
    lines,
  };
}

export function GitDiffViewer({
  diffs,
  selectedPath,
  scrollRequestId,
  isLoading,
  error,
  diffStyle = "split",
  ignoreWhitespaceChanges = false,
  pullRequest,
  pullRequestComments,
  pullRequestCommentsLoading = false,
  pullRequestCommentsError = null,
  pullRequestReviewActions = [],
  onRunPullRequestReview,
  pullRequestReviewLaunching = false,
  pullRequestReviewThreadId = null,
  onCheckoutPullRequest,
  canRevert = false,
  onRevertFile,
  onActivePathChange,
  onInsertComposerText,
}: GitDiffViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const activePathRef = useRef<string | null>(null);
  const ignoreActivePathUntilRef = useRef<number>(0);
  const lastScrollRequestIdRef = useRef<number | null>(null);
  const onActivePathChangeRef = useRef(onActivePathChange);
  const rowResizeObserversRef = useRef(new Map<Element, ResizeObserver>());
  const rowNodesByPathRef = useRef(new Map<string, HTMLDivElement>());

  const hasActivePathHandler = Boolean(onActivePathChange);
  const interactiveSelectionEnabled = Boolean(
    pullRequest &&
      diffStyle === "unified" &&
      onRunPullRequestReview &&
      pullRequestReviewActions.length > 0,
  );
  const [lineSelection, setLineSelection] = useState<{
    path: string;
    range: SelectedLineRange;
  } | null>(null);

  const clearSelection = useCallback(() => {
    setLineSelection(null);
  }, []);

  const selectedLinesForPath = useCallback(
    (path: string) => {
      if (!lineSelection || lineSelection.path !== path) {
        return null;
      }
      return lineSelection.range;
    },
    [lineSelection],
  );

  const setSelectedLinesForPath = useCallback((path: string, range: SelectedLineRange | null) => {
    setLineSelection((previous) => {
      if (!range) {
        if (previous?.path !== path) {
          return previous;
        }
        return null;
      }
      return { path, range };
    });
  }, []);

  const poolOptions = useMemo(() => ({ workerFactory }), []);
  const highlighterOptions = useMemo(() => DIFF_VIEWER_HIGHLIGHTER_OPTIONS, []);

  const indexByPath = useMemo(() => {
    const map = new Map<string, number>();
    diffs.forEach((entry, index) => {
      map.set(entry.path, index);
    });
    return map;
  }, [diffs]);

  const rowVirtualizer = useVirtualizer({
    count: diffs.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => 260,
    overscan: 6,
  });

  const virtualItems = rowVirtualizer.getVirtualItems();

  const setRowRef = useCallback(
    (path: string) => (node: HTMLDivElement | null) => {
      const prevNode = rowNodesByPathRef.current.get(path);
      if (prevNode && prevNode !== node) {
        const prevObserver = rowResizeObserversRef.current.get(prevNode);
        if (prevObserver) {
          prevObserver.disconnect();
          rowResizeObserversRef.current.delete(prevNode);
        }
      }
      if (!node) {
        rowNodesByPathRef.current.delete(path);
        return;
      }
      rowNodesByPathRef.current.set(path, node);
      rowVirtualizer.measureElement(node);
      if (rowResizeObserversRef.current.has(node)) {
        return;
      }
      const observer = new ResizeObserver(() => {
        rowVirtualizer.measureElement(node);
      });
      observer.observe(node);
      rowResizeObserversRef.current.set(node, observer);
    },
    [rowVirtualizer],
  );

  const stickyEntry = useMemo(() => {
    if (!diffs.length) {
      return null;
    }
    if (selectedPath) {
      const index = indexByPath.get(selectedPath);
      if (index !== undefined) {
        return diffs[index];
      }
    }
    return diffs[0];
  }, [diffs, selectedPath, indexByPath]);

  const stickyPathDisplay = useMemo(() => {
    if (!stickyEntry) {
      return null;
    }
    const stickyPath = stickyEntry.displayPath ?? stickyEntry.path;
    const { name, dir } = splitPath(stickyPath);
    return { fileName: name, displayDir: dir ? `${dir}/` : "" };
  }, [stickyEntry]);

  const showRevert = canRevert && Boolean(onRevertFile);

  const handleInsertLineReference = useCallback(
    (entry: GitDiffViewerItem, line: ParsedDiffLine, index: number) => {
      if (!onInsertComposerText) {
        return;
      }
      const displayPath = entry.displayPath ?? entry.path;
      const lineNumber = line.newLine ?? line.oldLine;
      const lineLabel = typeof lineNumber === "number" ? `L${lineNumber}` : `line-${index + 1}`;
      const prefix = line.type === "add" ? "+" : line.type === "del" ? "-" : " ";
      const reference = `${displayPath}:${lineLabel}\n\`\`\`diff\n${prefix}${line.text}\n\`\`\`\n\n`;
      onInsertComposerText(reference);
    },
    [onInsertComposerText],
  );

  const handleRunSelectionReview = useCallback(
    async (
      intent: PullRequestReviewIntent,
      entry: GitDiffViewerItem,
      parsedLines: ParsedDiffLine[],
      selectedLines: SelectedLineRange | null,
    ) => {
      if (!onRunPullRequestReview) {
        return;
      }
      const selection = buildSelectionRangeFromLineSelection({
        path: entry.path,
        status: entry.status,
        parsedLines,
        selectedLines,
      });
      if (!selection) {
        return;
      }
      await onRunPullRequestReview({
        intent,
        selection,
      });
    },
    [onRunPullRequestReview],
  );

  const handleRequestRevert = useCallback(
    async (path: string) => {
      if (!onRevertFile) {
        return;
      }
      const confirmed = await ask(`Discard changes in:\n\n${path}\n\nThis cannot be undone.`, {
        title: "Discard changes",
        kind: "warning",
      });
      if (!confirmed) {
        return;
      }
      await onRevertFile(path);
    },
    [onRevertFile],
  );

  useEffect(() => {
    if (!selectedPath || !scrollRequestId) {
      return;
    }
    if (lastScrollRequestIdRef.current === scrollRequestId) {
      return;
    }
    const index = indexByPath.get(selectedPath);
    if (index === undefined) {
      return;
    }
    ignoreActivePathUntilRef.current = Date.now() + 250;
    rowVirtualizer.scrollToIndex(index, { align: "start" });
    lastScrollRequestIdRef.current = scrollRequestId;
  }, [selectedPath, scrollRequestId, indexByPath, rowVirtualizer]);

  useEffect(() => {
    const observers = rowResizeObserversRef.current;
    return () => {
      for (const observer of observers.values()) {
        observer.disconnect();
      }
      observers.clear();
    };
  }, []);

  useEffect(() => {
    activePathRef.current = selectedPath;
  }, [selectedPath]);

  useEffect(() => {
    if (!interactiveSelectionEnabled) {
      clearSelection();
    }
  }, [clearSelection, interactiveSelectionEnabled]);

  useEffect(() => {
    clearSelection();
  }, [clearSelection]);

  useEffect(() => {
    onActivePathChangeRef.current = onActivePathChange;
  }, [onActivePathChange]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || !hasActivePathHandler) {
      return;
    }
    let frameId: number | null = null;

    const updateActivePath = () => {
      frameId = null;
      if (Date.now() < ignoreActivePathUntilRef.current) {
        return;
      }
      const items = rowVirtualizer.getVirtualItems();
      if (!items.length) {
        return;
      }
      const scrollTop = container.scrollTop;
      const canScroll = container.scrollHeight > container.clientHeight;
      const isAtBottom =
        canScroll && scrollTop + container.clientHeight >= container.scrollHeight - 4;
      let nextPath: string | undefined;
      if (isAtBottom) {
        nextPath = diffs[diffs.length - 1]?.path;
      } else {
        const targetOffset = scrollTop + 8;
        let activeItem = items[0];
        for (const item of items) {
          if (item.start <= targetOffset) {
            activeItem = item;
          } else {
            break;
          }
        }
        nextPath = diffs[activeItem.index]?.path;
      }
      if (!nextPath || nextPath === activePathRef.current) {
        return;
      }
      activePathRef.current = nextPath;
      onActivePathChangeRef.current?.(nextPath);
    };

    const handleScroll = () => {
      if (frameId !== null) {
        return;
      }
      frameId = requestAnimationFrame(updateActivePath);
    };

    handleScroll();
    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => {
      if (frameId !== null) {
        cancelAnimationFrame(frameId);
      }
      container.removeEventListener("scroll", handleScroll);
    };
  }, [diffs, rowVirtualizer, hasActivePathHandler]);

  const diffStats = useMemo(() => calculateDiffStats(diffs), [diffs]);

  const handleScrollToFirstFile = useCallback(() => {
    if (!diffs.length) {
      return;
    }
    const container = containerRef.current;
    const list = listRef.current;
    if (container && list) {
      const top = list.offsetTop;
      container.scrollTo({ top, behavior: "smooth" });
      return;
    }
    rowVirtualizer.scrollToIndex(0, { align: "start" });
  }, [diffs.length, rowVirtualizer]);

  const emptyStateCopy = pullRequest
    ? {
        title: "No file changes in this pull request",
        subtitle:
          "The pull request loaded, but there are no diff hunks to render for this selection.",
        hint: "Try switching to another pull request or commit from the Git panel.",
      }
    : {
        title: "Working tree is clean",
        subtitle: "No local changes were detected for the current workspace.",
        hint: "Make an edit, stage a file, or select a commit to inspect changes here.",
      };

  return (
    <WorkerPoolContextProvider poolOptions={poolOptions} highlighterOptions={highlighterOptions}>
      <div
        className={cn(
          "diff-viewer ds-diff-viewer flex flex-col gap-0 overflow-y-auto relative pt-3 pb-4 flex-1 min-h-0 min-w-0 bg-surface-messages",
          diffStyle === "unified" ? "is-unified" : "is-split",
        )}
        ref={containerRef}
      >
        {pullRequest && (
          <PullRequestSummary
            pullRequest={pullRequest}
            hasDiffs={diffs.length > 0}
            diffStats={diffStats}
            onJumpToFirstFile={handleScrollToFirstFile}
            pullRequestComments={pullRequestComments}
            pullRequestCommentsLoading={pullRequestCommentsLoading}
            pullRequestCommentsError={pullRequestCommentsError}
            onCheckoutPullRequest={onCheckoutPullRequest}
          />
        )}
        {!error && stickyEntry && (
          <div className="diff-viewer-sticky">
            <div className="diff-viewer-header diff-viewer-header-sticky flex items-center gap-2 text-ui-sm text-text-muted px-3 py-[10px] m-0 border-b border-border-subtle">
              <span
                className="diff-viewer-status font-bold text-ui-xs px-[6px] py-[2px] rounded-full border border-border-stronger text-text-stronger bg-surface-control uppercase"
                data-status={stickyEntry.status}
              >
                {stickyEntry.status}
              </span>
              <span
                className="diff-viewer-path inline-flex items-baseline gap-[6px] flex-1 min-w-0 break-words"
                title={stickyEntry.displayPath ?? stickyEntry.path}
              >
                <span className="diff-viewer-name text-text-emphasis font-semibold min-w-0 shrink-[1] grow-0 basis-auto overflow-hidden text-ellipsis whitespace-nowrap">
                  {stickyPathDisplay?.fileName ?? stickyEntry.path}
                </span>
                {stickyPathDisplay?.displayDir && (
                  <span className="diff-viewer-dir text-text-faint flex-1 overflow-hidden text-ellipsis whitespace-nowrap min-w-0">
                    {stickyPathDisplay.displayDir}
                  </span>
                )}
              </span>
              {showRevert && (
                <button
                  type="button"
                  className="w-6 h-6 rounded-md p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer shrink-0 transition-[background,border-color,color] duration-ui-fast hover:bg-surface-control-hover hover:border-border-subtle hover:text-text-emphasis hover:!bg-[rgba(255,107,107,0.14)] hover:!border-[rgba(255,107,107,0.35)] hover:!text-[#ff6b6b] focus-visible:outline-2 focus-visible:outline-border-accent-soft focus-visible:outline-offset-2"
                  title="Discard changes in this file"
                  aria-label="Discard changes in this file"
                  onClick={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    void handleRequestRevert(stickyEntry.displayPath ?? stickyEntry.path);
                  }}
                >
                  <RotateCcw size={14} aria-hidden />
                </button>
              )}
            </div>
          </div>
        )}
        {error && <div className="text-ui-sm text-text-faint px-4 py-2">{error}</div>}
        {!error && isLoading && diffs.length > 0 && (
          <div className="diff-viewer-loading diff-viewer-loading-overlay text-ui-xs text-text-faint py-[6px]">
            Refreshing diff...
          </div>
        )}
        {!error && !isLoading && !diffs.length && (
          <div
            className="diff-viewer-empty-state relative flex-1 min-h-[240px] flex flex-col items-center justify-center gap-2 px-4 pt-6 pb-[30px] text-center"
            role="status"
            aria-live="polite"
          >
            <div className="diff-viewer-empty-glow" aria-hidden />
            <span
              className="diff-viewer-empty-icon relative w-[34px] h-[34px] rounded-full inline-flex items-center justify-center text-text-emphasis"
              aria-hidden
            >
              <GitCommitHorizontal size={18} />
            </span>
            <h3 className="diff-viewer-empty-title text-ui-xl leading-snug text-text-emphasis tracking-[0.01em]">
              {emptyStateCopy.title}
            </h3>
            <p className="diff-viewer-empty-subtitle text-ui-sm text-text-subtle leading-snug max-w-[560px]">
              {emptyStateCopy.subtitle}
            </p>
            <p className="diff-viewer-empty-hint text-ui-sm text-text-faint leading-normal max-w-[560px]">
              {emptyStateCopy.hint}
            </p>
          </div>
        )}
        {!error && diffs.length > 0 && (
          <div
            className="diff-viewer-list relative w-full"
            ref={listRef}
            style={{
              height: rowVirtualizer.getTotalSize(),
            }}
          >
            {virtualItems.map((virtualRow) => {
              const entry = diffs[virtualRow.index];
              return (
                <div
                  key={entry.path}
                  className="diff-viewer-row absolute left-0 top-0 w-full pb-0"
                  style={{
                    willChange: "transform",
                    isolation: "isolate",
                    transform: `translate3d(0, ${virtualRow.start}px, 0)`,
                  }}
                  data-index={virtualRow.index}
                  ref={setRowRef(entry.path)}
                >
                  {entry.isImage ? (
                    <ImageDiffCard
                      path={entry.path}
                      status={entry.status}
                      oldImageData={entry.oldImageData}
                      newImageData={entry.newImageData}
                      oldImageMime={entry.oldImageMime}
                      newImageMime={entry.newImageMime}
                      isSelected={entry.path === selectedPath}
                      showRevert={showRevert}
                      onRequestRevert={(path) => void handleRequestRevert(path)}
                    />
                  ) : (
                    <DiffCard
                      entry={entry}
                      isSelected={entry.path === selectedPath}
                      diffStyle={diffStyle}
                      isLoading={isLoading}
                      ignoreWhitespaceChanges={ignoreWhitespaceChanges}
                      showRevert={showRevert}
                      onRequestRevert={(path) => void handleRequestRevert(path)}
                      interactiveSelectionEnabled={interactiveSelectionEnabled}
                      selectedLines={selectedLinesForPath(entry.path)}
                      onSelectedLinesChange={(range) => {
                        setSelectedLinesForPath(entry.path, range);
                      }}
                      onLineAction={
                        onInsertComposerText
                          ? (line, index) => {
                              handleInsertLineReference(entry, line, index);
                            }
                          : undefined
                      }
                      reviewActions={pullRequestReviewActions}
                      onRunReviewAction={(intent, parsedLines, selectedLines) => {
                        void handleRunSelectionReview(intent, entry, parsedLines, selectedLines);
                      }}
                      onClearSelection={clearSelection}
                      pullRequestReviewLaunching={pullRequestReviewLaunching}
                      pullRequestReviewThreadId={pullRequestReviewThreadId}
                    />
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </WorkerPoolContextProvider>
  );
}
