import { Check, Circle, Loader2 } from "lucide-react";

interface PlanProgressProps {
  steps: string[];
  completedSteps: number[];
  isStreaming?: boolean;
}

export function PlanProgress({ steps, completedSteps, isStreaming }: PlanProgressProps) {
  if (steps.length === 0) return null;

  const completedSet = new Set(completedSteps);
  const doneCount = completedSet.size;
  const totalCount = steps.length;

  // Find the currently executing step (first non-completed step)
  const activeIndex = steps.findIndex((_, i) => !completedSet.has(i));

  return (
    <div className="my-2 rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2.5">
      <div className="flex items-center gap-2 mb-2">
        <span className="text-[11px] font-medium text-secondary">Plan</span>
        <span className="text-[10px] font-light text-dim">
          {doneCount}/{totalCount} steps
        </span>
        {doneCount < totalCount && isStreaming && (
          <Loader2 className="w-3 h-3 text-brand animate-spin" strokeWidth={2} />
        )}
        {doneCount === totalCount && <Check className="w-3 h-3 text-success" strokeWidth={2} />}
      </div>
      <div className="space-y-1">
        {steps.map((step, i) => {
          const isCompleted = completedSet.has(i);
          const isActive = i === activeIndex && isStreaming;
          return (
            <div key={`step-${i}`} className="flex items-start gap-2">
              <div className="mt-0.5 flex-shrink-0">
                {isCompleted ? (
                  <Check className="w-3 h-3 text-success" strokeWidth={2} />
                ) : isActive ? (
                  <Loader2 className="w-3 h-3 text-brand animate-spin" strokeWidth={2} />
                ) : (
                  <Circle className="w-3 h-3 text-dim" strokeWidth={1.5} />
                )}
              </div>
              <span
                className={`text-[11px] font-light leading-snug ${
                  isCompleted ? "text-dim line-through" : isActive ? "text-secondary" : "text-muted"
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
