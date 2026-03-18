import { ipc } from "@shared/hooks/useIpc";
import { Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

const LIABILITY_TYPES = [
  "mortgage",
  "student_loan",
  "auto_loan",
  "credit_card",
  "personal_loan",
  "other",
] as const;
const LIABILITY_TYPE_OPTIONS = LIABILITY_TYPES.map((t) => ({
  value: t,
  label: t.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()),
}));

interface LiabilityEntry {
  key: string;
  name: string;
  liabilityType: string;
  principal: string;
  interestRate: string;
  monthlyPayment: string;
}

interface LiabilitiesFormProps {
  registerSave: (fn: () => Promise<void>) => void;
  onDirty: () => void;
}

export function LiabilitiesForm({ registerSave, onDirty }: LiabilitiesFormProps) {
  const [liabilities, setLiabilities] = useState<LiabilityEntry[]>([]);

  const addLiability = () => {
    setLiabilities([
      ...liabilities,
      {
        key: crypto.randomUUID(),
        name: "",
        liabilityType: "mortgage",
        principal: "",
        interestRate: "",
        monthlyPayment: "",
      },
    ]);
    onDirty();
  };

  const removeLiability = (key: string) => {
    setLiabilities(liabilities.filter((l) => l.key !== key));
    onDirty();
  };

  const updateLiability = (key: string, update: Partial<LiabilityEntry>) => {
    setLiabilities(liabilities.map((l) => (l.key === key ? { ...l, ...update } : l)));
    onDirty();
  };

  const savedKeysRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    registerSave(async () => {
      const valid = liabilities.filter(
        (l) => l.name.trim() && l.principal && !savedKeysRef.current.has(l.key),
      );
      if (valid.length > 0) {
        const results = await Promise.allSettled(
          valid.map((lia) =>
            ipc("finance_liability_create", {
              params: {
                name: lia.name.trim(),
                liabilityType: lia.liabilityType,
                principal: Math.round(Number.parseFloat(lia.principal) * 100),
                remaining: null,
                interestRate: lia.interestRate ? Number.parseFloat(lia.interestRate) : null,
                monthlyPayment: lia.monthlyPayment
                  ? Math.round(Number.parseFloat(lia.monthlyPayment) * 100)
                  : null,
                currency: null,
                dueDate: null,
                notes: null,
              },
            }),
          ),
        );
        for (let i = 0; i < results.length; i++) {
          if (results[i].status === "fulfilled") savedKeysRef.current.add(valid[i].key);
          else
            console.error(
              "Failed to save liability:",
              (results[i] as PromiseRejectedResult).reason,
            );
        }
      }
    });
  }, [liabilities, registerSave]);

  return (
    <div>
      <h3 className="text-[14px] font-medium text-muted-foreground mb-1">Liabilities</h3>
      <p className="text-[11px] text-dim mb-4">
        Track debts and loans for accurate net worth. You can add more later.
      </p>

      <div className="space-y-2 max-h-[240px] overflow-y-auto pr-1">
        {liabilities.map((lia) => (
          <div
            key={lia.key}
            className="bg-card rounded-lg border border-border-subtle p-3 space-y-2"
          >
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={lia.name}
                onChange={(e) => updateLiability(lia.key, { name: e.target.value })}
                placeholder="Liability name"
                className="flex-1 px-3 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
              <button
                type="button"
                onClick={() => removeLiability(lia.key)}
                className="p-1 text-dim hover:text-destructive transition-colors"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
            <div className="flex gap-2">
              <select
                value={lia.liabilityType}
                onChange={(e) => updateLiability(lia.key, { liabilityType: e.target.value })}
                className="flex-1 px-2 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              >
                {LIABILITY_TYPE_OPTIONS.map((t) => (
                  <option key={t.value} value={t.value} className="bg-popover">
                    {t.label}
                  </option>
                ))}
              </select>
              <input
                type="number"
                value={lia.principal}
                onChange={(e) => updateLiability(lia.key, { principal: e.target.value })}
                placeholder="Principal"
                step="0.01"
                className="w-28 px-2 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </div>
            <div className="flex gap-2">
              <input
                type="number"
                value={lia.interestRate}
                onChange={(e) => updateLiability(lia.key, { interestRate: e.target.value })}
                placeholder="Interest rate %"
                step="0.1"
                className="flex-1 px-2 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
              <input
                type="number"
                value={lia.monthlyPayment}
                onChange={(e) => updateLiability(lia.key, { monthlyPayment: e.target.value })}
                placeholder="Monthly payment"
                step="0.01"
                className="flex-1 px-2 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </div>
          </div>
        ))}
      </div>

      <button
        type="button"
        onClick={addLiability}
        className="flex items-center gap-1.5 mt-3 text-[12px] text-muted-foreground hover:text-foreground transition-colors"
      >
        <Plus className="w-3.5 h-3.5" />
        Add liability
      </button>
    </div>
  );
}
