import { OpenAppMenu } from "@app/components/OpenAppMenu";
import { highlightLine, languageFromPath } from "@utils/syntax";
import X from "lucide-react/dist/esm/icons/x";
import type { CSSProperties, MouseEvent } from "react";
import { useEffect, useMemo, useRef } from "react";
import { cn } from "@/utils/cn";
import { PopoverSurface } from "@/features/design-system/components/popover/PopoverPrimitives";
import type { OpenAppTarget } from "@/types";

function SafeHtmlSpan({ html, className }: { html: string; className?: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (ref.current) {
      ref.current.innerHTML = html;
    }
  }, [html]);
  return <span ref={ref} className={className} />;
}

type FilePreviewPopoverProps = {
  path: string;
  absolutePath: string;
  content: string;
  truncated: boolean;
  previewKind?: "text" | "image";
  imageSrc?: string | null;
  openTargets: OpenAppTarget[];
  openAppIconById: Record<string, string>;
  selectedOpenAppId: string;
  onSelectOpenAppId: (id: string) => void;
  selection: { start: number; end: number } | null;
  onSelectLine: (index: number, event: MouseEvent<HTMLButtonElement>) => void;
  onLineMouseDown?: (index: number, event: MouseEvent<HTMLButtonElement>) => void;
  onLineMouseEnter?: (index: number, event: MouseEvent<HTMLButtonElement>) => void;
  onLineMouseUp?: (index: number, event: MouseEvent<HTMLButtonElement>) => void;
  onClearSelection: () => void;
  onAddSelection: () => void;
  canInsertText?: boolean;
  onClose: () => void;
  selectionHints?: string[];
  style?: CSSProperties;
  isLoading?: boolean;
  error?: string | null;
};

