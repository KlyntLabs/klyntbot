import { ChevronLeft, ChevronRight, SkipForward } from "lucide-react";
import { Outlet } from "react-router";
import { SetupProgress } from "./SetupProgress";
import { useSetupNavigation } from "./useSetupNavigation";

export function SetupLayout() {
  const { currentStep, currentIndex, isFirst, isLast, next, back, skip } = useSetupNavigation();

  const isWelcome = currentStep?.id === "welcome";
  const isComplete = currentStep?.id === "complete";

  return (
    <div className="fixed inset-0 flex items-center justify-center bg-background">
      <div className="w-full max-w-xl mx-auto px-6">
        {/* Progress bar — hidden on welcome & complete */}
        {!isWelcome && !isComplete && (
          <div className="mb-6">
            <SetupProgress currentIndex={currentIndex} />
          </div>
        )}

        {/* Main card */}
        <div className="bg-surface-low rounded-2xl border border-border p-8">
          <Outlet context={{ next, back, skip }} />
        </div>

        {/* Navigation buttons — hidden on welcome & complete */}
        {!isWelcome && !isComplete && (
          <div className="flex items-center justify-between mt-6">
            <button
              type="button"
              onClick={back}
              disabled={isFirst}
              className="flex items-center gap-1.5 px-4 py-2 text-[13px] text-muted hover:text-secondary transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            >
              <ChevronLeft className="w-3.5 h-3.5" />
              Back
            </button>

            <div className="flex items-center gap-2">
              {!currentStep?.required && (
                <button
                  type="button"
                  onClick={skip}
                  className="flex items-center gap-1.5 px-4 py-2 text-[13px] text-muted hover:text-secondary transition-colors"
                >
                  Skip
                  <SkipForward className="w-3.5 h-3.5" />
                </button>
              )}

              {!isLast && (
                <button
                  type="button"
                  onClick={next}
                  className="flex items-center gap-1.5 px-5 py-2 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors"
                >
                  Continue
                  <ChevronRight className="w-3.5 h-3.5" />
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
