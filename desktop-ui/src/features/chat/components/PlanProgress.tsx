import { Check, Circle, Loader2 } from "lucide-react";
import { useMemo } from "react";

interface PlanProgressProps {
  steps: string[];
  completedSteps: number[];
  isStreaming?: boolean;
}

export function PlanProgress({ steps, completedSteps, isStreaming }: PlanProgressProps) {
  const completedSet = useMemo(() => new Set(completedSteps), [completedSteps]);

  if (steps.length === 0) return null;

  const doneCount = completedSet.size;
  const totalCount = steps.length;

  // Find the currently executing step (first non-completed step)
  const activeIndex = steps.findIndex((_, i) => !completedSet.has(i));

  return (
    <div className="my-2 glass-card px-3 py-2.5">
      <div className="flex items-center gap-2 mb-2">
        <span className="text-ui-xs font-medium text-fg-secondary">Plan</span>
        <span className="text-ui-xs font-light text-fg-dim">
          {doneCount}/{totalCount} steps
        </span>
        {doneCount < totalCount && isStreaming && (
          <Loader2 className="size-3 text-brand animate-spin" strokeWidth={2} />
        )}
        {doneCount === totalCount && <Check className="size-3 text-status-success" strokeWidth={2} />}
      </div>
      <div className="space-y-1">
        {steps.map((step, i) => {
          const isCompleted = completedSet.has(i);
          const isActive = i === activeIndex && isStreaming;
          return (
            <div key={step} className="flex items-start gap-2">
              {isCompleted ? (
                <Check className="mt-0.5 size-3 flex-shrink-0 text-status-success" strokeWidth={2} />
              ) : isActive ? (
                <Loader2
                  className="mt-0.5 size-3 flex-shrink-0 text-brand animate-spin"
                  strokeWidth={2}
                />
              ) : (
                <Circle className="mt-0.5 size-3 flex-shrink-0 text-fg-dim" strokeWidth={1.5} />
              )}
              <span
                className={`text-ui-xs font-light leading-snug ${
                  isCompleted
                    ? "text-fg-dim line-through"
                    : isActive
                      ? "text-fg-secondary"
                      : "text-fg-secondary"
                }`}
              >
                {step}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
