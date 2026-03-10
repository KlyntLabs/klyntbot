import { ChevronLeft, ChevronRight, SkipForward } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Outlet } from "react-router";
import type { SetupContext } from "../hooks/steps";
import { useSetupNavigation } from "../hooks/useSetupNavigation";
import { SetupProgress } from "./SetupProgress";

export function SetupLayout() {
  const { currentStep, currentIndex, isFirst, next, back } = useSetupNavigation();

  const forwardRef = useRef<((isSkip: boolean) => Promise<boolean>) | null>(null);
  const backRef = useRef<(() => boolean) | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);

  // Reset when the active step changes
  useEffect(() => {
    if (currentIndex !== undefined) {
      forwardRef.current = null;
      backRef.current = null;
      setDirty(false);
      setSaving(false);
    }
  }, [currentIndex]);

  const isWelcome = currentStep?.id === "welcome";
  const isComplete = currentStep?.id === "complete";
  const showNav = !isWelcome && !isComplete;
  const isSkip = !currentStep?.required && !dirty;

  const handleForward = async () => {
    if (forwardRef.current) {
      if (!isSkip) setSaving(true);
      try {
        const shouldNavigate = await forwardRef.current(isSkip);
        if (shouldNavigate) next();
      } finally {
        setSaving(false);
      }
    } else {
      next();
    }
  };

  const handleBack = () => {
    if (backRef.current) {
      const shouldNavigate = backRef.current();
      if (shouldNavigate) back();
    } else {
      back();
    }
  };

  const context: SetupContext = { forwardRef, backRef, setDirty };

  return (
    <div className="fixed inset-0 flex items-center justify-center bg-background">
      <div className="w-full max-w-xl mx-auto px-6">
        {showNav && (
          <div className="mb-6">
            <SetupProgress currentIndex={currentIndex} />
          </div>
        )}

        {/* Main card */}
        <div className="bg-surface-low rounded-2xl border border-border p-8">
          <Outlet context={context} />
        </div>

        {/* Consolidated navigation — Back + (Skip or Continue) */}
        {showNav && (
          <div className="flex items-center justify-between mt-6">
            <button
              type="button"
              onClick={handleBack}
              disabled={isFirst}
              className="flex items-center gap-1.5 px-4 py-2 text-[13px] text-muted hover:text-secondary transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            >
              <ChevronLeft className="w-3.5 h-3.5" />
              Back
            </button>

            <button
              type="button"
              onClick={handleForward}
              disabled={saving}
              className={
                isSkip
                  ? "flex items-center gap-1.5 px-4 py-2 text-[13px] text-muted hover:text-secondary transition-colors"
                  : "flex items-center gap-1.5 px-5 py-2 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors disabled:opacity-50"
              }
            >
              {saving ? "Saving..." : isSkip ? "Skip" : "Continue"}
              {!saving &&
                (isSkip ? (
                  <SkipForward className="w-3.5 h-3.5" />
                ) : (
                  <ChevronRight className="w-3.5 h-3.5" />
                ))}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
