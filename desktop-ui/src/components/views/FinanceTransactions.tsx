import { ArrowDownRight, ArrowLeftRight, ArrowUpRight, Plus, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { useEvent } from "../../hooks/useEvent";
import { useQuery } from "../../hooks/useQuery";
import { COLORS, fmtCompact, fmtMoney, toVnd } from "../../lib/finance";
import type { FinanceAccount, FinanceTransaction } from "../../lib/types";
import { cn } from "../../lib/utils";
import { Card, SectionLabel } from "../finance/Card";
import { Donut } from "../finance/Donut";
import { FinanceLayout } from "../finance/FinanceLayout";

type TxFilter = "all" | "income" | "expense" | "transfer";

export function FinanceTransactions() {
  const { data: accounts, refetch: rA } = useQuery<FinanceAccount[]>(
    "finance_accounts",
    undefined,
    [],
  );
  const { data: transactions, refetch: rT } = useQuery<FinanceTransaction[]>(
    "finance_transactions",
    undefined,
    [],
  );
  const { data: rates } = useQuery<Record<string, number>>("finance_exchange_rates", undefined, {});

  const refetchAll = () => {
    rA();
    rT();
  };
  useEvent<{ entityKind: string }>("entity:updated", refetchAll);

  const accountMap = useMemo(() => new Map(accounts.map((a) => [a.id, a])), [accounts]);
  const [filter, setFilter] = useState<TxFilter>("all");
  const [searchQ, setSearchQ] = useState("");
  const [acctFilter, setAcctFilter] = useState<string>("all");

  const filtered = useMemo(() => {
    let txs = transactions;
    if (filter !== "all") txs = txs.filter((t) => t.txType === filter);
    if (acctFilter !== "all") txs = txs.filter((t) => t.accountId === acctFilter);
    if (searchQ) {
      const q = searchQ.toLowerCase();
      txs = txs.filter(
        (t) =>
          t.counterparty?.toLowerCase().includes(q) ||
          t.notes?.toLowerCase().includes(q) ||
          t.category?.toLowerCase().includes(q),
      );
    }
    return txs;
  }, [transactions, filter, acctFilter, searchQ]);

  const totalIncome = useMemo(
    () =>
      filtered
        .filter((t) => t.txType === "income")
        .reduce((s, t) => s + toVnd(t.amount, t.currency, rates), 0),
    [filtered, rates],
  );
  const totalExpense = useMemo(
    () =>
      filtered
        .filter((t) => t.txType === "expense")
        .reduce((s, t) => s + toVnd(t.amount, t.currency, rates), 0),
    [filtered, rates],
  );

  const catSegs = useMemo(() => {
    const m = new Map<string, number>();
    filtered
      .filter((t) => t.txType === "expense")
      .forEach((t) => {
        const c = t.category ?? "Other";
        m.set(c, (m.get(c) ?? 0) + toVnd(t.amount, t.currency, rates));
      });
    return Array.from(m.entries())
      .sort((a, b) => b[1] - a[1])
      .map(([name, value], i) => ({ name, value, color: COLORS[i % COLORS.length] }));
  }, [filtered, rates]);

  const filters: { key: TxFilter; label: string }[] = [
    { key: "all", label: "All" },
    { key: "income", label: "Income" },
    { key: "expense", label: "Expense" },
    { key: "transfer", label: "Transfer" },
  ];

  return (
    <FinanceLayout onRefresh={refetchAll}>
      <div className="grid grid-cols-12 gap-3 auto-rows-min">
        {/* ── Stats row ─────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-4 gap-3">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Total Transactions
            </p>
            <p className="text-[20px] font-light text-primary">{filtered.length}</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Income</p>
            <p className="text-[20px] font-light text-success">{fmtCompact(totalIncome)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Expenses
            </p>
            <p className="text-[20px] font-light text-destructive">{fmtCompact(totalExpense)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Net</p>
            <p
              className={cn(
                "text-[20px] font-light",
                totalIncome - totalExpense >= 0 ? "text-success" : "text-destructive",
              )}
            >
              {totalIncome - totalExpense >= 0 ? "+" : ""}
              {fmtCompact(totalIncome - totalExpense)}đ
            </p>
          </Card>
        </div>

        {/* ── Filters ─────────────────────────────────── */}
        <div className="col-span-12 flex items-center gap-3">
          <div className="flex items-center gap-0.5">
            {filters.map((f) => (
              <button
                key={f.key}
                onClick={() => setFilter(f.key)}
                className={cn(
                  "px-2.5 py-1 rounded-md text-[11px] font-light transition-colors",
                  filter === f.key
                    ? "bg-surface-highest text-brand"
                    : "text-muted hover:text-secondary hover:bg-surface-base",
                )}
              >
                {f.label}
              </button>
            ))}
          </div>
          <select
            value={acctFilter}
            onChange={(e) => setAcctFilter(e.target.value)}
            className="bg-surface-low border border-border rounded-md px-2 py-1 text-[11px] font-light text-secondary"
          >
            <option value="all">All Accounts</option>
            {accounts
              .filter((a) => !a.isArchived)
              .map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
          </select>
          <div className="flex-1" />
          <div className="relative">
            <Search
              className="w-3 h-3 text-dim absolute left-2 top-1/2 -translate-y-1/2"
              strokeWidth={1.5}
            />
            <input
              type="text"
              value={searchQ}
              onChange={(e) => setSearchQ(e.target.value)}
              placeholder="Search..."
              className="bg-surface-low border border-border rounded-md pl-6 pr-2 py-1 text-[11px] font-light text-secondary placeholder:text-dim w-48"
            />
          </div>
          <button className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors">
            <Plus className="w-3 h-3" strokeWidth={1.5} /> Add
          </button>
        </div>

        {/* ── Transaction list (9col) + Category breakdown (3col) ── */}
        <div className="col-span-9">
          <SectionLabel>Transactions ({filtered.length})</SectionLabel>
          <Card className="overflow-hidden">
            <div className="grid grid-cols-[70px_24px_1fr_80px_100px_120px] gap-2 border-b border-border text-[10px] text-dim font-light px-4 py-2">
              <div>Date</div>
              <div></div>
              <div>Description</div>
              <div>Category</div>
              <div>Account</div>
              <div className="text-right">Amount</div>
            </div>
            {filtered.length === 0 ? (
              <div className="p-6 text-center text-[11px] text-dim font-light">
                No transactions match your filters
              </div>
            ) : (
              filtered.map((tx) => {
                const acct = accountMap.get(tx.accountId);
                const TxI =
                  tx.txType === "income"
                    ? ArrowDownRight
                    : tx.txType === "expense"
                      ? ArrowUpRight
                      : ArrowLeftRight;
                const col =
                  tx.txType === "income"
                    ? "text-success"
                    : tx.txType === "expense"
                      ? "text-destructive"
                      : "text-info";
                const pre = tx.txType === "income" ? "+" : tx.txType === "expense" ? "-" : "";
                return (
                  <div
                    key={tx.id}
                    className="grid grid-cols-[70px_24px_1fr_80px_100px_120px] gap-2 items-center px-4 py-2.5 hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0"
                  >
                    <span className="text-[10px] text-dim font-light">{tx.txDate}</span>
                    <TxI className={cn("w-3.5 h-3.5", col)} strokeWidth={1.5} />
                    <div className="min-w-0">
                      <p className="text-[12px] font-light text-secondary truncate">
                        {tx.counterparty ?? tx.notes ?? tx.txType}
                      </p>
                      {tx.subcategory && (
                        <p className="text-[9px] text-dim font-light">{tx.subcategory}</p>
                      )}
                    </div>
                    <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-surface-base text-dim truncate">
                      {tx.category ?? "—"}
                    </span>
                    <span className="text-[10px] text-dim font-light truncate">
                      {acct?.name ?? "—"}
                    </span>
                    <span className={cn("text-[12px] font-light text-right", col)}>
                      {pre}
                      {fmtMoney(tx.amount, tx.currency)}
                    </span>
                  </div>
                );
              })
            )}
          </Card>
        </div>

        <div className="col-span-3">
          <SectionLabel>By Category</SectionLabel>
          <Card className="p-4">
            {catSegs.length > 0 ? (
              <>
                <Donut
                  segments={catSegs}
                  label="Spending"
                  value={`${fmtCompact(totalExpense)}đ`}
                  size={140}
                />
                <div className="mt-3 pt-2.5 border-t border-border-subtle space-y-1.5">
                  {catSegs.map((seg, i) => (
                    <div key={i} className="flex justify-between items-center">
                      <div className="flex items-center gap-1.5">
                        <div
                          className="w-2 h-2 rounded-full"
                          style={{ backgroundColor: seg.color }}
                        />
                        <span className="text-[10px] text-muted font-light">{seg.name}</span>
                      </div>
                      <span className="text-[10px] text-secondary font-light">
                        {fmtCompact(seg.value)}đ
                      </span>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <p className="text-[11px] text-dim font-light text-center py-4">No expense data</p>
            )}
          </Card>
        </div>
      </div>
    </FinanceLayout>
  );
}
