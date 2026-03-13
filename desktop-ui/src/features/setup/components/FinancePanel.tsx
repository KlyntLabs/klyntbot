import { useCallback, useMemo, useRef, useState } from "react";
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

  const markDirty = useCallback(() => {}, []); // No-op — panel doesn't need dirty tracking

  const handleNext = async () => {
    setSaving(true);
    try {
      const save = subSaveMap.current.get(subStep);
      await save?.();
    } finally {
      setSaving(false);
    }

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
    <div className="mt-4 bg-surface-low rounded-xl border border-border p-6 animate-in slide-in-from-top-2 duration-300">
      {/* Mini progress */}
      <div className="flex items-center gap-1 mb-5">
        {SUB_STEPS.map((step, i) => (
          <button
            key={step}
            type="button"
            onClick={() => setSubStep(i)}
            className={`text-[10px] px-2 py-0.5 rounded-full transition-colors ${
              i === subStep
                ? "bg-brand text-white"
                : i < subStep
                  ? "bg-brand/20 text-brand"
                  : "bg-white/[0.06] text-dim"
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

      {/* Navigation */}
      <div className="flex items-center justify-between mt-6 pt-4 border-t border-border">
        <div className="flex gap-2">
          {subStep > 0 && (
            <button
              type="button"
              onClick={handleBack}
              className="px-3 py-1.5 text-[12px] text-muted hover:text-secondary transition-colors"
            >
              Back
            </button>
          )}
          <button
            type="button"
            onClick={onComplete}
            className="px-3 py-1.5 text-[12px] text-muted hover:text-secondary transition-colors"
          >
            Skip
          </button>
        </div>
        <button
          type="button"
          onClick={handleNext}
          disabled={saving}
          className="px-4 py-1.5 text-[12px] font-medium text-white bg-brand hover:bg-brand-hover rounded-lg transition-colors disabled:opacity-50"
        >
          {saving ? "Saving..." : subStep === SUB_STEPS.length - 1 ? "Done" : "Next"}
        </button>
      </div>
    </div>
  );
}
