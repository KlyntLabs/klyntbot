import { useState } from "react";
import { ipc } from "../../../hooks/useIpc";

const FIRE_TYPES = [
  { value: "lean", label: "Lean FIRE", desc: "Minimal expenses, frugal lifestyle" },
  { value: "regular", label: "Regular FIRE", desc: "Comfortable middle-class lifestyle" },
  { value: "fat", label: "Fat FIRE", desc: "Generous spending, no compromise" },
  { value: "coast", label: "Coast FIRE", desc: "Save enough to coast to retirement" },
] as const;

interface FireFormProps {
  onNext: () => void;
  onBack: () => void;
}

export function FireForm({ onNext, onBack }: FireFormProps) {
  const [enabled, setEnabled] = useState(false);
  const [currentAge, setCurrentAge] = useState("");
  const [retirementAge, setRetirementAge] = useState("");
  const [annualExpenses, setAnnualExpenses] = useState("");
  const [swr, setSwr] = useState("4.0");
  const [fireType, setFireType] = useState("regular");
  const [saving, setSaving] = useState(false);

  const expenses = Number.parseFloat(annualExpenses) || 0;
  const rate = Number.parseFloat(swr) || 4;
  const fireNumber = rate > 0 ? Math.round((expenses / (rate / 100)) * 100) / 100 : 0;

  const handleSave = async () => {
    if (!enabled) {
      onNext();
      return;
    }
    setSaving(true);
    try {
      await ipc("config_update_section", {
        section: "finance",
        patch: {
          fire: {
            enabled: true,
            currentAge: currentAge ? Number.parseInt(currentAge, 10) : null,
            targetRetirementAge: retirementAge ? Number.parseInt(retirementAge, 10) : null,
            annualExpenses: expenses ? Math.round(expenses * 100) : null,
            safeWithdrawalRate: rate,
            fireType,
            targetNumber: fireNumber > 0 ? Math.round(fireNumber * 100) : null,
          },
        },
      });
    } catch (e) {
      console.error("Failed to save FIRE config:", e);
    } finally {
      setSaving(false);
    }
    onNext();
  };

  return (
    <div>
      <h3 className="text-[14px] font-medium text-secondary mb-1">FIRE Planning</h3>
      <p className="text-[11px] text-dim mb-4">
        Financial Independence, Retire Early. Track your path to freedom.
      </p>

      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <span className="text-[13px] text-secondary">Enable FIRE tracking</span>
          <button
            type="button"
            onClick={() => setEnabled(!enabled)}
            className={`relative w-9 h-5 rounded-full transition-colors ${
              enabled ? "bg-brand" : "bg-white/[0.1]"
            }`}
          >
            <span
              className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
                enabled ? "translate-x-4" : ""
              }`}
            />
          </button>
        </div>

        {enabled && (
          <>
            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[12px] font-medium text-secondary mb-1.5">
                  Current age
                </span>
                <input
                  type="number"
                  value={currentAge}
                  onChange={(e) => setCurrentAge(e.target.value)}
                  placeholder="30"
                  className="w-full px-3 py-2 text-[13px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </label>
              <label className="flex-1">
                <span className="block text-[12px] font-medium text-secondary mb-1.5">
                  Target retirement age
                </span>
                <input
                  type="number"
                  value={retirementAge}
                  onChange={(e) => setRetirementAge(e.target.value)}
                  placeholder="45"
                  className="w-full px-3 py-2 text-[13px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </label>
            </div>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[12px] font-medium text-secondary mb-1.5">
                  Annual expenses
                </span>
                <input
                  type="number"
                  value={annualExpenses}
                  onChange={(e) => setAnnualExpenses(e.target.value)}
                  placeholder="40000"
                  step="100"
                  className="w-full px-3 py-2 text-[13px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </label>
              <label className="flex-1">
                <span className="block text-[12px] font-medium text-secondary mb-1.5">
                  Safe withdrawal rate (%)
                </span>
                <input
                  type="number"
                  value={swr}
                  onChange={(e) => setSwr(e.target.value)}
                  step="0.1"
                  className="w-full px-3 py-2 text-[13px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </label>
            </div>

            <div>
              <span className="block text-[12px] font-medium text-secondary mb-2">FIRE type</span>
              <div className="grid grid-cols-2 gap-2">
                {FIRE_TYPES.map((ft) => (
                  <label
                    key={ft.value}
                    className={`p-2.5 rounded-lg border cursor-pointer transition-colors ${
                      fireType === ft.value
                        ? "border-brand/50 bg-brand/[0.06]"
                        : "border-white/[0.08] bg-white/[0.03] hover:bg-white/[0.05]"
                    }`}
                  >
                    <input
                      type="radio"
                      name="fire-type"
                      value={ft.value}
                      checked={fireType === ft.value}
                      onChange={() => setFireType(ft.value)}
                      className="sr-only"
                    />
                    <span className="text-[12px] font-medium text-primary">{ft.label}</span>
                    <p className="text-[10px] text-dim mt-0.5">{ft.desc}</p>
                  </label>
                ))}
              </div>
            </div>

            {fireNumber > 0 && (
              <div className="bg-brand/[0.08] border border-brand/20 rounded-lg p-3 text-center">
                <span className="text-[11px] text-muted">Your FIRE number</span>
                <p className="text-[18px] font-semibold text-brand mt-1">
                  ${fireNumber.toLocaleString()}
                </p>
              </div>
            )}
          </>
        )}
      </div>

      <div className="mt-5 flex justify-between">
        <button
          type="button"
          onClick={onBack}
          className="px-4 py-2 text-[13px] text-muted hover:text-secondary transition-colors"
        >
          Back
        </button>
        <button
          type="button"
          onClick={handleSave}
          disabled={saving}
          className="px-5 py-2 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors disabled:opacity-50"
        >
          {saving ? "Saving..." : "Next"}
        </button>
      </div>
    </div>
  );
}
