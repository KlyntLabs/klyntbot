import type { ReviewPromptState, ReviewPromptStep } from "@threads/hooks/useReviewPrompt";
import { getFileTypeIconUrl } from "@utils/fileTypeIcons";
import Brain from "lucide-react/dist/esm/icons/brain";
import FileText from "lucide-react/dist/esm/icons/file-text";
import GitFork from "lucide-react/dist/esm/icons/git-fork";
import Info from "lucide-react/dist/esm/icons/info";
import Plug from "lucide-react/dist/esm/icons/plug";
import PlusCircle from "lucide-react/dist/esm/icons/plus-circle";
import RotateCcw from "lucide-react/dist/esm/icons/rotate-ccw";
import ScrollText from "lucide-react/dist/esm/icons/scroll-text";
import Wrench from "lucide-react/dist/esm/icons/wrench";
import { type CSSProperties, memo, type RefObject, useEffect } from "react";
import { cn } from "@/utils/cn";
import { PopoverSurface } from "@/features/design-system/components/popover/PopoverPrimitives";
import type { AutocompleteItem } from "../hooks/useComposerAutocomplete";
import { ReviewInlinePrompt } from "./ReviewInlinePrompt";

type ComposerSuggestionsPopoverProps = {
  highlightIndex: number;
  highlightedBranchIndex?: number;
  highlightedCommitIndex?: number;
  highlightedPresetIndex?: number;
  onHighlightIndex: (index: number) => void;
  onReviewPromptChoosePreset?: (
    preset: Exclude<ReviewPromptStep, "preset"> | "uncommitted",
  ) => void;
  onReviewPromptClose?: () => void;
  onReviewPromptConfirmBranch?: () => Promise<void>;
  onReviewPromptConfirmCommit?: () => Promise<void>;
  onReviewPromptConfirmCustom?: () => Promise<void>;
  onReviewPromptHighlightBranch?: (index: number) => void;
  onReviewPromptHighlightCommit?: (index: number) => void;
  onReviewPromptHighlightPreset?: (index: number) => void;
  onReviewPromptSelectBranch?: (value: string) => void;
  onReviewPromptSelectBranchAtIndex?: (index: number) => void;
  onReviewPromptSelectCommit?: (sha: string, title: string) => void;
  onReviewPromptSelectCommitAtIndex?: (index: number) => void;
  onReviewPromptShowPreset?: () => void;
  onReviewPromptUpdateCustomInstructions?: (value: string) => void;
  onSelectSuggestion: (item: AutocompleteItem) => void;
  reviewPrompt?: ReviewPromptState;
  suggestionListRef: RefObject<HTMLDivElement | null>;
  suggestionRefs: RefObject<Array<HTMLButtonElement | null>>;
  suggestions: AutocompleteItem[];
  suggestionsOpen: boolean;
  suggestionsStyle?: CSSProperties;
};

const isFileSuggestion = (item: AutocompleteItem) => item.group === "Files";

const suggestionIcon = (item: AutocompleteItem) => {
  if (isFileSuggestion(item)) {
    return FileText;
  }
  if (item.id.startsWith("skill:")) {
    return Wrench;
  }
  if (item.id.startsWith("app:")) {
    return Plug;
  }
  if (item.id === "review") {
    return Brain;
  }
  if (item.id === "fork") {
    return GitFork;
  }
  if (item.id === "mcp" || item.id === "apps") {
    return Plug;
  }
  if (item.id === "new") {
    return PlusCircle;
  }
  if (item.id === "resume") {
    return RotateCcw;
  }
  if (item.id === "status") {
    return Info;
  }
  if (item.id.startsWith("prompt:")) {
    return ScrollText;
  }
  return Wrench;
};

const fileTitle = (path: string) => {
  const normalized = path.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  return parts.length ? parts[parts.length - 1] : path;
};

