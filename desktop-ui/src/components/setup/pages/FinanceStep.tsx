import { useState } from "react";
import { useOutletContext } from "react-router";
import { AccountsForm } from "../finance/AccountsForm";
import { FinanceBasicsForm } from "../finance/FinanceBasicsForm";
import { FireForm } from "../finance/FireForm";
import { GoalsForm } from "../finance/GoalsForm";
import { IncomeForm } from "../finance/IncomeForm";
import { InvestmentsForm } from "../finance/InvestmentsForm";
import { LiabilitiesForm } from "../finance/LiabilitiesForm";
import type { SetupContext } from "../steps";

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
  const { next } = useOutletContext<SetupContext>();
  const [subStep, setSubStep] = useState(0);

  const goNext = () => {
    if (subStep < SUB_STEPS.length - 1) {
      setSubStep(subStep + 1);
    } else {
      next();
    }
  };

  const goBack = () => {
    if (subStep > 0) setSubStep(subStep - 1);
  };

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

      {/* Sub-step content */}
      {subStep === 0 && <FinanceBasicsForm onNext={goNext} />}
      {subStep === 1 && <AccountsForm onNext={goNext} onBack={goBack} />}
      {subStep === 2 && <IncomeForm onNext={goNext} onBack={goBack} />}
      {subStep === 3 && <FireForm onNext={goNext} onBack={goBack} />}
      {subStep === 4 && <InvestmentsForm onNext={goNext} onBack={goBack} />}
      {subStep === 5 && <LiabilitiesForm onNext={goNext} onBack={goBack} />}
      {subStep === 6 && <GoalsForm onDone={goNext} onBack={goBack} />}
    </div>
  );
}