export function FilePreviewPopover({
  path,
  absolutePath,
  content,
  truncated,
  previewKind = "text",
  imageSrc = null,
  openTargets,
  openAppIconById,
  selectedOpenAppId,
  onSelectOpenAppId,
  selection,
  onSelectLine,
  onLineMouseDown,
  onLineMouseEnter,
  onLineMouseUp,
  onClearSelection,
  onAddSelection,
  canInsertText = true,
  onClose,
  selectionHints = [],
  style,
  isLoading = false,
  error = null,
}: FilePreviewPopoverProps) {
  const isImagePreview = previewKind === "image";
  const lines = useMemo(
    () => (isImagePreview ? [] : content.split("\n")),
    [content, isImagePreview],
  );
  const language = useMemo(() => languageFromPath(path), [path]);
  const selectionLabel = selection
    ? `Lines ${selection.start + 1}-${selection.end + 1}`
    : isImagePreview
      ? "Image preview"
      : "No selection";
  const highlightedLines = useMemo(
    () =>
      isImagePreview
        ? []
        : lines.map((line) => {
            const html = highlightLine(line, language);
            return html || "&nbsp;";
          }),
    [lines, language, isImagePreview],
  );

  return (
    <PopoverSurface className="file-preview-popover rounded-xl p-3 flex flex-col gap-[10px] z-30 relative" style={style}>
      <div className="file-preview-header flex items-center justify-between gap-3">
        <div className="file-preview-title flex items-center gap-2 min-w-0">
          <span className="file-preview-path text-ui-sm text-text-strong whitespace-nowrap overflow-hidden text-ellipsis">
            {path}
          </span>
          {truncated && (
            <span className="file-preview-warning text-ui-2xs uppercase tracking-[0.08em] text-text-faint">
              Truncated
            </span>
          )}
        </div>
        <button
          type="button"
          className="icon-button file-preview-close p-1"
          onClick={onClose}
          aria-label="Close preview"
          title="Close preview"
        >
          <X size={14} aria-hidden />
        </button>
      </div>
      {isLoading ? (
        <div className="file-preview-status text-ui-sm text-text-faint">Loading file...</div>
      ) : error ? (
        <div className="file-preview-status file-preview-error text-ui-sm text-text-danger">
          {error}
        </div>
      ) : isImagePreview ? (
        <div className="file-preview-body flex flex-col gap-3 min-h-0 max-h-[70vh]">
          <div className="file-preview-toolbar flex items-center justify-between gap-3 text-ui-xs text-text-faint">
            <span>{selectionLabel}</span>
            <div className="file-preview-actions flex items-center gap-[6px]">
              <OpenAppMenu
                path={absolutePath}
                openTargets={openTargets}
                selectedOpenAppId={selectedOpenAppId}
                onSelectOpenAppId={onSelectOpenAppId}
                iconById={openAppIconById}
              />
            </div>
          </div>
          {imageSrc ? (
            <div className="file-preview-image flex items-center justify-center p-3 rounded-[10px] overflow-auto max-h-[60vh]">
              <img src={imageSrc} alt={path} />
            </div>
          ) : (
            <div className="file-preview-status file-preview-error text-ui-sm text-text-danger">
              Image preview unavailable.
            </div>
          )}
        </div>
      ) : (
        <div className="file-preview-body flex flex-col gap-2 min-h-0 max-h-[70vh]">
          <div className="file-preview-toolbar flex items-center justify-between gap-3 text-ui-xs text-text-faint">
            <div className="file-preview-selection-group flex flex-col gap-[2px] min-w-0">
              <span>{selectionLabel}</span>
              {selectionHints.length > 0 ? (
                <div className="file-preview-hints flex flex-wrap gap-2 text-ui-2xs text-text-fainter">
                  {selectionHints.map((hint) => (
                    <span key={hint} className="file-preview-hint whitespace-nowrap">
                      {hint}
                    </span>
                  ))}
                </div>
              ) : null}
            </div>
            <div className="file-preview-actions flex items-center gap-[6px]">
              <OpenAppMenu
                path={absolutePath}
                openTargets={openTargets}
                selectedOpenAppId={selectedOpenAppId}
                onSelectOpenAppId={onSelectOpenAppId}
                iconById={openAppIconById}
              />
              <button
                type="button"
                className="ghost file-preview-action inline-flex items-center px-3 py-[6px] min-h-[30px] text-ui-sm leading-snug rounded-[10px]"
                onClick={onClearSelection}
                disabled={!selection}
              >
                Clear
              </button>
              <button
                type="button"
                className="primary file-preview-action inline-flex items-center px-3 py-[6px] min-h-[30px] text-ui-sm leading-snug rounded-[10px] bg-surface-active text-text-strong border border-border-accent shadow-lg hover:!-translate-y-px hover:!shadow-[0_10px_20px_rgba(0,0,0,0.3)]"
                onClick={onAddSelection}
                disabled={!selection || !canInsertText}
              >
                Add to chat
              </button>
            </div>
          </div>
          <ol className="file-preview-lines flex flex-col gap-0 overflow-auto rounded-lg bg-surface-command py-[6px] font-code text-[11px] font-normal leading-relaxed text-text-quiet whitespace-pre">
            {(() => {
              const lineButtons: React.ReactElement[] = [];
              for (let index = 0; index < lines.length; index++) {
                const html = highlightedLines[index] ?? "&nbsp;";
                const isSelected = selection && index >= selection.start && index <= selection.end;
                const isStart = isSelected && selection?.start === index;
                const isEnd = isSelected && selection?.end === index;
                lineButtons.push(
                  <button
                    key={`line-${index + 1}`}
                    type="button"
                    className={cn(
                      "file-preview-line grid items-start gap-[10px] px-[10px] py-[2px] min-w-full w-max border border-transparent rounded-0 bg-transparent text-left cursor-pointer font-code text-[11px] font-normal leading-relaxed outline-none transition-none",
                      isSelected && "is-selected",
                      isStart && "is-start",
                      isEnd && "is-end",
                    )}
                    style={{ gridTemplateColumns: "52px 1fr" }}
                    onClick={(event) => onSelectLine(index, event)}
                    onMouseDown={(event) => onLineMouseDown?.(index, event)}
                    onMouseEnter={(event) => onLineMouseEnter?.(index, event)}
                    onMouseUp={(event) => onLineMouseUp?.(index, event)}
                  >
                    <span className="file-preview-line-number text-text-fainter text-right tabular-nums">
                      {index + 1}
                    </span>
                    <SafeHtmlSpan className="file-preview-line-text min-w-0 overflow-wrap-anywhere break-words" html={html || "&nbsp;"} />
                  </button>,
                );
              }
              return lineButtons;
            })()}
          </ol>
        </div>
      )}
    </PopoverSurface>
  );
}