export const ComposerSuggestionsPopover = memo(function ComposerSuggestionsPopover({
  highlightIndex,
  highlightedBranchIndex,
  highlightedCommitIndex,
  highlightedPresetIndex,
  onHighlightIndex,
  onReviewPromptChoosePreset,
  onReviewPromptClose,
  onReviewPromptConfirmBranch,
  onReviewPromptConfirmCommit,
  onReviewPromptConfirmCustom,
  onReviewPromptHighlightBranch,
  onReviewPromptHighlightCommit,
  onReviewPromptHighlightPreset,
  onReviewPromptSelectBranch,
  onReviewPromptSelectBranchAtIndex,
  onReviewPromptSelectCommit,
  onReviewPromptSelectCommitAtIndex,
  onReviewPromptShowPreset,
  onReviewPromptUpdateCustomInstructions,
  onSelectSuggestion,
  reviewPrompt,
  suggestionListRef,
  suggestionRefs,
  suggestions,
  suggestionsOpen,
  suggestionsStyle,
}: ComposerSuggestionsPopoverProps) {
  const reviewPromptOpen = Boolean(reviewPrompt);
  const suggestionsCount = suggestions.length;

  useEffect(() => {
    if (!suggestionsOpen || reviewPromptOpen || suggestionsCount === 0) {
      return;
    }
    const list = suggestionListRef.current;
    const item = suggestionRefs.current[highlightIndex];
    if (!list || !item) {
      return;
    }
    const listRect = list.getBoundingClientRect();
    const itemRect = item.getBoundingClientRect();
    if (itemRect.top < listRect.top) {
      item.scrollIntoView({ block: "nearest" });
      return;
    }
    if (itemRect.bottom > listRect.bottom) {
      item.scrollIntoView({ block: "nearest" });
    }
  }, [
    highlightIndex,
    reviewPromptOpen,
    suggestionListRef,
    suggestionRefs,
    suggestionsCount,
    suggestionsOpen,
  ]);

  if (!suggestionsOpen) {
    return null;
  }

  return (
    <PopoverSurface
      className={cn(
        "absolute left-[-12px] right-[-12px] bottom-[calc(100%+8px)] top-auto z-10 grid gap-[2px] p-[6px] border border-[var(--cm-border-heavy)] bg-[var(--cm-surface-panel-elevated)] rounded-[20px] shadow-[inset_0_1px_0_rgba(255,255,255,0.03)] max-h-[280px] overflow-y-auto overflow-x-hidden",
        reviewPromptOpen && "review-inline-suggestions",
      )}
      role="listbox"
      ref={suggestionListRef}
      style={suggestionsStyle}
    >
      {reviewPromptOpen &&
      reviewPrompt &&
      onReviewPromptClose &&
      onReviewPromptShowPreset &&
      onReviewPromptChoosePreset &&
      highlightedPresetIndex !== undefined &&
      onReviewPromptHighlightPreset &&
      highlightedBranchIndex !== undefined &&
      onReviewPromptHighlightBranch &&
      highlightedCommitIndex !== undefined &&
      onReviewPromptHighlightCommit &&
      onReviewPromptSelectBranch &&
      onReviewPromptSelectBranchAtIndex &&
      onReviewPromptConfirmBranch &&
      onReviewPromptSelectCommit &&
      onReviewPromptSelectCommitAtIndex &&
      onReviewPromptConfirmCommit &&
      onReviewPromptUpdateCustomInstructions &&
      onReviewPromptConfirmCustom ? (
        <ReviewInlinePrompt
          reviewPrompt={reviewPrompt}
          onClose={onReviewPromptClose}
          onShowPreset={onReviewPromptShowPreset}
          onChoosePreset={onReviewPromptChoosePreset}
          highlightedPresetIndex={highlightedPresetIndex}
          onHighlightPreset={onReviewPromptHighlightPreset}
          highlightedBranchIndex={highlightedBranchIndex}
          onHighlightBranch={onReviewPromptHighlightBranch}
          highlightedCommitIndex={highlightedCommitIndex}
          onHighlightCommit={onReviewPromptHighlightCommit}
          onSelectBranch={onReviewPromptSelectBranch}
          onSelectBranchAtIndex={onReviewPromptSelectBranchAtIndex}
          onConfirmBranch={onReviewPromptConfirmBranch}
          onSelectCommit={onReviewPromptSelectCommit}
          onSelectCommitAtIndex={onReviewPromptSelectCommitAtIndex}
          onConfirmCommit={onReviewPromptConfirmCommit}
          onUpdateCustomInstructions={onReviewPromptUpdateCustomInstructions}
          onConfirmCustom={onReviewPromptConfirmCustom}
        />
      ) : (
        suggestions.map((item, index) => {
          const prevGroup = suggestions[index - 1]?.group;
          // Only show on transition between named groups; first item has prevGroup === undefined.
          const showGroup = Boolean(
            item.group && item.group !== prevGroup && prevGroup !== undefined,
          );
          const Icon = suggestionIcon(item);
          const fileSuggestion = isFileSuggestion(item);
          const skillSuggestion = item.id.startsWith("skill:");
          const title = fileSuggestion ? fileTitle(item.label) : item.label;
          const description = fileSuggestion ? item.label : item.description;
          const fileTypeIconUrl = fileSuggestion ? getFileTypeIconUrl(item.label) : null;

          return (
            <div key={item.id}>
              {showGroup && <div className="text-ui-2xs font-bold tracking-[0.08em] uppercase text-text-faint px-2 pt-1.5 pb-0.5">{item.group}</div>}
              <button
                type="button"
                className={cn(
                  "composer-suggestion flex flex-col gap-0 text-left border border-transparent rounded-md px-2 py-1 bg-transparent text-text-strong cursor-pointer w-full min-w-0",
                  index === highlightIndex && "is-active",
                )}
                role="option"
                aria-selected={index === highlightIndex}
                ref={(node) => {
                  suggestionRefs.current[index] = node;
                }}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => onSelectSuggestion(item)}
                onMouseEnter={() => onHighlightIndex(index)}
              >
                <span className="flex items-center gap-2 min-w-0">
                  <span className="composer-suggestion-icon inline-flex w-4 h-4 text-text-muted flex-shrink-0 mt-0" aria-hidden>
                    {fileTypeIconUrl ? (
                      <img
                        className="w-3.5 h-3.5 block"
                        src={fileTypeIconUrl}
                        alt=""
                        loading="lazy"
                        decoding="async"
                      />
                    ) : (
                      <Icon size={14} />
                    )}
                  </span>
                  <span className="flex flex-row items-baseline gap-2 min-w-0 flex-1">
                    <span className="text-ui-sm font-semibold whitespace-nowrap overflow-hidden text-ellipsis flex-shrink-0 w-auto">{title}</span>
                    {description && (
                      <span
                        className={cn(
                          "text-ui-xs text-text-faint font-normal whitespace-nowrap overflow-hidden text-ellipsis flex-1 min-w-0 w-auto",
                          skillSuggestion && "line-clamp-2",
                        )}
                      >
                        {description}
                      </span>
                    )}
                    {!fileSuggestion && item.hint && (
                      <span className="text-ui-xs text-text-faint font-normal whitespace-nowrap overflow-hidden text-ellipsis flex-1 min-w-0 w-auto">{item.hint}</span>
                    )}
                  </span>
                </span>
              </button>
            </div>
          );
        })
      )}
    </PopoverSurface>
  );
});
