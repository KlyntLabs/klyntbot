import { ipc } from "@shared/hooks/useIpc";
import { Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

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
  registerSave: (fn: () => Promise<void>) => void;
  onDirty: () => void;
}

export function InvestmentsForm({ registerSave, onDirty }: InvestmentsFormProps) {
  const [portfolioName, setPortfolioName] = useState("My Portfolio");
  const [investments, setInvestments] = useState<InvestmentEntry[]>([]);

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
    onDirty();
  };

  const removeInvestment = (key: string) => {
    setInvestments(investments.filter((i) => i.key !== key));
    onDirty();
  };

  const updateInvestment = (key: string, update: Partial<InvestmentEntry>) => {
    setInvestments(investments.map((i) => (i.key === key ? { ...i, ...update } : i)));
    onDirty();
  };

  // Track which entries have already been saved to prevent duplicates
  const savedKeysRef = useRef<Set<string>>(new Set());
  const portfolioIdRef = useRef<string | null>(null);
  const prevPortfolioNameRef = useRef(portfolioName);

  // Clear stale portfolio ref when name changes after a partial save
  if (prevPortfolioNameRef.current !== portfolioName) {
    prevPortfolioNameRef.current = portfolioName;
    portfolioIdRef.current = null;
  }

  useEffect(() => {
    registerSave(async () => {
      const validInvestments = investments.filter(
        (i) => (i.symbol.trim() || i.name.trim()) && !savedKeysRef.current.has(i.key),
      );
      if (validInvestments.length > 0 && portfolioName.trim()) {
        try {
          // Reuse existing portfolio if already created
          if (!portfolioIdRef.current) {
            const portfolio = await ipc<{ id: string }>("finance_portfolio_create", {
              params: {
                name: portfolioName.trim(),
                description: null,
                currency: null,
              },
            });
            portfolioIdRef.current = portfolio?.id ?? null;
          }
          if (portfolioIdRef.current) {
            const results = await Promise.allSettled(
              validInvestments.map((inv) =>
                ipc("finance_investment_create", {
                  params: {
                    portfolioId: portfolioIdRef.current,
                    assetType: inv.assetType,
                    symbol: inv.symbol.trim() || null,
                    name: inv.name.trim() || null,
                    quantity: Number.parseFloat(inv.quantity) || 0,
                    costBasis: inv.costBasis
                      ? Math.round(Number.parseFloat(inv.costBasis) * 100)
                      : 0,
                    currency: null,
                    purchaseDate: null,
                    notes: null,
                  },
                }),
              ),
            );
            for (let i = 0; i < results.length; i++) {
              if (results[i].status === "fulfilled")
                savedKeysRef.current.add(validInvestments[i].key);
              else
                console.error(
                  "Failed to save investment:",
                  (results[i] as PromiseRejectedResult).reason,
                );
            }
          }
        } catch (e) {
          console.error("Failed to save investments:", e);
        }
      }
    });
  }, [portfolioName, investments, registerSave]);

  return (
    <div>
      <h3 className="text-sm font-medium text-muted-foreground mb-1">Investments</h3>
      <p className="text-[11px] text-dim mb-4">
        Track your investment portfolio. You can add more later.
      </p>

      <div className="space-y-4">
        <label className="block">
          <span className="block text-xs font-medium text-muted-foreground mb-1.5">
            Portfolio name
          </span>
          <input
            type="text"
            value={portfolioName}
            onChange={(e) => {
              setPortfolioName(e.target.value);
              onDirty();
            }}
            className="w-full px-3 py-2 text-[13px] text-foreground bg-accent border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
          />
        </label>

        <div className="space-y-2 max-h-[200px] overflow-y-auto pr-1">
          {investments.map((inv) => (
            <div
              key={inv.key}
              className="bg-card rounded-lg border border-border-subtle p-3 space-y-2"
            >
              <div className="flex items-center gap-2">
                <select
                  value={inv.assetType}
                  onChange={(e) => updateInvestment(inv.key, { assetType: e.target.value })}
                  className="w-28 px-2 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
                >
                  {ASSET_TYPE_OPTIONS.map((t) => (
                    <option key={t.value} value={t.value} className="bg-popover">
                      {t.label}
                    </option>
                  ))}
                </select>
                <input
                  type="text"
                  value={inv.symbol}
                  onChange={(e) => updateInvestment(inv.key, { symbol: e.target.value })}
                  placeholder="Symbol (e.g. AAPL)"
                  className="flex-1 px-2 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
                <button
                  type="button"
                  onClick={() => removeInvestment(inv.key)}
                  className="p-1 text-dim hover:text-destructive transition-colors"
                >
                  <X className="size-3.5" />
                </button>
              </div>
              <div className="flex gap-2">
                <input
                  type="number"
                  value={inv.quantity}
                  onChange={(e) => updateInvestment(inv.key, { quantity: e.target.value })}
                  placeholder="Qty"
                  step="any"
                  className="w-24 px-2 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
                <input
                  type="number"
                  value={inv.costBasis}
                  onChange={(e) => updateInvestment(inv.key, { costBasis: e.target.value })}
                  placeholder="Cost basis"
                  step="0.01"
                  className="flex-1 px-2 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              </div>
            </div>
          ))}
        </div>

        <button
          type="button"
          onClick={addInvestment}
          className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <Plus className="size-3.5" />
          Add investment
        </button>
      </div>
    </div>
  );
}
