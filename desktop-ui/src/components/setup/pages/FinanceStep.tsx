import { useOutletContext } from "react-router";
import type { SetupContext } from "../steps";

export function FinanceStep() {
  const { next } = useOutletContext<SetupContext>();

  return (
    <div>
      <h2 className="text-lg font-medium text-primary mb-2">Finance</h2>
      <p className="text-[13px] text-muted mb-6">
        Set up your financial accounts, budgets, and goals. You can configure these later in
        Settings.
      </p>
      <button
        type="button"
        onClick={next}
        className="px-5 py-2 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors"
      >
        Continue
      </button>
    </div>
  );
}
