import { BookOpen, X } from "lucide-react";

interface MicroReviewPromptProps {
  dueCount: number;
  onAccept: () => void;
  onSkip: () => void;
}

export function MicroReviewPrompt({ dueCount, onAccept, onSkip }: MicroReviewPromptProps) {
  if (dueCount <= 0) return null;

  return (
    <div className="island border border-brand/20 p-4">
      <div className="flex items-start gap-3">
        <div className="p-1.5 rounded-control bg-brand/10 shrink-0 mt-0.5">
          <BookOpen size={14} className="text-brand" strokeWidth={1.5} />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-ui-sm text-fg leading-relaxed">
            Before you dive in — 45s review to keep your streak alive?
          </p>
          <p className="text-ui-xs text-fg-secondary mt-0.5">
            {dueCount} card{dueCount !== 1 ? "s" : ""} due for review
          </p>
        </div>
      </div>
      <div className="flex items-center gap-2 mt-3">
        <button
          type="button"
          onClick={onAccept}
          className="glass-button px-3 py-1.5 text-ui-xs font-medium text-brand inline-flex items-center gap-1.5"
        >
          <BookOpen size={12} strokeWidth={1.5} />
          Quick Review (45s)
        </button>
        <button
          type="button"
          onClick={onSkip}
          className="px-3 py-1.5 text-ui-xs text-fg-secondary hover:text-fg transition-colors inline-flex items-center gap-1"
        >
          <X size={12} strokeWidth={1.5} />
          Skip
        </button>
      </div>
    </div>
  );
}
