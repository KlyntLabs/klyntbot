import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useState } from "react";

const BUDGET_METHODS = [
  { value: "standard", label: "Standard", desc: "Track spending by custom categories" },
  {
    value: "six_jar",
    label: "Six Jar",
    desc: "Allocate income into 6 buckets (essentials, savings, etc.)",
  },
] as const;

interface SixJarRatios {
  essentials: number;
  savings: number;
  investment: number;
  education: number;
  entertainment: number;
  charity: number;
}

const DEFAULT_RATIOS: SixJarRatios = {
  essentials: 55,
  savings: 10,
  investment: 10,
  education: 10,
  entertainment: 10,
  charity: 5,
};

const JAR_LABELS: Record<keyof SixJarRatios, string> = {
  essentials: "Essentials",
  savings: "Savings",
  investment: "Investment",
  education: "Education",
  entertainment: "Entertainment",
  charity: "Charity",
};

interface IncomeFormProps {
  registerSave: (fn: () => Promise<void>) => void;
  onDirty: () => void;
}

export function IncomeForm({ registerSave, onDirty }: IncomeFormProps) {
  const [method, setMethod] = useState("standard");
  const [ratios, setRatios] = useState<SixJarRatios>(DEFAULT_RATIOS);

  const total = Object.values(ratios).reduce((a, b) => a + b, 0);

  const updateRatio = (key: keyof SixJarRatios, value: number) => {
    setRatios((prev) => ({ ...prev, [key]: value }));
  };

  useEffect(() => {
    registerSave(async () => {
      try {
        const patch: Record<string, unknown> = {
          budgeting: {
            defaultMethod: method,
            ...(method === "six_jar" ? { sixJarRatios: ratios } : {}),
          },
        };
        await ipc("config_update_section", { section: "finance", patch });
      } catch (e) {
        console.error("Failed to save budgeting config:", e);
      }
    });
  }, [method, ratios, registerSave]);

  return (
    <div>
      <h3 className="text-sm font-medium text-muted-foreground mb-1">Budgeting</h3>
      <p className="text-[11px] text-dim mb-4">Choose how you want to manage your budget.</p>

      <div className="space-y-4">
        <div className="space-y-2">
          {BUDGET_METHODS.map((m) => (
            <label
              key={m.value}
              className={`flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${
                method === m.value
                  ? "border-brand/50 bg-brand/[0.06]"
                  : "border-border bg-card hover:bg-accent"
              }`}
            >
              <input
                type="radio"
                name="budget-method"
                value={m.value}
                checked={method === m.value}
                onChange={() => {
                  setMethod(m.value);
                  onDirty();
                }}
                className="mt-0.5 accent-brand"
              />
              <div>
                <span className="text-[13px] font-medium text-foreground">{m.label}</span>
                <p className="text-[11px] text-dim mt-0.5">{m.desc}</p>
              </div>
            </label>
          ))}
        </div>

        {method === "six_jar" && (
          <div className="bg-card rounded-lg border border-border-subtle p-3 space-y-2">
            <div className="flex items-center justify-between mb-1">
              <span className="text-xs font-medium text-muted-foreground">Jar ratios</span>
              <span
                className={`text-[11px] font-mono ${total === 100 ? "text-success" : "text-warning"}`}
              >
                {total}%
              </span>
            </div>
            {(Object.keys(JAR_LABELS) as (keyof SixJarRatios)[]).map((key) => (
              <label key={key} className="flex items-center gap-3">
                <span className="text-xs text-muted-foreground w-24">{JAR_LABELS[key]}</span>
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={5}
                  value={ratios[key]}
                  onChange={(e) => {
                    updateRatio(key, Number(e.target.value));
                    onDirty();
                  }}
                  className="flex-1 accent-brand"
                />
                <span className="text-xs text-muted-foreground font-mono w-10 text-right">
                  {ratios[key]}%
                </span>
              </label>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
