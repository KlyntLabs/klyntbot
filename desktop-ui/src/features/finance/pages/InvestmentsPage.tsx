import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { cn } from "@shared/lib/utils";
import type {
  FinanceInvestment,
  FinanceInvestmentCreateParams,
  FinancePortfolio,
  FinancePortfolioCreateParams,
} from "@shared/types";
import { Plus, TrendingDown, TrendingUp } from "lucide-react";
import { useMemo, useState } from "react";
import { Card, CardHeader } from "../components/Card";
import { Donut } from "../components/Donut";
import { FinanceLayout } from "../components/FinanceLayout";
import { FinanceSkeleton } from "../components/FinanceSkeleton";
import { FormField, FormModal, fieldClass } from "../components/FormModal";
import { useFinanceCurrency } from "../hooks/useFinanceCurrency";
import { usePrivacyMode } from "../hooks/usePrivacyMode";
import { displayAmount } from "../lib/displayAmount";
import { COLORS, fmtCompact, fmtMoney, retPct } from "../lib/finance";

export function FinanceInvestments() {
  const { mode, setMode, baseCurrency, rates, currencies, displayCur, convertTotal } =
    useFinanceCurrency();
  const { hidden, toggle } = usePrivacyMode();
  const {
    data: portfolios,
    loading,
    error,
    refetch: rP,
  } = useQuery<FinancePortfolio[]>("finance_portfolios", undefined, []);
  const { data: investments, refetch: rI } = useQuery<FinanceInvestment[]>(
    "finance_investments",
    undefined,
    [],
  );

  const refetchAll = () => {
    rP();
    rI();
  };
  useEvent<{ entityKind: string }>("entity:updated", refetchAll);

  const [selectedPortfolio, setSelectedPortfolio] = useState<string | null>(null);

  const totalValue = useMemo(
    () => investments.reduce((s, i) => s + (i.baseCurrentValue ?? 0), 0),
    [investments],
  );
  const totalCost = useMemo(
    () => investments.reduce((s, i) => s + (i.baseCostBasis ?? 0), 0),
    [investments],
  );
  const totalReturn = retPct(totalValue, totalCost);

  const assetSegs = useMemo(() => {
    const m = new Map<string, number>();
    for (const inv of investments) {
      m.set(inv.assetType, (m.get(inv.assetType) ?? 0) + (inv.baseCurrentValue ?? 0));
    }
    return Array.from(m.entries())
      .sort((a, b) => b[1] - a[1])
      .map(([name, value], i) => ({ name, value, color: COLORS[i % COLORS.length] }));
  }, [investments]);

  const portfolioSegs = useMemo(() => {
    const m = new Map<string, number>();
    for (const inv of investments) {
      m.set(inv.portfolioId, (m.get(inv.portfolioId) ?? 0) + (inv.baseCurrentValue ?? 0));
    }
    return portfolios.map((p, i) => ({
      name: p.name,
      value: m.get(p.id) ?? 0,
      color: COLORS[i % COLORS.length],
    }));
  }, [portfolios, investments]);

  const filteredInvestments = selectedPortfolio
    ? investments.filter((i) => i.portfolioId === selectedPortfolio)
    : investments;

  // ── Add Portfolio modal ──
  const [portfolioModalOpen, setPortfolioModalOpen] = useState(false);
  const [pfName, setPfName] = useState("");
  const [pfDescription, setPfDescription] = useState("");
  const [pfCurrency, setPfCurrency] = useState("VND");
  const { mutate: createPortfolio } = useMutation<FinancePortfolio, FinancePortfolioCreateParams>(
    "finance_portfolio_create",
    "params",
  );

  const handleCreatePortfolio = async () => {
    const result = await createPortfolio({
      name: pfName,
      description: pfDescription || undefined,
      currency: pfCurrency,
    });
    if (!result) return;
    setPortfolioModalOpen(false);
    setPfName("");
    setPfDescription("");
    refetchAll();
  };

  // ── Add Investment modal ──
  const [investModalOpen, setInvestModalOpen] = useState(false);
  const [invPortfolioId, setInvPortfolioId] = useState("");
  const [invAssetType, setInvAssetType] = useState("stock");
  const [invSymbol, setInvSymbol] = useState("");
  const [invName, setInvName] = useState("");
  const [invQuantity, setInvQuantity] = useState("");
  const [invCostBasis, setInvCostBasis] = useState("");
  const { mutate: createInvestment } = useMutation<
    FinanceInvestment,
    FinanceInvestmentCreateParams
  >("finance_investment_create", "params");

  const handleCreateInvestment = async () => {
    const result = await createInvestment({
      portfolioId: invPortfolioId,
      assetType: invAssetType,
      costBasis: Math.round(Number(invCostBasis) * 100),
      quantity: Number(invQuantity),
      symbol: invSymbol || undefined,
      name: invName || undefined,
    });
    if (!result) return;
    setInvestModalOpen(false);
    setInvSymbol("");
    setInvName("");
    setInvQuantity("");
    setInvCostBasis("");
    refetchAll();
  };

  if (loading && portfolios.length === 0) {
    return (
      <FinanceLayout
        hidden={hidden}
        onTogglePrivacy={toggle}
        currencyMode={mode}
        currencies={currencies}
        onSelectCurrency={setMode}
      >
        <FinanceSkeleton />
      </FinanceLayout>
    );
  }

  if (error && portfolios.length === 0) {
    return (
      <FinanceLayout
        hidden={hidden}
        onTogglePrivacy={toggle}
        currencyMode={mode}
        currencies={currencies}
        onSelectCurrency={setMode}
      >
        <Card className="p-6 text-center">
          <p className="text-[12px] text-destructive mb-2">{error.message}</p>
          <button
            type="button"
            onClick={refetchAll}
            className="text-[11px] text-brand hover:text-brand-hover transition-colors"
          >
            Retry
          </button>
        </Card>
      </FinanceLayout>
    );
  }

  return (
    <FinanceLayout
      hidden={hidden}
      onTogglePrivacy={toggle}
      currencyMode={mode}
      currencies={currencies}
      onSelectCurrency={setMode}
    >
      <div className="grid grid-cols-12 gap-4 auto-rows-min">
        {/* ── Stats row ─────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-4 gap-4">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Value
            </p>
            <p className="text-[24px] font-light text-primary tabular-nums">
              {fmtCompact(convertTotal(totalValue), displayCur, hidden)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Cost Basis
            </p>
            <p className="text-[24px] font-light text-muted tabular-nums">
              {fmtCompact(convertTotal(totalCost), displayCur, hidden)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Return
            </p>
            <p
              className={cn(
                "text-[24px] font-light tabular-nums",
                totalReturn >= 0 ? "text-success" : "text-destructive",
              )}
            >
              {totalReturn >= 0 ? "+" : ""}
              {totalReturn}%
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Holdings
            </p>
            <p className="text-[24px] font-light text-primary">{investments.length}</p>
          </Card>
        </div>

        {/* ── Portfolio cards ─────────────────────────── */}
        <div className="col-span-12">
          <Card className="p-4">
            <CardHeader
              title="Portfolios"
              action={
                <button
                  type="button"
                  onClick={() => setPortfolioModalOpen(true)}
                  className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors"
                >
                  <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Portfolio
                </button>
              }
            />
            <div className="grid grid-cols-3 gap-4">
              {portfolios.map((p, i) => {
                const r = retPct(p.totalValue, p.totalCostBasis);
                const isSelected = p.id === selectedPortfolio;
                return (
                  <div
                    key={p.id}
                    className={cn(
                      "glass-card p-4 cursor-pointer transition-colors",
                      isSelected ? "ring-1 ring-brand bg-white/[0.06]" : "hover:bg-white/[0.06]",
                    )}
                    onClick={() => setSelectedPortfolio(isSelected ? null : p.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") setSelectedPortfolio(isSelected ? null : p.id);
                    }}
                    role="button"
                    tabIndex={0}
                  >
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <div
                          className="w-2.5 h-2.5 rounded-full"
                          style={{ backgroundColor: COLORS[i % COLORS.length] }}
                        />
                        <span className="text-[13px] font-medium text-secondary">{p.name}</span>
                      </div>
                      <div className="flex items-center gap-1">
                        {r >= 0 ? (
                          <TrendingUp className="w-3 h-3 text-success" strokeWidth={1.5} />
                        ) : (
                          <TrendingDown className="w-3 h-3 text-destructive" strokeWidth={1.5} />
                        )}
                        <span
                          className={cn(
                            "text-[11px] font-light",
                            r >= 0 ? "text-success" : "text-destructive",
                          )}
                        >
                          {r >= 0 ? "+" : ""}
                          {r}%
                        </span>
                      </div>
                    </div>
                    <p className="text-[18px] font-light text-primary tabular-nums">
                      {fmtMoney(p.totalValue, p.currency, hidden)}
                    </p>
                    <div className="flex items-center gap-2 mt-1">
                      <span className="text-[9px] text-dim font-light">
                        Cost: {fmtMoney(p.totalCostBasis, p.currency, hidden)}
                      </span>
                      <span className="text-[9px] text-dim font-light">
                        {p.holdingCount} holdings
                      </span>
                    </div>
                    {p.description && (
                      <p className="text-[9px] text-dim font-light mt-2">{p.description}</p>
                    )}
                  </div>
                );
              })}
            </div>
          </Card>
        </div>

        {/* ── Holdings table (9col) + Allocation (3col) ── */}
        <div className="col-span-9">
          <Card className="overflow-hidden">
            <div className="px-4 pt-4">
              <CardHeader
                title={`Holdings${selectedPortfolio ? ` — ${portfolios.find((p) => p.id === selectedPortfolio)?.name}` : ""}`}
                action={
                  <button
                    type="button"
                    onClick={() => {
                      if (portfolios.length > 0 && !invPortfolioId)
                        setInvPortfolioId(selectedPortfolio ?? portfolios[0].id);
                      setInvestModalOpen(true);
                    }}
                    className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors"
                  >
                    <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Investment
                  </button>
                }
              />
            </div>
            <div className="grid grid-cols-[1fr_80px_80px_90px_80px_80px] gap-2 border-b border-white/[0.08] text-[10px] text-dim font-light px-4 py-2">
              <div>Asset</div>
              <div className="text-right">Qty</div>
              <div className="text-right">Price</div>
              <div className="text-right">Value</div>
              <div className="text-right">Cost</div>
              <div className="text-right">Return</div>
            </div>
            {filteredInvestments.map((inv) => {
              const r = retPct(inv.currentValue ?? 0, inv.costBasis);
              return (
                <div
                  key={inv.id}
                  className="grid grid-cols-[1fr_80px_80px_90px_80px_80px] gap-2 items-center px-4 py-2.5 hover:bg-white/[0.06] transition-colors border-b border-white/[0.04] last:border-b-0"
                >
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] font-medium text-primary">
                        {inv.symbol ?? inv.name}
                      </span>
                      <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-white/[0.06] text-dim">
                        {inv.assetType}
                      </span>
                    </div>
                    <p className="text-[9px] text-dim font-light truncate">{inv.name}</p>
                  </div>
                  <span className="text-right text-[11px] text-muted font-light tabular-nums">
                    {inv.quantity}
                  </span>
                  <span className="text-right text-[11px] text-muted font-light tabular-nums">
                    {inv.currentPrice != null
                      ? fmtMoney(inv.currentPrice, inv.marketCurrency ?? inv.currency, hidden)
                      : "—"}
                  </span>
                  <div className="text-right">
                    <span className="text-[12px] text-primary font-light tabular-nums">
                      {inv.currentValue != null
                        ? fmtMoney(inv.currentValue, inv.marketCurrency ?? inv.currency, hidden)
                        : "—"}
                    </span>
                    {mode === "multi" &&
                      (inv.marketCurrency ?? inv.currency) !== baseCurrency &&
                      inv.baseCurrentValue != null && (
                        <p className="text-[9px] text-dim font-light">
                          {fmtCompact(inv.baseCurrentValue, baseCurrency, hidden)}
                        </p>
                      )}
                  </div>
                  <span className="text-right text-[11px] text-dim font-light tabular-nums">
                    {displayAmount({
                      amount: inv.costBasis,
                      currency: inv.currency,
                      baseAmount: inv.baseCostBasis,
                      baseCurrency,
                      mode,
                      rates,
                      hidden,
                    })}
                  </span>
                  <span
                    className={cn(
                      "text-right text-[11px] font-light tabular-nums",
                      r >= 0 ? "text-success" : "text-destructive",
                    )}
                  >
                    {r >= 0 ? "+" : ""}
                    {r}%
                  </span>
                </div>
              );
            })}
          </Card>
        </div>

        <div className="col-span-3 space-y-4">
          <Card className="p-4">
            <CardHeader title="Asset Allocation" />
            <div className="flex items-center justify-center">
              <Donut
                segments={assetSegs}
                label="By type"
                value={fmtCompact(convertTotal(totalValue), displayCur, hidden)}
                size={140}
              />
            </div>
          </Card>
          <Card className="p-4">
            <CardHeader title="By Portfolio" />
            <div className="flex items-center justify-center">
              <Donut
                segments={portfolioSegs}
                label="Portfolios"
                value={fmtCompact(convertTotal(totalValue), displayCur, hidden)}
                size={140}
              />
            </div>
          </Card>
        </div>
      </div>

      {/* ── Add Portfolio Modal ──────────────────────── */}
      <FormModal
        open={portfolioModalOpen}
        onClose={() => setPortfolioModalOpen(false)}
        title="Add Portfolio"
        onSubmit={handleCreatePortfolio}
        canSubmit={pfName.trim().length > 0}
      >
        <FormField label="Portfolio Name">
          <input
            className={fieldClass}
            value={pfName}
            onChange={(e) => setPfName(e.target.value)}
            placeholder="e.g. Long-term Growth"
            autoFocus
          />
        </FormField>
        <FormField label="Description (optional)">
          <input
            className={fieldClass}
            value={pfDescription}
            onChange={(e) => setPfDescription(e.target.value)}
            placeholder="Portfolio description"
          />
        </FormField>
        <FormField label="Currency">
          <select
            className={fieldClass}
            value={pfCurrency}
            onChange={(e) => setPfCurrency(e.target.value)}
          >
            <option value="VND">VND</option>
            <option value="USD">USD</option>
            <option value="USDT">USDT</option>
          </select>
        </FormField>
      </FormModal>

      {/* ── Add Investment Modal ─────────────────────── */}
      <FormModal
        open={investModalOpen}
        onClose={() => setInvestModalOpen(false)}
        title="Add Investment"
        onSubmit={handleCreateInvestment}
        canSubmit={!!invPortfolioId && Number(invQuantity) > 0 && Number(invCostBasis) > 0}
      >
        <FormField label="Portfolio">
          <select
            className={fieldClass}
            value={invPortfolioId}
            onChange={(e) => setInvPortfolioId(e.target.value)}
          >
            <option value="" disabled>
              Select portfolio
            </option>
            {portfolios.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </FormField>
        <FormField label="Asset Type">
          <select
            className={fieldClass}
            value={invAssetType}
            onChange={(e) => setInvAssetType(e.target.value)}
          >
            <option value="stock">Stock</option>
            <option value="crypto">Crypto</option>
            <option value="etf">ETF</option>
            <option value="bond">Bond</option>
            <option value="real_estate">Real Estate</option>
            <option value="other">Other</option>
          </select>
        </FormField>
        <FormField label="Symbol (optional)">
          <input
            className={fieldClass}
            value={invSymbol}
            onChange={(e) => setInvSymbol(e.target.value)}
            placeholder="e.g. BTC, AAPL"
          />
        </FormField>
        <FormField label="Name (optional)">
          <input
            className={fieldClass}
            value={invName}
            onChange={(e) => setInvName(e.target.value)}
            placeholder="e.g. Bitcoin, Apple Inc."
          />
        </FormField>
        <FormField label="Quantity">
          <input
            className={fieldClass}
            type="number"
            value={invQuantity}
            onChange={(e) => setInvQuantity(e.target.value)}
            placeholder="0"
            step="any"
          />
        </FormField>
        <FormField label="Cost Basis (total)">
          <input
            className={fieldClass}
            type="number"
            value={invCostBasis}
            onChange={(e) => setInvCostBasis(e.target.value)}
            placeholder="0"
          />
        </FormField>
      </FormModal>
    </FinanceLayout>
  );
}
