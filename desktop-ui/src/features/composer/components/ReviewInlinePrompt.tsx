import type { ReviewPromptState, ReviewPromptStep } from "@threads/hooks/useReviewPrompt";
import { cn } from "@/utils/cn";
import { memo, useMemo } from "react";

type ReviewInlinePromptProps = {
  reviewPrompt: NonNullable<ReviewPromptState>;
  onClose: () => void;
  onShowPreset: () => void;
  onChoosePreset: (preset: Exclude<ReviewPromptStep, "preset"> | "uncommitted") => void;
  highlightedPresetIndex: number;
  onHighlightPreset: (index: number) => void;
  highlightedBranchIndex: number;
  onHighlightBranch: (index: number) => void;
  highlightedCommitIndex: number;
  onHighlightCommit: (index: number) => void;
  onSelectBranch: (value: string) => void;
  onSelectBranchAtIndex: (index: number) => void;
  onConfirmBranch: () => Promise<void>;
  onSelectCommit: (sha: string, title: string) => void;
  onSelectCommitAtIndex: (index: number) => void;
  onConfirmCommit: () => Promise<void>;
  onUpdateCustomInstructions: (value: string) => void;
  onConfirmCustom: () => Promise<void>;
};

function shortSha(sha: string) {
  return sha.slice(0, 7);
}

const PresetStep = memo(function PresetStep({
  onChoosePreset,
  isSubmitting,
  highlightedPresetIndex,
  onHighlightPreset,
}: {
  onChoosePreset: ReviewInlinePromptProps["onChoosePreset"];
  isSubmitting: boolean;
  highlightedPresetIndex: number;
  onHighlightPreset: (index: number) => void;
}) {
  const optionClass = (index: number) =>
    cn(
      "review-inline-option rounded-[10px] border border-border-subtle bg-surface-card-muted text-text-strong p-[10px_12px] text-ui-sm flex flex-col items-start gap-0.5 text-left",
      index === highlightedPresetIndex && "is-selected",
    );
  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        className={optionClass(0)}
        onClick={() => onChoosePreset("baseBranch")}
        onMouseEnter={() => onHighlightPreset(0)}
        disabled={isSubmitting}
      >
        <span className="font-semibold">Review against a base branch</span>
        <span className="text-ui-xs text-text-faint">(PR Style)</span>
      </button>
      <button
        type="button"
        className={optionClass(1)}
        onClick={() => onChoosePreset("uncommitted")}
        onMouseEnter={() => onHighlightPreset(1)}
        disabled={isSubmitting}
      >
        <span className="font-semibold">Review uncommitted changes</span>
      </button>
      <button
        type="button"
        className={optionClass(2)}
        onClick={() => onChoosePreset("commit")}
        onMouseEnter={() => onHighlightPreset(2)}
        disabled={isSubmitting}
      >
        <span className="font-semibold">Review a commit</span>
      </button>
      <button
        type="button"
        className={optionClass(3)}
        onClick={() => onChoosePreset("custom")}
        onMouseEnter={() => onHighlightPreset(3)}
        disabled={isSubmitting}
      >
        <span className="font-semibold">Custom review instructions</span>
      </button>
    </div>
  );
});

