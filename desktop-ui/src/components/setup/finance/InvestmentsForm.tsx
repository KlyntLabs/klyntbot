import { Plus, X } from "lucide-react";
import { useState } from "react";
import { ipc } from "../../../hooks/useIpc";

const ASSET_TYPES = [
  "stock",
  "etf",
  "crypto",
  "bond",
  "real_estate",
  "commodity",
  "other",
] as const;
const ASSET_TYPE_OPTIONS = ASSET_TYPES.map((t) => ({
  value: t,
  label: t.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()),
}));

interface InvestmentEntry {
  key: string;
  assetType: string;
  symbol: string;
  name: string;
  quantity: string;
  costBasis: string;
}

interface InvestmentsFormProps {
  onNext: () => void;
  onBack: () => void;
}

export function InvestmentsForm({ onNext, onBack }: InvestmentsFormProps) {
  const [portfolioName, setPortfolioName] = useState("My Portfolio");
  const [investments, setInvestments] = useState<InvestmentEntry[]>([]);
  const [saving, setSaving] = useState(false);

  const addInvestment = () => {
    setInvestments([
      ...investments,
      {
        key: crypto.randomUUID(),
        assetType: "stock",
        symbol: "",
        name: "",
        quantity: "",
        costBasis: "",
      },
    ]);
  };

  const removeInvestment = (key: string) => {
    setInvestments(investments.filter((i) => i.key !== key));
  };

  const updateInvestment = (key: string, update: Partial<InvestmentEntry>) => {
    setInvestments(investments.map((i) => (i.key === key ? { ...i, ...update } : i)));
  };

  const handleSave = async () => {
    const validInvestments = investments.filter((i) => i.symbol.trim() || i.name.trim());
    if (validInvestments.length > 0 && portfolioName.trim()) {
      setSaving(true);
      try {
        const portfolio = await ipc<{ id: string }>("finance_portfolio_create", {
          params: {
            name: portfolioName.trim(),
            description: null,
            currency: null,
          },
        });
        if (portfolio?.id) {
          await Promise.all(
            validInvestments.map((inv) =>
              ipc("finance_investment_create", {
                params: {
                  portfolioId: portfolio.id,
                  assetType: inv.assetType,
                  symbol: inv.symbol.trim() || null,
                  name: inv.name.trim() || null,
                  quantity: Number.parseFloat(inv.quantity) || 0,
                  costBasis: inv.costBasis ? Math.round(Number.parseFloat(inv.costBasis) * 100) : 0,
                  currency: null,
                  purchaseDate: null,
                  notes: null,
                },
              }),
            ),
          );
        }
      } catch (e) {
        console.error("Failed to save investments:", e);
      } finally {
        setSaving(false);
      }
    }
    onNext();
  };

  return (
    <div>
      <h3 className="text-[14px] font-medium text-secondary mb-1">Investments</h3>
      <p className="text-[11px] text-dim mb-4">
        Track your investment portfolio. You can add more later.
      </p>

      <div className="space-y-4">
        <label className="block">
          <span className="block text-[12px] font-medium text-secondary mb-1.5">
            Portfolio name
          </span>
          <input
            type="text"
            value={portfolioName}
            onChange={(e) => setPortfolioName(e.target.value)}
            className="w-full px-3 py-2 text-[13px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
          />
        </label>

        <div className="space-y-2 max-h-[200px] overflow-y-auto pr-1">
          {investments.map((inv) => (
            <div
              key={inv.key}
              className="bg-white/[0.03] rounded-lg border border-white/[0.06] p-3 space-y-2"
            >
              <div className="flex items-center gap-2">
                <select
                  value={inv.assetType}
                  onChange={(e) => updateInvestment(inv.key, { assetType: e.target.value })}
                  className="w-28 px-2 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors"
                >
                  {ASSET_TYPE_OPTIONS.map((t) => (
                    <option key={t.value} value={t.value} className="bg-[#1a1a1a]">
                      {t.label}
                    </option>
                  ))}
                </select>
                <input
                  type="text"
                  value={inv.symbol}
                  onChange={(e) => updateInvestment(inv.key, { symbol: e.target.value })}
                  placeholder="Symbol (e.g. AAPL)"
                  className="flex-1 px-2 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
                <button
                  type="button"
                  onClick={() => removeInvestment(inv.key)}
                  className="p-1 text-dim hover:text-destructive transition-colors"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
              <div className="flex gap-2">
                <input
                  type="number"
                  value={inv.quantity}
                  onChange={(e) => updateInvestment(inv.key, { quantity: e.target.value })}
                  placeholder="Qty"
                  step="any"
                  className="w-24 px-2 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
                <input
                  type="number"
                  value={inv.costBasis}
                  onChange={(e) => updateInvestment(inv.key, { costBasis: e.target.value })}
                  placeholder="Cost basis"
                  step="0.01"
                  className="flex-1 px-2 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </div>
            </div>
          ))}
        </div>

        <button
          type="button"
          onClick={addInvestment}
          className="flex items-center gap-1.5 text-[12px] text-muted hover:text-secondary transition-colors"
        >
          <Plus className="w-3.5 h-3.5" />
          Add investment
        </button>
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
