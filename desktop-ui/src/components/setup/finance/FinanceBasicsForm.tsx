import { useState } from "react";
import { ipc } from "../../../hooks/useIpc";

const CURRENCIES = [
  "USD",
  "EUR",
  "GBP",
  "VND",
  "CNY",
  "JPY",
  "KRW",
  "AUD",
  "CAD",
  "CHF",
  "SGD",
  "INR",
  "BRL",
  "MXN",
] as const;

const PROACTIVITY_LEVELS = [
  { value: "full", label: "Full", desc: "Proactive alerts, analysis, and suggestions" },
  { value: "moderate", label: "Moderate", desc: "Alerts on important changes only" },
  { value: "reactive", label: "Reactive", desc: "Only respond when asked" },
] as const;

interface FinanceBasicsFormProps {
  onNext: () => void;
}

export function FinanceBasicsForm({ onNext }: FinanceBasicsFormProps) {
  const [currency, setCurrency] = useState("USD");
  const [proactivity, setProactivity] = useState("full");
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    setSaving(true);
    try {
      await ipc("config_update_section", {
        section: "finance",
        patch: { defaultCurrency: currency, proactivityLevel: proactivity },
      });
    } catch (e) {
      console.error("Failed to save finance basics:", e);
    } finally {
      setSaving(false);
    }
    onNext();
  };

  return (
    <div>
      <h3 className="text-[14px] font-medium text-secondary mb-4">Basics</h3>

      <div className="space-y-4">
        <div>
          <label
            htmlFor="fin-currency"
            className="block text-[12px] font-medium text-secondary mb-1.5"
          >
            Default currency
          </label>
          <select
            id="fin-currency"
            value={currency}
            onChange={(e) => setCurrency(e.target.value)}
            className="w-full px-3 py-2 text-[13px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-lg focus:outline-none focus:border-brand/50 transition-colors"
          >
            {CURRENCIES.map((c) => (
              <option key={c} value={c} className="bg-[#1a1a1a]">
                {c}
              </option>
            ))}
          </select>
        </div>

        <div>
          <span className="block text-[12px] font-medium text-secondary mb-2">
            Proactivity level
          </span>
          <div className="space-y-2">
            {PROACTIVITY_LEVELS.map((level) => (
              <label
                key={level.value}
                className={`flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${
                  proactivity === level.value
                    ? "border-brand/50 bg-brand/[0.06]"
                    : "border-white/[0.08] bg-white/[0.03] hover:bg-white/[0.05]"
                }`}
              >
                <input
                  type="radio"
                  name="proactivity"
                  value={level.value}
                  checked={proactivity === level.value}
                  onChange={() => setProactivity(level.value)}
                  className="mt-0.5 accent-brand"
                />
                <div>
                  <span className="text-[13px] font-medium text-primary">{level.label}</span>
                  <p className="text-[11px] text-dim mt-0.5">{level.desc}</p>
                </div>
              </label>
            ))}
          </div>
        </div>
      </div>

      <div className="mt-5 flex justify-end">
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
