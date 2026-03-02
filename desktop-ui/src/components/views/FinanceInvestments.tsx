import { useState, useMemo } from 'react';
import { TrendingUp, TrendingDown } from 'lucide-react';
import { useQuery } from '../../hooks/useQuery';
import { useEvent } from '../../hooks/useEvent';
import { cn } from '../../lib/utils';
import { fmtMoney, toVnd, fmtCompact, retPct, COLORS } from '../../lib/finance';
import { FinanceLayout } from '../finance/FinanceLayout';
import { Card, SectionLabel } from '../finance/Card';
import { Donut } from '../finance/Donut';
import type { FinancePortfolio, FinanceInvestment } from '../../lib/types';
import { mockPortfolios, mockInvestments, mockExchangeRates } from '../../data/mockFinanceData';

export function FinanceInvestments() {
  const { data: portfolios, refetch: rP } = useQuery<FinancePortfolio[]>('finance_portfolios', undefined, mockPortfolios);
  const { data: investments, refetch: rI } = useQuery<FinanceInvestment[]>('finance_investments', undefined, mockInvestments);
  const { data: rates } = useQuery<Record<string, number>>('finance_exchange_rates', undefined, mockExchangeRates);

  const refetchAll = () => { rP(); rI(); };
  useEvent<{ entityKind: string }>('entity:updated', refetchAll);

  const [selectedPortfolio, setSelectedPortfolio] = useState<string | null>(null);

  const totalValue = useMemo(() => portfolios.reduce((s, p) => s + toVnd(p.totalValue, p.currency, rates), 0), [portfolios, rates]);
  const totalCost = useMemo(() => portfolios.reduce((s, p) => s + toVnd(p.totalCostBasis, p.currency, rates), 0), [portfolios, rates]);
  const totalReturn = retPct(totalValue, totalCost);

  const assetSegs = useMemo(() => {
    const m = new Map<string, number>();
    investments.forEach(inv => {
      m.set(inv.assetType, (m.get(inv.assetType) ?? 0) + toVnd(inv.currentValue ?? 0, inv.currency, rates));
    });
    return Array.from(m.entries()).sort((a, b) => b[1] - a[1]).map(([name, value], i) => ({ name, value, color: COLORS[i % COLORS.length] }));
  }, [investments, rates]);

  const portfolioSegs = useMemo(() =>
    portfolios.map((p, i) => ({ name: p.name, value: toVnd(p.totalValue, p.currency, rates), color: COLORS[i % COLORS.length] })),
    [portfolios, rates],
  );

  const filteredInvestments = selectedPortfolio
    ? investments.filter(i => i.portfolioId === selectedPortfolio)
    : investments;

  return (
    <FinanceLayout onRefresh={refetchAll}>
      <div className="grid grid-cols-12 gap-3 auto-rows-min">

        {/* ── Stats row ─────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-4 gap-3">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Total Value</p>
            <p className="text-[20px] font-light text-primary">{fmtCompact(totalValue)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Cost Basis</p>
            <p className="text-[20px] font-light text-muted">{fmtCompact(totalCost)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Total Return</p>
            <p className={cn('text-[20px] font-light', totalReturn >= 0 ? 'text-success' : 'text-destructive')}>
              {totalReturn >= 0 ? '+' : ''}{totalReturn}%
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Holdings</p>
            <p className="text-[20px] font-light text-primary">{investments.length}</p>
          </Card>
        </div>

        {/* ── Portfolio cards ─────────────────────────── */}
        <div className="col-span-12">
          <SectionLabel>Portfolios</SectionLabel>
          <div className="grid grid-cols-3 gap-3">
            {portfolios.map((p, i) => {
              const r = retPct(p.totalValue, p.totalCostBasis);
              const isSelected = p.id === selectedPortfolio;
              return (
                <Card
                  key={p.id}
                  className={cn('p-4 cursor-pointer transition-colors', isSelected ? 'ring-1 ring-brand bg-surface-base' : 'hover:bg-surface-base')}
                  onClick={() => setSelectedPortfolio(isSelected ? null : p.id)}
                >
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: COLORS[i % COLORS.length] }} />
                      <span className="text-[13px] font-light text-secondary">{p.name}</span>
                    </div>
                    <div className="flex items-center gap-1">
                      {r >= 0 ? <TrendingUp className="w-3 h-3 text-success" strokeWidth={1.5} /> : <TrendingDown className="w-3 h-3 text-destructive" strokeWidth={1.5} />}
                      <span className={cn('text-[11px] font-light', r >= 0 ? 'text-success' : 'text-destructive')}>{r >= 0 ? '+' : ''}{r}%</span>
                    </div>
                  </div>
                  <p className="text-[18px] font-light text-primary">{fmtMoney(p.totalValue, p.currency)}</p>
                  <div className="flex items-center gap-2 mt-1">
                    <span className="text-[9px] text-dim font-light">Cost: {fmtMoney(p.totalCostBasis, p.currency)}</span>
                    <span className="text-[9px] text-dim font-light">{p.holdingCount} holdings</span>
                  </div>
                  {p.description && <p className="text-[9px] text-dim font-light mt-2">{p.description}</p>}
                </Card>
              );
            })}
          </div>
        </div>

        {/* ── Holdings table (9col) + Allocation (3col) ── */}
        <div className="col-span-9">
          <SectionLabel>Holdings {selectedPortfolio ? `— ${portfolios.find(p => p.id === selectedPortfolio)?.name}` : ''}</SectionLabel>
          <Card className="overflow-hidden">
            <div className="grid grid-cols-[1fr_80px_80px_90px_80px_80px] gap-2 border-b border-border text-[10px] text-dim font-light px-4 py-2">
              <div>Asset</div>
              <div className="text-right">Qty</div>
              <div className="text-right">Price</div>
              <div className="text-right">Value</div>
              <div className="text-right">Cost</div>
              <div className="text-right">Return</div>
            </div>
            {filteredInvestments.map(inv => {
              const r = retPct(inv.currentValue ?? 0, inv.costBasis);
              return (
                <div key={inv.id} className="grid grid-cols-[1fr_80px_80px_90px_80px_80px] gap-2 items-center px-4 py-2.5 hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] font-light text-primary">{inv.symbol ?? inv.name}</span>
                      <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-surface-base text-dim">{inv.assetType}</span>
                    </div>
                    <p className="text-[9px] text-dim font-light truncate">{inv.name}</p>
                  </div>
                  <span className="text-right text-[11px] text-muted font-light">{inv.quantity}</span>
                  <span className="text-right text-[11px] text-muted font-light">{inv.currentPrice != null ? fmtMoney(inv.currentPrice, inv.currency) : '—'}</span>
                  <span className="text-right text-[12px] text-primary font-light">{inv.currentValue != null ? fmtMoney(inv.currentValue, inv.currency) : '—'}</span>
                  <span className="text-right text-[11px] text-dim font-light">{fmtMoney(inv.costBasis, inv.currency)}</span>
                  <span className={cn('text-right text-[11px] font-light', r >= 0 ? 'text-success' : 'text-destructive')}>{r >= 0 ? '+' : ''}{r}%</span>
                </div>
              );
            })}
          </Card>
        </div>

        <div className="col-span-3 space-y-3">
          <div>
            <SectionLabel>Asset Allocation</SectionLabel>
            <Card className="p-4 flex items-center justify-center">
              <Donut segments={assetSegs} label="By type" value={fmtCompact(totalValue) + 'đ'} size={140} />
            </Card>
          </div>
          <div>
            <SectionLabel>By Portfolio</SectionLabel>
            <Card className="p-4 flex items-center justify-center">
              <Donut segments={portfolioSegs} label="Portfolios" value={fmtCompact(totalValue) + 'đ'} size={140} />
            </Card>
          </div>
        </div>

      </div>
    </FinanceLayout>
  );
}
