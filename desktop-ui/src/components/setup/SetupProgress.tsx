import { SETUP_STEPS } from "./steps";

interface SetupProgressProps {
  currentIndex: number;
}

export function SetupProgress({ currentIndex }: SetupProgressProps) {
  const progress = ((currentIndex + 1) / SETUP_STEPS.length) * 100;

  return (
    <div className="w-full">
      <div className="flex justify-between text-[11px] text-muted mb-1.5">
        <span>
          Step {currentIndex + 1} of {SETUP_STEPS.length}
        </span>
        <span>{SETUP_STEPS[currentIndex]?.label}</span>
      </div>
      <div className="h-1 rounded-full bg-surface-base overflow-hidden">
        <div
          className="h-full rounded-full bg-brand transition-all duration-500 ease-out"
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
  );
}
