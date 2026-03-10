import { useEffect, useState } from "react";
import { ipc } from "@shared/hooks/useIpc";

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
  registerSave: (fn: () => Promise<void>) => void;
  onDirty: () => void;
}

export function FinanceBasicsForm({ registerSave, onDirty }: FinanceBasicsFormProps) {
  const [currency, setCurrency] = useState("USD");
  const [proactivity, setProactivity] = useState("full");

  useEffect(() => {
    registerSave(async () => {
      try {
        await ipc("config_update_section", {
          section: "finance",
          patch: { defaultCurrency: currency, proactivityLevel: proactivity },
        });
      } catch (e) {
        console.error("Failed to save finance basics:", e);
      }
    });
  }, [currency, proactivity, registerSave]);

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
            onChange={(e) => {
              setCurrency(e.target.value);
              onDirty();
            }}
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
                  onChange={() => {
                    setProactivity(level.value);
                    onDirty();
                  }}
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
    </div>
  );
}
