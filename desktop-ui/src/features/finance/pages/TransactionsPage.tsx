import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { todayISO } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type {
  FinanceAccount,
  FinanceTransaction,
  FinanceTransactionCreateParams,
} from "@shared/types";
import { ArrowDownRight, ArrowLeftRight, ArrowUpRight, Plus, Search } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Card, CardHeader } from "../components/Card";
import { Donut } from "../components/Donut";
import { FinanceLayout } from "../components/FinanceLayout";
import { FinanceSkeleton } from "../components/FinanceSkeleton";
import { FormField, fieldClass } from "../components/FormModal";
import { SlidePanel } from "../components/SlidePanel";
import { COLORS, fmtCompact, fmtMoney, toBase } from "../lib/finance";

type TxFilter = "all" | "income" | "expense" | "transfer";

export function FinanceTransactions() {
  const { data: accounts, refetch: rA } = useQuery<FinanceAccount[]>(
    "finance_accounts",
    undefined,
    [],
  );
  const { data: rates } = useQuery<Record<string, number>>("finance_exchange_rates", undefined, {});
  const { data: settings } = useQuery<{ defaultCurrency: string }>(
    "finance_settings",
    undefined,
    {},
  );
  const baseCurrency = settings?.defaultCurrency ?? "USD";

  // ── Server-side filter state ──
  const [filter, setFilter] = useState<TxFilter>("all");
  const [acctFilter, setAcctFilter] = useState<string | undefined>(undefined);
  const [searchQ, setSearchQ] = useState("");
  const [debouncedQ, setDebouncedQ] = useState("");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const handleSearch = (value: string) => {
    setSearchQ(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => setDebouncedQ(value), 300);
  };

  useEffect(
    () => () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    },
    [],
  );

  const filterParams = useMemo(
    () => ({
      params: {
        txType: filter === "all" ? undefined : filter,
        accountId: acctFilter,
        query: debouncedQ || undefined,
        limit: 100,
      },
    }),
    [filter, acctFilter, debouncedQ],
  );

  const {
    data: transactions,
    loading,
    error,
    refetch: rT,
  } = useQuery<FinanceTransaction[]>("finance_transactions_filtered", filterParams, []);

  const refetchAll = useCallback(() => {
    rA();
    rT();
  }, [rA, rT]);
  useEvent<{ entityKind: string }>("entity:updated", refetchAll);

  const accountMap = useMemo(() => new Map(accounts.map((a) => [a.id, a])), [accounts]);

  const { totalIncome, totalExpense, catSegs } = useMemo(() => {
    let income = 0;
    let expense = 0;
    const catMap = new Map<string, number>();

    for (const t of transactions) {
      const vnd = toBase(t.amount, t.currency, rates, baseCurrency);
      if (t.txType === "income") {
        income += vnd;
      } else if (t.txType === "expense") {
        expense += vnd;
        const c = t.category ?? "Other";
        catMap.set(c, (catMap.get(c) ?? 0) + vnd);
      }
    }

    const segments = Array.from(catMap.entries())
      .sort((a, b) => b[1] - a[1])
      .map(([name, value], i) => ({ name, value, color: COLORS[i % COLORS.length] }));

    return { totalIncome: income, totalExpense: expense, catSegs: segments };
  }, [transactions, rates]);

  // ── Add Transaction slide panel ──
  const [panelOpen, setPanelOpen] = useState(false);
  const [txAccountId, setTxAccountId] = useState("");
  const [txType, setTxType] = useState<"income" | "expense" | "transfer">("expense");
  const [txAmount, setTxAmount] = useState("");
  const [txCategory, setTxCategory] = useState("");
  const [txCounterparty, setTxCounterparty] = useState("");
  const [txDate, setTxDate] = useState(todayISO);
  const [txNotes, setTxNotes] = useState("");

  const { mutate: createTx } = useMutation<FinanceTransaction, FinanceTransactionCreateParams>(
    "finance_transaction_create",
    "params",
  );

  const handleCreateTx = async () => {
    if (!txAccountId || !txAmount) return;
    const result = await createTx({
      accountId: txAccountId,
      txType,
      amount: Math.round(Number(txAmount) * 100),
      category: txCategory || undefined,
      counterparty: txCounterparty || undefined,
      txDate: txDate || undefined,
      notes: txNotes || undefined,
    });
    if (!result) return;
    setPanelOpen(false);
    setTxAmount("");
    setTxCategory("");
    setTxCounterparty("");
    setTxNotes("");
    refetchAll();
  };

  const filters: { key: TxFilter; label: string }[] = [
    { key: "all", label: "All" },
    { key: "income", label: "Income" },
    { key: "expense", label: "Expense" },
    { key: "transfer", label: "Transfer" },
  ];

  const activeAccounts = useMemo(() => accounts.filter((a) => !a.isArchived), [accounts]);

  if (loading && transactions.length === 0) {
    return (
      <FinanceLayout onRefresh={refetchAll}>
        <FinanceSkeleton rows={5} />
      </FinanceLayout>
    );
  }

  if (error && transactions.length === 0) {
    return (
      <FinanceLayout onRefresh={refetchAll}>
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
    <FinanceLayout onRefresh={refetchAll}>
      <div className="grid grid-cols-12 gap-4 auto-rows-min">
        {/* ── Stats row ─────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-4 gap-4">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Transactions
            </p>
            <p className="text-[20px] font-light text-primary">{transactions.length}</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">Income</p>
            <p className="text-[20px] font-light text-success tabular-nums">
              {fmtCompact(totalIncome, baseCurrency)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Expenses
            </p>
            <p className="text-[20px] font-light text-destructive tabular-nums">
              {fmtCompact(totalExpense, baseCurrency)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">Net</p>
            <p
              className={cn(
                "text-[20px] font-light tabular-nums",
                totalIncome - totalExpense >= 0 ? "text-success" : "text-destructive",
              )}
            >
              {totalIncome - totalExpense >= 0 ? "+" : ""}
              {fmtCompact(totalIncome - totalExpense, baseCurrency)}
            </p>
          </Card>
        </div>

        {/* ── Filters ─────────────────────────────────── */}
        <div className="col-span-12 flex items-center gap-3">
          <div className="flex items-center gap-0.5">
            {filters.map((f) => (
              <button
                type="button"
                key={f.key}
                onClick={() => setFilter(f.key)}
                className={cn(
                  "px-2.5 py-1 rounded-md text-[11px] font-light transition-colors",
                  filter === f.key
                    ? "bg-white/[0.12] text-brand"
                    : "text-muted hover:text-secondary hover:bg-white/[0.06]",
                )}
              >
                {f.label}
              </button>
            ))}
          </div>
          <select
            value={acctFilter ?? "all"}
            onChange={(e) => setAcctFilter(e.target.value === "all" ? undefined : e.target.value)}
            className="glass-input border border-white/[0.08] rounded-md px-2 py-1 text-[11px] font-light text-secondary"
          >
            <option value="all">All Accounts</option>
            {activeAccounts.map((a) => (
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
              onChange={(e) => handleSearch(e.target.value)}
              placeholder="Search…"
              className="glass-input border border-white/[0.08] rounded-md pl-6 pr-2 py-1 text-[11px] font-light text-secondary placeholder:text-dim w-48"
            />
          </div>
          <button
            type="button"
            onClick={() => {
              if (activeAccounts.length > 0 && !txAccountId) setTxAccountId(activeAccounts[0].id);
              setPanelOpen(true);
            }}
            className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors"
          >
            <Plus className="w-3 h-3" strokeWidth={1.5} /> Add
          </button>
        </div>

        {/* ── Transaction list (9col) + Category breakdown (3col) ── */}
        <div className="col-span-9">
          <Card className="overflow-hidden">
            <div className="px-4 pt-4">
              <CardHeader title={`Transactions (${transactions.length})`} />
            </div>
            <div className="grid grid-cols-[70px_24px_1fr_80px_100px_120px] gap-2 border-b border-white/[0.08] text-[10px] text-dim font-light px-4 py-2">
              <div>Date</div>
              <div />
              <div>Description</div>
              <div>Category</div>
              <div>Account</div>
              <div className="text-right">Amount</div>
            </div>
            {transactions.length === 0 ? (
              <div className="p-6 text-center text-[11px] text-dim font-light">
                No transactions match your filters
              </div>
            ) : (
              transactions.map((tx) => {
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
                    className="grid grid-cols-[70px_24px_1fr_80px_100px_120px] gap-2 items-center px-4 py-2.5 hover:bg-white/[0.06] transition-colors border-b border-white/[0.04] last:border-b-0"
                  >
                    <span className="text-[10px] text-dim font-light tabular-nums">
                      {tx.txDate}
                    </span>
                    <TxI className={cn("w-3.5 h-3.5", col)} strokeWidth={1.5} />
                    <div className="min-w-0">
                      <p className="text-[12px] font-light text-secondary truncate">
                        {tx.counterparty ?? tx.notes ?? tx.txType}
                      </p>
                      {tx.subcategory && (
                        <p className="text-[9px] text-dim font-light">{tx.subcategory}</p>
                      )}
                    </div>
                    <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-white/[0.06] text-dim truncate">
                      {tx.category ?? "—"}
                    </span>
                    <span className="text-[10px] text-dim font-light truncate">
                      {acct?.name ?? "—"}
                    </span>
                    <span className={cn("text-[12px] font-light text-right tabular-nums", col)}>
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
          <Card className="p-4">
            <CardHeader title="By Category" />
            {catSegs.length > 0 ? (
              <>
                <Donut
                  segments={catSegs}
                  label="Spending"
                  value={fmtCompact(totalExpense, baseCurrency)}
                  size={140}
                />
                <div className="mt-3 pt-2.5 border-t border-white/[0.04] space-y-1.5">
                  {catSegs.map((seg) => (
                    <div key={seg.name} className="flex justify-between items-center">
                      <div className="flex items-center gap-1.5">
                        <div
                          className="w-2 h-2 rounded-full"
                          style={{ backgroundColor: seg.color }}
                        />
                        <span className="text-[10px] text-muted font-light">{seg.name}</span>
                      </div>
                      <span className="text-[10px] text-secondary font-light">
                        {fmtCompact(seg.value, baseCurrency)}
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

      {/* ── Add Transaction SlidePanel ──────────────── */}
      <SlidePanel open={panelOpen} onClose={() => setPanelOpen(false)} title="Add Transaction">
        <div className="space-y-3">
          <FormField label="Account">
            <select
              className={fieldClass}
              value={txAccountId}
              onChange={(e) => setTxAccountId(e.target.value)}
            >
              <option value="" disabled>
                Select account
              </option>
              {activeAccounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </FormField>
          <FormField label="Type">
            <div className="flex gap-2">
              {(["expense", "income", "transfer"] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => setTxType(t)}
                  className={cn(
                    "flex-1 py-1.5 text-[12px] rounded-md border transition-colors capitalize",
                    txType === t
                      ? "border-brand/50 text-brand bg-brand/5"
                      : "border-white/[0.08] text-muted bg-white/[0.06] hover:bg-white/[0.08]",
                  )}
                >
                  {t}
                </button>
              ))}
            </div>
          </FormField>
          <FormField label="Amount">
            <input
              className={fieldClass}
              type="number"
              value={txAmount}
              onChange={(e) => setTxAmount(e.target.value)}
              placeholder="0"
              autoFocus
            />
          </FormField>
          <FormField label="Category">
            <input
              className={fieldClass}
              value={txCategory}
              onChange={(e) => setTxCategory(e.target.value)}
              placeholder="e.g. Food, Transport"
            />
          </FormField>
          <FormField label="Counterparty">
            <input
              className={fieldClass}
              value={txCounterparty}
              onChange={(e) => setTxCounterparty(e.target.value)}
              placeholder="e.g. Grab, Shopee"
            />
          </FormField>
          <FormField label="Date">
            <input
              className={fieldClass}
              type="date"
              value={txDate}
              onChange={(e) => setTxDate(e.target.value)}
            />
          </FormField>
          <FormField label="Notes">
            <input
              className={fieldClass}
              value={txNotes}
              onChange={(e) => setTxNotes(e.target.value)}
              placeholder="Optional notes"
            />
          </FormField>
          <div className="pt-2">
            <button
              type="button"
              onClick={handleCreateTx}
              disabled={!txAccountId || !txAmount}
              className="w-full py-2 text-[12px] rounded-lg bg-brand text-white hover:bg-brand-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Add Transaction
            </button>
          </div>
        </div>
      </SlidePanel>
    </FinanceLayout>
  );
}
