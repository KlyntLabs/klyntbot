import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useOutletContext } from "react-router";
import { AccountsForm } from "../components/finance/AccountsForm";
import { FinanceBasicsForm } from "../components/finance/FinanceBasicsForm";
import { FireForm } from "../components/finance/FireForm";
import { GoalsForm } from "../components/finance/GoalsForm";
import { IncomeForm } from "../components/finance/IncomeForm";
import { InvestmentsForm } from "../components/finance/InvestmentsForm";
import { LiabilitiesForm } from "../components/finance/LiabilitiesForm";
import type { SetupContext } from "../hooks/steps";

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

export function FinanceStep() {
  const { forwardRef, backRef, setDirty } = useOutletContext<SetupContext>();
  const [subStep, setSubStep] = useState(0);

  // Map of sub-step index → save function (all sub-forms are mounted, each registers here)
  const subSaveMap = useRef<Map<number, () => Promise<void>>>(new Map());

  const makeRegisterSave = useCallback(
    (index: number) => (fn: () => Promise<void>) => {
      subSaveMap.current.set(index, fn);
    },
    [],
  );

  // Memoize per-sub-step registerSave functions so child useEffects don't re-fire on every render
  const registerSaves = useMemo(
    () => SUB_STEPS.map((_, i) => makeRegisterSave(i)),
    [makeRegisterSave],
  );

  const markDirty = useCallback(() => setDirty(true), [setDirty]);

  // Register forward handler — advances through sub-steps
  useEffect(() => {
    forwardRef.current = async (isSkip: boolean) => {
      if (!isSkip) {
        const save = subSaveMap.current.get(subStep);
        await save?.();
      }
      if (subStep < SUB_STEPS.length - 1) {
        setSubStep((s) => s + 1);
        setDirty(false);
        return false; // don't navigate to next main step
      }
      return true; // last sub-step, navigate to MCP
    };
  }, [forwardRef, setDirty, subStep]);

  // Register back handler — goes to previous sub-step or back to Productivity
  useEffect(() => {
    backRef.current = () => {
      if (subStep > 0) {
        setSubStep((s) => s - 1);
        setDirty(false);
        return false; // don't navigate to previous main step
      }
      return true; // first sub-step, navigate to Productivity
    };
  }, [backRef, setDirty, subStep]);

  return (
    <div>
      <h2 className="text-lg font-medium text-primary mb-1">Finance</h2>
      <p className="text-[13px] text-muted mb-4">
        Set up your financial accounts, budgets, and goals. All sub-steps are optional.
      </p>

      {/* Mini progress indicator */}
      <div className="flex items-center gap-1 mb-5">
        {SUB_STEPS.map((step, i) => (
          <div key={step} className="flex items-center gap-1">
            <button
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
          </div>
        ))}
      </div>

      {/* All sub-forms rendered, hidden when inactive — preserves state across navigation */}
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
    </div>
  );
}
