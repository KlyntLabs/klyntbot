import { Sparkles } from "lucide-react";
import { useOutletContext } from "react-router";
import type { SetupContext } from "../steps";

export function WelcomeStep() {
  const { next } = useOutletContext<SetupContext>();

  return (
    <div className="text-center py-4">
      <Sparkles className="w-10 h-10 text-brand mx-auto mb-4" strokeWidth={1.5} />
      <h1 className="text-2xl font-semibold text-primary mb-3">Welcome to Klynt</h1>
      <p className="text-[13px] text-muted leading-relaxed mb-8 max-w-sm mx-auto">
        Your personal AI agent for task management, finance tracking, and productivity. Let's get
        you set up in a few quick steps.
      </p>
      <button
        type="button"
        onClick={next}
        className="px-6 py-2.5 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors"
      >
        Get Started
      </button>
    </div>
  );
}