const BaseBranchStep = memo(function BaseBranchStep({
  reviewPrompt,
  onShowPreset,
  onSelectBranch,
  onSelectBranchAtIndex,
  onConfirmBranch,
  highlightedBranchIndex,
  onHighlightBranch,
}: {
  reviewPrompt: NonNullable<ReviewPromptState>;
  onShowPreset: () => void;
  onSelectBranch: (value: string) => void;
  onSelectBranchAtIndex: (index: number) => void;
  onConfirmBranch: () => Promise<void>;
  highlightedBranchIndex: number;
  onHighlightBranch: (index: number) => void;
}) {
  const branches = reviewPrompt.branches;
  return (
    <div className="flex flex-col gap-2">
      <div className="flex justify-between items-center gap-2">
        <button
          type="button"
          className="ghost review-inline-back"
          onClick={onShowPreset}
          disabled={reviewPrompt.isSubmitting}
        >
          Back
        </button>
        <button
          type="button"
          className="primary review-inline-confirm"
          onClick={() => void onConfirmBranch()}
          disabled={reviewPrompt.isSubmitting || !reviewPrompt.selectedBranch.trim()}
        >
          Start review
        </button>
      </div>
      <div className="text-ui-sm text-text-subtle">Pick a recent local branch:</div>
      <div className="flex flex-col gap-[6px] max-h-[200px] overflow-auto pr-0.5" role="listbox" aria-label="Base branches">
        {reviewPrompt.isLoadingBranches ? (
          <div className="text-ui-sm text-text-subtle px-0.5 py-1">Loading branches…</div>
        ) : branches.length === 0 ? (
          <div className="text-ui-sm text-text-subtle px-0.5 py-1">No branches found.</div>
        ) : (
          branches.map((branch, index) => {
            const selected = index === highlightedBranchIndex;
            return (
              <button
                key={branch.name}
                type="button"
                role="option"
                aria-selected={selected}
                className={cn(
                  "review-inline-list-item rounded-[9px] border border-border-subtle bg-surface-card-muted text-text-strong p-[8px_10px] text-ui-sm text-left",
                  selected && "is-selected",
                )}
                onClick={() => onSelectBranch(branch.name)}
                onMouseEnter={() => {
                  onHighlightBranch(index);
                  onSelectBranchAtIndex(index);
                }}
                disabled={reviewPrompt.isSubmitting}
              >
                {branch.name}
              </button>
            );
          })
        )}
      </div>
    </div>
  );
});

const CommitStep = memo(function CommitStep({
  reviewPrompt,
  onShowPreset,
  onSelectCommit,
  onSelectCommitAtIndex,
  onConfirmCommit,
  highlightedCommitIndex,
  onHighlightCommit,
}: {
  reviewPrompt: NonNullable<ReviewPromptState>;
  onShowPreset: () => void;
  onSelectCommit: (sha: string, title: string) => void;
  onSelectCommitAtIndex: (index: number) => void;
  onConfirmCommit: () => Promise<void>;
  highlightedCommitIndex: number;
  onHighlightCommit: (index: number) => void;
}) {
  const commits = reviewPrompt.commits;
  return (
    <div className="flex flex-col gap-2">
      <div className="flex justify-between items-center gap-2">
        <button
          type="button"
          className="ghost review-inline-back"
          onClick={onShowPreset}
          disabled={reviewPrompt.isSubmitting}
        >
          Back
        </button>
        <button
          type="button"
          className="primary review-inline-confirm"
          onClick={() => void onConfirmCommit()}
          disabled={reviewPrompt.isSubmitting || !reviewPrompt.selectedCommitSha}
        >
          Start review
        </button>
      </div>
      <div className="text-ui-sm text-text-subtle">Select a recent commit:</div>
      <div className="flex flex-col gap-[6px] max-h-[200px] overflow-auto pr-0.5" role="listbox" aria-label="Commits">
        {reviewPrompt.isLoadingCommits ? (
          <div className="text-ui-sm text-text-subtle px-0.5 py-1">Loading commits…</div>
        ) : commits.length === 0 ? (
          <div className="text-ui-sm text-text-subtle px-0.5 py-1">No commits found.</div>
        ) : (
          commits.map((commit, index) => {
            const title = commit.summary || commit.sha;
            const selected = index === highlightedCommitIndex;
            return (
              <button
                key={commit.sha}
                type="button"
                role="option"
                aria-selected={selected}
                className={cn(
                  "review-inline-list-item flex flex-col gap-0.5 rounded-[9px] border border-border-subtle bg-surface-card-muted text-text-strong p-[8px_10px] text-ui-sm text-left",
                  selected && "is-selected",
                )}
                onClick={() => onSelectCommit(commit.sha, title)}
                onMouseEnter={() => {
                  onHighlightCommit(index);
                  onSelectCommitAtIndex(index);
                }}
                disabled={reviewPrompt.isSubmitting}
              >
                <span className="font-semibold">{title}</span>
                <span className="text-ui-xs text-text-faint">{shortSha(commit.sha)}</span>
              </button>
            );
          })
        )}
      </div>
    </div>
  );
});

