import {
  type AnnotationSide,
  type FileDiffMetadata,
  parsePatchFiles,
  type SelectedLineRange,
} from "@pierre/diffs";
import { FileDiff } from "@pierre/diffs/react";
import { type ParsedDiffLine, parseDiff } from "@utils/diff";
import { highlightLine, languageFromPath } from "@utils/syntax";
import RotateCcw from "lucide-react/dist/esm/icons/rotate-ccw";
import { memo, useMemo } from "react";
import { cn } from "@/utils/cn";
import { DIFF_VIEWER_SCROLL_CSS } from "@/features/design-system/diff/diffViewerTheme";
import type { PullRequestReviewAction, PullRequestReviewIntent } from "@/types";
import { splitPath } from "./GitDiffPanel.utils";
import type { GitDiffViewerItem } from "./GitDiffViewer.types";
import {
  isFallbackRawDiffLineHighlightable,
  normalizePatchName,
  parseRawDiffLines,
} from "./GitDiffViewer.utils";

type HoveredDiffLine =
  | {
      lineNumber: number;
      side?: AnnotationSide;
      annotationSide?: AnnotationSide;
    }
  | undefined;

function isSelectableLine(
  line: ParsedDiffLine,
): line is ParsedDiffLine & { type: "add" | "del" | "context" } {
  return line.type === "add" || line.type === "del" || line.type === "context";
}

function resolveParsedLineForHover(
  parsedLines: ParsedDiffLine[],
  hovered: HoveredDiffLine,
): { line: ParsedDiffLine; index: number } | null {
  if (!hovered) {
    return null;
  }
  const side = hovered.annotationSide ?? hovered.side ?? "additions";
  const lineNumber = hovered.lineNumber;

  const matchForSide = (line: ParsedDiffLine) => {
    if (!isSelectableLine(line)) {
      return false;
    }
    if (side === "deletions") {
      return line.oldLine === lineNumber;
    }
    return line.newLine === lineNumber;
  };

  let index = parsedLines.findIndex(matchForSide);
  if (index >= 0) {
    return { line: parsedLines[index], index };
  }

  index = parsedLines.findIndex(
    (line) =>
      isSelectableLine(line) && (line.newLine === lineNumber || line.oldLine === lineNumber),
  );
  if (index >= 0) {
    return { line: parsedLines[index], index };
  }

  return null;
}

export type DiffCardProps = {
  entry: GitDiffViewerItem;
  isSelected: boolean;
  diffStyle: "split" | "unified";
  isLoading: boolean;
  ignoreWhitespaceChanges: boolean;
  showRevert: boolean;
  onRequestRevert?: (path: string) => void;
  interactiveSelectionEnabled: boolean;
  selectedLines?: SelectedLineRange | null;
  onSelectedLinesChange?: (range: SelectedLineRange | null) => void;
  onLineAction?: (line: ParsedDiffLine, index: number) => void;
  reviewActions?: PullRequestReviewAction[];
  onRunReviewAction?: (
    intent: PullRequestReviewIntent,
    parsedLines: ParsedDiffLine[],
    selectedLines: SelectedLineRange | null,
  ) => void | Promise<void>;
  onClearSelection?: () => void;
  pullRequestReviewLaunching?: boolean;
  pullRequestReviewThreadId?: string | null;
};

