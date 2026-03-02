import { useState } from 'react';
import { Wallet, Plus, ArrowUpRight, ArrowDownRight, ArrowLeftRight } from 'lucide-react';
import { useQuery } from '../../hooks/useQuery';
import { useEvent } from '../../hooks/useEvent';
import { cn } from '../../lib/utils';
import { fmtMoney, toVnd, fmtVnd, ACCT_ICONS } from '../../lib/finance';
import { FinanceLayout } from '../finance/FinanceLayout';
import { Card, SectionLabel } from '../finance/Card';
import type { FinanceAccount, FinanceTransaction } from '../../lib/types';
import { mockAccounts, mockTransactions, mockExchangeRates } from '../../data/mockFinanceData';

export function FinanceAccounts() {
  const { data: accounts, refetch: rA } = useQuery<FinanceAccount[]>('finance_accounts', undefined, mockAccounts);
  const { data: transactions, refetch: rT } = useQuery<FinanceTransaction[]>('finance_transactions', undefined, mockTransactions);
  const { data: rates } = useQuery<Record<string, number>>('finance_exchange_rates', undefined, mockExchangeRates);

  const refetchAll = () => { rA(); rT(); };
  useEvent<{ entityKind: string }>('entity:updated', refetchAll);

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const active = accounts.filter(a => !a.isArchived);
  const archived = accounts.filter(a => a.isArchived);
  const selected = accounts.find(a => a.id === selectedId);
  const acctTxs = selectedId ? transactions.filter(t => t.accountId === selectedId) : [];

  const totalVnd = active.reduce((s, a) => s + toVnd(a.balance, a.currency, rates), 0);

  return (
    <FinanceLayout onRefresh={refetchAll}>
      <div className="grid grid-cols-12 gap-3 auto-rows-min">

        {/* ── Header stats ────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-3 gap-3">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Total Balance</p>
            <p className="text-[20px] font-light text-primary">{fmtVnd(totalVnd)}</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Active Accounts</p>
            <p className="text-[20px] font-light text-primary">{active.length}</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Currencies</p>
            <p className="text-[20px] font-light text-primary">{new Set(active.map(a => a.currency)).size}</p>
          </Card>
        </div>

        {/* ── Account grid (left 8col) + Detail (right 4col) ── */}
        <div className="col-span-8">
          <div className="flex items-center justify-between mb-2">
            <SectionLabel>Accounts</SectionLabel>
            <button className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors">
              <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Account
            </button>
          </div>
          <div className="grid grid-cols-2 gap-3">
            {active.map(acct => {
              const Icon = ACCT_ICONS[acct.accountType] ?? Wallet;
              const vnd = toVnd(acct.balance, acct.currency, rates);
              const isSelected = acct.id === selectedId;
              return (
                <Card
                  key={acct.id}
                  className={cn('p-4 cursor-pointer transition-colors', isSelected ? 'ring-1 ring-brand bg-surface-base' : 'hover:bg-surface-base')}
                  onClick={() => setSelectedId(isSelected ? null : acct.id)}
                >
                  <div className="flex items-center gap-3 mb-3">
                    <div className="w-9 h-9 rounded-lg bg-surface-raised flex items-center justify-center flex-shrink-0">
                      <Icon className="w-4 h-4 text-muted" strokeWidth={1.5} />
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="text-[12px] font-light text-secondary truncate">{acct.name}</p>
                      <p className="text-[9px] text-dim font-light">{acct.institution ?? acct.accountType.replace('_', ' ')}</p>
                    </div>
                  </div>
                  <p className="text-[18px] font-light text-primary">{fmtMoney(acct.balance, acct.currency)}</p>
                  {acct.currency !== 'VND' && <p className="text-[10px] text-dim font-light mt-0.5">≈ {fmtVnd(vnd)}</p>}
                  {acct.notes && <p className="text-[9px] text-dim font-light mt-2">{acct.notes}</p>}
                </Card>
              );
            })}
          </div>

          {archived.length > 0 && (
            <div className="mt-4">
              <SectionLabel>Archived</SectionLabel>
              <div className="grid grid-cols-2 gap-3">
                {archived.map(acct => {
                  const Icon = ACCT_ICONS[acct.accountType] ?? Wallet;
                  return (
                    <Card key={acct.id} className="p-4 opacity-50">
                      <div className="flex items-center gap-3">
                        <Icon className="w-4 h-4 text-dim" strokeWidth={1.5} />
                        <div className="min-w-0 flex-1">
                          <p className="text-[12px] font-light text-muted truncate">{acct.name}</p>
                          <p className="text-[10px] text-dim font-light">{fmtMoney(acct.balance, acct.currency)}</p>
                        </div>
                      </div>
                    </Card>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        {/* ── Right sidebar: selected account transactions ── */}
        <div className="col-span-4">
          <SectionLabel>{selected ? `${selected.name} Transactions` : 'Select an account'}</SectionLabel>
          {selected ? (
            <Card className="overflow-hidden">
              {acctTxs.length === 0 ? (
                <div className="p-6 text-center text-[11px] text-dim font-light">No transactions</div>
              ) : (
                acctTxs.map(tx => {
                  const TxI = tx.txType === 'income' ? ArrowDownRight : tx.txType === 'expense' ? ArrowUpRight : ArrowLeftRight;
                  const col = tx.txType === 'income' ? 'text-success' : tx.txType === 'expense' ? 'text-destructive' : 'text-info';
                  const pre = tx.txType === 'income' ? '+' : tx.txType === 'expense' ? '-' : '';
                  return (
                    <div key={tx.id} className="flex items-center gap-2 px-3 py-2 hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0">
                      <TxI className={cn('w-3 h-3 flex-shrink-0', col)} strokeWidth={1.5} />
                      <div className="min-w-0 flex-1">
                        <p className="text-[11px] font-light text-secondary truncate">{tx.counterparty ?? tx.notes ?? tx.txType}</p>
                        <p className="text-[9px] text-dim font-light">{tx.txDate}</p>
                      </div>
                      <span className={cn('text-[11px] font-light flex-shrink-0', col)}>{pre}{fmtMoney(tx.amount, tx.currency)}</span>
                    </div>
                  );
                })
              )}
            </Card>
          ) : (
            <Card className="p-8 flex items-center justify-center">
              <p className="text-[11px] text-dim font-light">Click an account to view its transactions</p>
            </Card>
          )}
        </div>

      </div>
    </FinanceLayout>
  );
}