const CustomStep = memo(function CustomStep({
  reviewPrompt,
  onShowPreset,
  onUpdateCustomInstructions,
  onConfirmCustom,
}: {
  reviewPrompt: NonNullable<ReviewPromptState>;
  onShowPreset: () => void;
  onUpdateCustomInstructions: (value: string) => void;
  onConfirmCustom: () => Promise<void>;
}) {
  const canSubmit = reviewPrompt.customInstructions.trim().length > 0;
  return (
    <div className="flex flex-col gap-2">
      <div className="flex justify-between items-center gap-2">
        <button
          type="button"
          className="ghost review-inline-back"
          onClick={onShowPreset}
          disabled={reviewPrompt.isSubmitting}
        >
          Back
        </button>
        <button
          type="button"
          className="primary review-inline-confirm"
          onClick={() => void onConfirmCustom()}
          disabled={reviewPrompt.isSubmitting || !canSubmit}
        >
          Start review
        </button>
      </div>
      <label className="text-ui-sm text-text-faint" htmlFor="review-inline-custom-instructions">
        Instructions
      </label>
      <textarea
        id="review-inline-custom-instructions"
        className="review-inline-textarea rounded-[10px] border border-border-subtle bg-surface-card-muted text-text-strong p-[9px_11px] text-ui-sm w-full"
        value={reviewPrompt.customInstructions}
        onChange={(event) => onUpdateCustomInstructions(event.target.value)}
        placeholder="Focus on correctness, edge cases, and missing tests."
        rows={6}
      />
    </div>
  );
});

export const ReviewInlinePrompt = memo(function ReviewInlinePrompt({
  reviewPrompt,
  onClose,
  onShowPreset,
  onChoosePreset,
  highlightedPresetIndex,
  onHighlightPreset,
  highlightedBranchIndex,
  onHighlightBranch,
  highlightedCommitIndex,
  onHighlightCommit,
  onSelectBranch,
  onSelectBranchAtIndex,
  onConfirmBranch,
  onSelectCommit,
  onSelectCommitAtIndex,
  onConfirmCommit,
  onUpdateCustomInstructions,
  onConfirmCustom,
}: ReviewInlinePromptProps) {
  const { step, error, isSubmitting } = reviewPrompt;

  const title = useMemo(() => {
    switch (step) {
      case "baseBranch":
        return "Select a base branch";
      case "commit":
        return "Select a commit to review";
      case "custom":
        return "Custom review instructions";
      default:
        return "Select a review preset";
    }
  }, [step]);

  return (
    <div className="flex flex-col gap-[10px]" role="dialog" aria-label={title}>
      <div className="flex flex-col gap-0.5">
        <div className="text-ui-md font-semibold text-text-strong">{title}</div>
        <div className="text-ui-sm text-text-subtle">{reviewPrompt.workspace.name}</div>
      </div>

      {step === "preset" ? (
        <PresetStep
          onChoosePreset={onChoosePreset}
          isSubmitting={isSubmitting}
          highlightedPresetIndex={highlightedPresetIndex}
          onHighlightPreset={onHighlightPreset}
        />
      ) : step === "baseBranch" ? (
        <BaseBranchStep
          reviewPrompt={reviewPrompt}
          onShowPreset={onShowPreset}
          onSelectBranch={onSelectBranch}
          onSelectBranchAtIndex={onSelectBranchAtIndex}
          onConfirmBranch={onConfirmBranch}
          highlightedBranchIndex={highlightedBranchIndex}
          onHighlightBranch={onHighlightBranch}
        />
      ) : step === "commit" ? (
        <CommitStep
          reviewPrompt={reviewPrompt}
          onShowPreset={onShowPreset}
          onSelectCommit={onSelectCommit}
          onSelectCommitAtIndex={onSelectCommitAtIndex}
          onConfirmCommit={onConfirmCommit}
          highlightedCommitIndex={highlightedCommitIndex}
          onHighlightCommit={onHighlightCommit}
        />
      ) : (
        <CustomStep
          reviewPrompt={reviewPrompt}
          onShowPreset={onShowPreset}
          onUpdateCustomInstructions={onUpdateCustomInstructions}
          onConfirmCustom={onConfirmCustom}
        />
      )}

      {error && <div className="text-ui-sm text-[var(--accent-danger)] bg-[rgba(255,100,100,0.08)] border border-[rgba(255,100,100,0.2)] p-[7px_9px] rounded-[9px]">{error}</div>}

      <div className="flex justify-end gap-2">
        <button type="button" className="ghost review-inline-button" onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
});
