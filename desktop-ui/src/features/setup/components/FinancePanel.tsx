import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AccountsForm } from "./finance/AccountsForm";
import { FinanceBasicsForm } from "./finance/FinanceBasicsForm";
import { FireForm } from "./finance/FireForm";
import { GoalsForm } from "./finance/GoalsForm";
import { IncomeForm } from "./finance/IncomeForm";
import { InvestmentsForm } from "./finance/InvestmentsForm";
import { LiabilitiesForm } from "./finance/LiabilitiesForm";

const SUB_STEPS = [
  "basics",
  "accounts",
  "budgeting",
  "fire",
  "investments",
  "liabilities",
  "goals",
] as const;
const SUB_STEP_LABELS: Record<(typeof SUB_STEPS)[number], string> = {
  basics: "Basics",
  accounts: "Accounts",
  budgeting: "Budgeting",
  fire: "FIRE",
  investments: "Investments",
  liabilities: "Liabilities",
  goals: "Goals",
};

interface FinancePanelProps {
  onComplete: () => void;
}

export function FinancePanel({ onComplete }: FinancePanelProps) {
  const [subStep, setSubStep] = useState(0);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const subSaveMap = useRef<Map<number, () => Promise<void>>>(new Map());

  const makeRegisterSave = useCallback(
    (index: number) => (fn: () => Promise<void>) => {
      subSaveMap.current.set(index, fn);
    },
    [],
  );

  const registerSaves = useMemo(
    () => SUB_STEPS.map((_, i) => makeRegisterSave(i)),
    [makeRegisterSave],
  );

  const [isDirty, setIsDirty] = useState(false);
  const markDirty = useCallback(() => setIsDirty(true), []);

  // Warn on page leave when there are unsaved changes
  useEffect(() => {
    if (!isDirty) return;
    const handler = (e: BeforeUnloadEvent) => {
      e.preventDefault();
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [isDirty]);

  const handleNext = async () => {
    setSaving(true);
    setSaveError(null);
    try {
      const save = subSaveMap.current.get(subStep);
      await save?.();
    } catch (e) {
      console.error("Failed to save finance step:", e);
      setSaveError(e instanceof Error ? e.message : "Failed to save. Please try again.");
      setSaving(false);
      return;
    }
    setSaving(false);
    setIsDirty(false);

    if (subStep < SUB_STEPS.length - 1) {
      setSubStep((s) => s + 1);
    } else {
      onComplete();
    }
  };

  const handleBack = () => {
    if (subStep > 0) setSubStep((s) => s - 1);
  };

  return (
    <div className="mt-4 bg-card rounded-xl border border-border p-6 animate-in slide-in-from-top-2 duration-300">
      {/* Mini progress */}
      <div className="flex items-center gap-1 mb-5">
        {SUB_STEPS.map((step, i) => (
          <button
            key={step}
            type="button"
            onClick={() => setSubStep(i)}
            className={`text-2xs px-2 py-0.5 rounded-full transition-colors ${
              i === subStep
                ? "bg-brand text-white"
                : i < subStep
                  ? "bg-brand/20 text-brand"
                  : "bg-accent text-dim"
            }`}
          >
            {SUB_STEP_LABELS[step]}
          </button>
        ))}
      </div>

      {/* Sub-forms — all rendered, hidden when inactive */}
      <div className={subStep !== 0 ? "hidden" : undefined}>
        <FinanceBasicsForm registerSave={registerSaves[0]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 1 ? "hidden" : undefined}>
        <AccountsForm registerSave={registerSaves[1]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 2 ? "hidden" : undefined}>
        <IncomeForm registerSave={registerSaves[2]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 3 ? "hidden" : undefined}>
        <FireForm registerSave={registerSaves[3]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 4 ? "hidden" : undefined}>
        <InvestmentsForm registerSave={registerSaves[4]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 5 ? "hidden" : undefined}>
        <LiabilitiesForm registerSave={registerSaves[5]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 6 ? "hidden" : undefined}>
        <GoalsForm registerSave={registerSaves[6]} onDirty={markDirty} />
      </div>

      {/* Error feedback */}
      {saveError && <p className="text-xs text-destructive mt-4">{saveError}</p>}

      {/* Navigation */}
      <div className="flex items-center justify-between mt-6 pt-4 border-t border-border">
        <div className="flex gap-2">
          {subStep > 0 && (
            <button
              type="button"
              onClick={handleBack}
              className="px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              Back
            </button>
          )}
          <button
            type="button"
            onClick={onComplete}
            className="px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            Skip
          </button>
        </div>
        <button
          type="button"
          onClick={handleNext}
          disabled={saving}
          className="px-4 py-1.5 text-xs font-medium text-white bg-brand hover:bg-brand-hover rounded-lg transition-colors disabled:opacity-50"
        >
          {saving ? "Saving..." : subStep === SUB_STEPS.length - 1 ? "Done" : "Next"}
        </button>
      </div>
    </div>
  );
}
