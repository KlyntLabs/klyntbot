import { Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { ipc } from "@shared/hooks/useIpc";

const ACCOUNT_TYPES = ["checking", "savings", "credit", "investment", "cash", "other"] as const;
const ACCOUNT_TYPE_OPTIONS = ACCOUNT_TYPES.map((t) => ({
  value: t,
  label: t.charAt(0).toUpperCase() + t.slice(1),
}));

interface AccountEntry {
  key: string;
  name: string;
  accountType: string;
  balance: string;
  institution: string;
}

interface AccountsFormProps {
  registerSave: (fn: () => Promise<void>) => void;
  onDirty: () => void;
}

export function AccountsForm({ registerSave, onDirty }: AccountsFormProps) {
  const [accounts, setAccounts] = useState<AccountEntry[]>([]);

  const addAccount = () => {
    setAccounts([
      ...accounts,
      { key: crypto.randomUUID(), name: "", accountType: "checking", balance: "", institution: "" },
    ]);
    onDirty();
  };

  const removeAccount = (key: string) => {
    setAccounts(accounts.filter((a) => a.key !== key));
    onDirty();
  };

  const updateAccount = (key: string, update: Partial<AccountEntry>) => {
    setAccounts(accounts.map((a) => (a.key === key ? { ...a, ...update } : a)));
    onDirty();
  };

  // Track which entries have already been saved to prevent duplicates
  const savedKeysRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    registerSave(async () => {
      const valid = accounts.filter((a) => a.name.trim() && !savedKeysRef.current.has(a.key));
      if (valid.length > 0) {
        const results = await Promise.allSettled(
          valid.map((acc) =>
            ipc("finance_account_create", {
              params: {
                name: acc.name.trim(),
                accountType: acc.accountType,
                balance: acc.balance ? Math.round(Number.parseFloat(acc.balance) * 100) : null,
                institution: acc.institution.trim() || null,
                currency: null,
                notes: null,
              },
            }),
          ),
        );
        for (let i = 0; i < results.length; i++) {
          if (results[i].status === "fulfilled") savedKeysRef.current.add(valid[i].key);
          else
            console.error("Failed to save account:", (results[i] as PromiseRejectedResult).reason);
        }
      }
    });
  }, [accounts, registerSave]);

  return (
    <div>
      <h3 className="text-[14px] font-medium text-secondary mb-1">Accounts</h3>
      <p className="text-[11px] text-dim mb-4">Add your bank accounts. You can add more later.</p>

      <div className="space-y-2 max-h-[240px] overflow-y-auto pr-1">
        {accounts.map((acc) => (
          <div
            key={acc.key}
            className="bg-white/[0.03] rounded-lg border border-white/[0.06] p-3 space-y-2"
          >
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={acc.name}
                onChange={(e) => updateAccount(acc.key, { name: e.target.value })}
                placeholder="Account name"
                className="flex-1 px-3 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
              <button
                type="button"
                onClick={() => removeAccount(acc.key)}
                className="p-1 text-dim hover:text-destructive transition-colors"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
            <div className="flex gap-2">
              <select
                value={acc.accountType}
                onChange={(e) => updateAccount(acc.key, { accountType: e.target.value })}
                className="flex-1 px-2 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              >
                {ACCOUNT_TYPE_OPTIONS.map((t) => (
                  <option key={t.value} value={t.value} className="bg-[#1a1a1a]">
                    {t.label}
                  </option>
                ))}
              </select>
              <input
                type="number"
                value={acc.balance}
                onChange={(e) => updateAccount(acc.key, { balance: e.target.value })}
                placeholder="Balance"
                step="0.01"
                className="w-28 px-2 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
              <input
                type="text"
                value={acc.institution}
                onChange={(e) => updateAccount(acc.key, { institution: e.target.value })}
                placeholder="Institution"
                className="flex-1 px-2 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </div>
          </div>
        ))}
      </div>

      <button
        type="button"
        onClick={addAccount}
        className="flex items-center gap-1.5 mt-3 text-[12px] text-muted hover:text-secondary transition-colors"
      >
        <Plus className="w-3.5 h-3.5" />
        Add account
      </button>
    </div>
  );
}