export const DiffCard = memo(function DiffCard({
  entry,
  isSelected,
  diffStyle,
  isLoading,
  ignoreWhitespaceChanges,
  showRevert,
  onRequestRevert,
  interactiveSelectionEnabled,
  selectedLines = null,
  onSelectedLinesChange,
  onLineAction,
  reviewActions = [],
  onRunReviewAction,
  onClearSelection,
  pullRequestReviewLaunching = false,
  pullRequestReviewThreadId = null,
}: DiffCardProps) {
  const displayPath = entry.displayPath ?? entry.path;
  const { name: fileName, dir } = useMemo(() => splitPath(displayPath), [displayPath]);
  const displayDir = dir ? `${dir}/` : "";
  const fallbackLanguage = useMemo(() => languageFromPath(displayPath), [displayPath]);

  const fileDiff = useMemo(() => {
    if (!entry.diff.trim()) {
      return null;
    }
    const patch = parsePatchFiles(entry.diff);
    const parsed = patch[0]?.files[0];
    if (!parsed) {
      return null;
    }
    const normalizedName = normalizePatchName(parsed.name || displayPath);
    const normalizedPrevName = parsed.prevName ? normalizePatchName(parsed.prevName) : undefined;
    return {
      ...parsed,
      name: normalizedName,
      prevName: normalizedPrevName,
      deletionLines: entry.oldLines ?? parsed.deletionLines,
      additionLines: entry.newLines ?? parsed.additionLines,
    } satisfies FileDiffMetadata;
  }, [displayPath, entry.diff, entry.newLines, entry.oldLines]);

  const placeholder = useMemo(() => {
    if (isLoading) {
      return "Loading diff...";
    }
    if (ignoreWhitespaceChanges && !entry.diff.trim()) {
      return "No non-whitespace changes.";
    }
    return "Diff unavailable.";
  }, [entry.diff, ignoreWhitespaceChanges, isLoading]);

  const parsedLines = useMemo(() => {
    const parsed = parseDiff(entry.diff);
    if (parsed.length > 0) {
      return parsed;
    }
    return parseRawDiffLines(entry.diff);
  }, [entry.diff]);

  const hasSelectableLines = useMemo(() => parsedLines.some(isSelectableLine), [parsedLines]);
  const useInteractiveDiff = interactiveSelectionEnabled && hasSelectableLines;
  const lineActionEnabled = diffStyle === "unified" && Boolean(onLineAction) && hasSelectableLines;

  const diffOptions = useMemo(
    () => ({
      diffStyle,
      hunkSeparators: "line-info" as const,
      overflow: "scroll" as const,
      unsafeCSS: DIFF_VIEWER_SCROLL_CSS,
      disableFileHeader: true,
      enableLineSelection: useInteractiveDiff,
      onLineSelected: useInteractiveDiff ? onSelectedLinesChange : undefined,
      enableHoverUtility: lineActionEnabled,
    }),
    [diffStyle, lineActionEnabled, onSelectedLinesChange, useInteractiveDiff],
  );

  return (
    <div
      data-diff-path={entry.path}
      className={cn(
        "diff-viewer-item rounded-0 border-none bg-transparent p-0 w-full border-b border-border-subtle relative isolate",
        isSelected && "active",
      )}
    >
      <div className="diff-viewer-header flex items-center gap-2 text-ui-sm text-text-muted px-3 py-[10px] m-0 border-b border-border-subtle">
        <span
          className="diff-viewer-status font-bold text-ui-xs px-[6px] py-[2px] rounded-full border border-border-stronger text-text-stronger bg-surface-control uppercase"
          data-status={entry.status}
        >
          {entry.status}
        </span>
        <span className="diff-viewer-path inline-flex items-baseline gap-[6px] flex-1 min-w-0 break-words" title={displayPath}>
          <span className="diff-viewer-name text-text-emphasis font-semibold min-w-0 shrink-[1] grow-0 basis-auto overflow-hidden text-ellipsis whitespace-nowrap">
            {fileName}
          </span>
          {displayDir && (
            <span className="diff-viewer-dir text-text-faint flex-1 overflow-hidden text-ellipsis whitespace-nowrap min-w-0">
              {displayDir}
            </span>
          )}
        </span>
        {showRevert && (
          <button
            type="button"
            className="w-6 h-6 rounded-md p-0 border border-transparent bg-transparent text-text-faint inline-flex items-center justify-center cursor-pointer shrink-0 transition-[background,border-color,color] duration-ui-fast hover:!bg-[rgba(255,107,107,0.14)] hover:!border-[rgba(255,107,107,0.35)] hover:!text-[#ff6b6b] focus-visible:outline-2 focus-visible:outline-border-accent-soft focus-visible:outline-offset-2"
            title="Discard changes in this file"
            aria-label="Discard changes in this file"
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
              onRequestRevert?.(displayPath);
            }}
          >
            <RotateCcw size={14} aria-hidden />
          </button>
        )}
      </div>
      {useInteractiveDiff && selectedLines && reviewActions.length > 0 ? (
        <div
          className="diff-viewer-review-actions flex flex-wrap items-center gap-[6px] px-3 py-2 border-b border-border-subtle"
          role="toolbar"
          aria-label="PR selection actions"
        >
          {reviewActions.map((action) => (
            <button
              key={action.id}
              type="button"
              className="ghost diff-viewer-review-action text-ui-xs px-[10px] py-1 rounded-full border border-border-muted"
              disabled={pullRequestReviewLaunching}
              onClick={() => {
                if (!onRunReviewAction) {
                  return;
                }
                void onRunReviewAction(action.intent, parsedLines, selectedLines);
              }}
            >
              {action.label}
            </button>
          ))}
          <button
            type="button"
            className="ghost diff-viewer-review-action text-ui-xs px-[10px] py-1 rounded-full border border-border-muted"
            onClick={onClearSelection}
          >
            Clear
          </button>
          {pullRequestReviewThreadId ? (
            <span className="diff-viewer-review-thread ml-auto text-text-faint text-ui-xs font-code">
              Last review thread: {pullRequestReviewThreadId}
            </span>
          ) : null}
        </div>
      ) : null}
      {entry.diff.trim().length > 0 && fileDiff ? (
        <div className="diff-viewer-output diff-viewer-output-flat relative overflow-visible max-w-full min-w-0 w-full">
          <FileDiff
            fileDiff={fileDiff}
            options={diffOptions}
            selectedLines={useInteractiveDiff ? selectedLines : null}
            renderHoverUtility={
              lineActionEnabled
                ? (getHoveredLine) => (
                    <button
                      type="button"
                      className="diff-viewer-line-action-button"
                      aria-label="Ask for changes on hovered line"
                      title="Ask for changes on this line"
                      onMouseDown={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                      }}
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        const resolved = resolveParsedLineForHover(
                          parsedLines,
                          getHoveredLine() as HoveredDiffLine,
                        );
                        if (!resolved) {
                          return;
                        }
                        onLineAction?.(resolved.line, resolved.index);
                      }}
                    >
                      +
                    </button>
                  )
                : undefined
            }
            style={{ width: "100%", maxWidth: "100%", minWidth: 0 }}
          />
        </div>
      ) : entry.diff.trim().length > 0 && parsedLines.length > 0 ? (
        <div className="diff-viewer-output diff-viewer-output-flat diff-viewer-output-raw px-[10px] py-[6px]">
          {parsedLines.map((line) => {
            const highlighted = highlightLine(
              line.text,
              isFallbackRawDiffLineHighlightable(line.type) ? fallbackLanguage : null,
            );

            return (
              <div
                key={`${line.type}-${line.text.slice(0, 20)}`}
                className={cn(
                  "diff-viewer-raw-line whitespace-pre-wrap",
                  line.type === "add" && "diff-viewer-raw-line-add",
                  line.type === "del" && "diff-viewer-raw-line-del",
                )}
              >
                <span
                  ref={(el) => {
                    if (el) el.innerHTML = highlighted;
                  }}
                  className="diff-line-content min-w-0 whitespace-pre-wrap"
                />
              </div>
            );
          })}
        </div>
      ) : (
        <div className="text-text-subtle text-ui-sm py-2">{placeholder}</div>
      )}
    </div>
  );
});
