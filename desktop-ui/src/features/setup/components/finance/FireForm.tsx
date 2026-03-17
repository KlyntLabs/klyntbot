import { ipc } from "@shared/hooks/useIpc";
import { Toggle } from "@shared/ui";
import { useEffect, useState } from "react";

const FIRE_TYPES = [
  { value: "lean", label: "Lean FIRE", desc: "Minimal expenses, frugal lifestyle" },
  { value: "regular", label: "Regular FIRE", desc: "Comfortable middle-class lifestyle" },
  { value: "fat", label: "Fat FIRE", desc: "Generous spending, no compromise" },
  { value: "coast", label: "Coast FIRE", desc: "Save enough to coast to retirement" },
] as const;

interface FireFormProps {
  registerSave: (fn: () => Promise<void>) => void;
  onDirty: () => void;
}

export function FireForm({ registerSave, onDirty }: FireFormProps) {
  const [enabled, setEnabled] = useState(false);
  const [currentAge, setCurrentAge] = useState("");
  const [retirementAge, setRetirementAge] = useState("");
  const [annualExpenses, setAnnualExpenses] = useState("");
  const [swr, setSwr] = useState("4.0");
  const [fireType, setFireType] = useState("regular");

  const expenses = Number.parseFloat(annualExpenses) || 0;
  const rate = Number.parseFloat(swr) || 4;
  const fireNumber = rate > 0 ? Math.round((expenses / (rate / 100)) * 100) / 100 : 0;

  useEffect(() => {
    registerSave(async () => {
      if (!enabled) return;
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
      }
    });
  }, [enabled, currentAge, retirementAge, expenses, rate, fireType, registerSave]);

  return (
    <div>
      <h3 className="text-[14px] font-medium text-muted-foreground mb-1">FIRE Planning</h3>
      <p className="text-[11px] text-dim mb-4">
        Financial Independence, Retire Early. Track your path to freedom.
      </p>

      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <span className="text-[13px] text-muted-foreground">Enable FIRE tracking</span>
          <Toggle
            checked={enabled}
            onChange={(v) => {
              setEnabled(v);
              onDirty();
            }}
          />
        </div>

        {enabled && (
          <>
            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[12px] font-medium text-muted-foreground mb-1.5">
                  Current age
                </span>
                <input
                  type="number"
                  value={currentAge}
                  onChange={(e) => {
                    setCurrentAge(e.target.value);
                    onDirty();
                  }}
                  placeholder="30"
                  className="w-full px-3 py-2 text-[13px] text-foreground bg-accent border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </label>
              <label className="flex-1">
                <span className="block text-[12px] font-medium text-muted-foreground mb-1.5">
                  Target retirement age
                </span>
                <input
                  type="number"
                  value={retirementAge}
                  onChange={(e) => {
                    setRetirementAge(e.target.value);
                    onDirty();
                  }}
                  placeholder="45"
                  className="w-full px-3 py-2 text-[13px] text-foreground bg-accent border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </label>
            </div>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[12px] font-medium text-muted-foreground mb-1.5">
                  Annual expenses
                </span>
                <input
                  type="number"
                  value={annualExpenses}
                  onChange={(e) => {
                    setAnnualExpenses(e.target.value);
                    onDirty();
                  }}
                  placeholder="40000"
                  step="100"
                  className="w-full px-3 py-2 text-[13px] text-foreground bg-accent border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </label>
              <label className="flex-1">
                <span className="block text-[12px] font-medium text-muted-foreground mb-1.5">
                  Safe withdrawal rate (%)
                </span>
                <input
                  type="number"
                  value={swr}
                  onChange={(e) => {
                    setSwr(e.target.value);
                    onDirty();
                  }}
                  step="0.1"
                  className="w-full px-3 py-2 text-[13px] text-foreground bg-accent border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </label>
            </div>

            <div>
              <span className="block text-[12px] font-medium text-muted-foreground mb-2">FIRE type</span>
              <div className="grid grid-cols-2 gap-2">
                {FIRE_TYPES.map((ft) => (
                  <label
                    key={ft.value}
                    className={`p-2.5 rounded-lg border cursor-pointer transition-colors ${
                      fireType === ft.value
                        ? "border-brand/50 bg-brand/[0.06]"
                        : "border-border bg-card hover:bg-accent"
                    }`}
                  >
                    <input
                      type="radio"
                      name="fire-type"
                      value={ft.value}
                      checked={fireType === ft.value}
                      onChange={() => {
                        setFireType(ft.value);
                        onDirty();
                      }}
                      className="sr-only"
                    />
                    <span className="text-[12px] font-medium text-foreground">{ft.label}</span>
                    <p className="text-[10px] text-dim mt-0.5">{ft.desc}</p>
                  </label>
                ))}
              </div>
            </div>

            {fireNumber > 0 && (
              <div className="bg-brand/[0.08] border border-brand/20 rounded-lg p-3 text-center">
                <span className="text-[11px] text-muted-foreground">Your FIRE number</span>
                <p className="text-[18px] font-semibold text-brand mt-1">
                  ${fireNumber.toLocaleString()}
                </p>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
