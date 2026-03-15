import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { cn } from "@shared/lib/utils";
import type { FinanceAccount, FinanceAccountCreateParams, FinanceTransaction } from "@shared/types";
import { ArrowDownRight, ArrowLeftRight, ArrowUpRight, Plus, Wallet } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router";
import { Card, CardHeader } from "../components/Card";
import { FinanceLayout } from "../components/FinanceLayout";
import { FinanceSkeleton } from "../components/FinanceSkeleton";
import { FormField, FormModal, fieldClass } from "../components/FormModal";
import { useFinanceCurrency } from "../hooks/useFinanceCurrency";
import { usePrivacyMode } from "../hooks/usePrivacyMode";
import { displayAmount, displayHint } from "../lib/displayAmount";
import { ACCT_ICONS, fmtCompact, fmtMoney } from "../lib/finance";

export function FinanceAccounts() {
  const { mode, setMode, baseCurrency, rates, currencies, displayCur, convertTotal } =
    useFinanceCurrency();
  const { hidden, toggle } = usePrivacyMode();
  const [searchParams] = useSearchParams();
  const {
    data: accounts,
    loading,
    error,
    refetch: rA,
  } = useQuery<FinanceAccount[]>("finance_accounts", undefined, []);
  const { data: transactions, refetch: rT } = useQuery<FinanceTransaction[]>(
    "finance_transactions",
    undefined,
    [],
  );

  const refetchAll = () => {
    rA();
    rT();
  };
  useEvent<{ entityKind: string }>("entity:updated", refetchAll);

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [modalOpen, setModalOpen] = useState(false);

  // Read ?id= from URL for click-through from Dashboard
  useEffect(() => {
    const id = searchParams.get("id");
    if (id) setSelectedId(id);
  }, [searchParams]);

  const active = useMemo(() => accounts.filter((a) => !a.isArchived), [accounts]);
  const archived = useMemo(() => accounts.filter((a) => a.isArchived), [accounts]);
  const selected = useMemo(() => accounts.find((a) => a.id === selectedId), [accounts, selectedId]);
  const acctTxs = useMemo(
    () => (selectedId ? transactions.filter((t) => t.accountId === selectedId) : []),
    [transactions, selectedId],
  );
  const totalBalance = useMemo(
    () => active.reduce((s, a) => s + (a.baseBalance ?? 0), 0),
    [active],
  );

  // ── Add Account form state ──
  const [name, setName] = useState("");
  const [accountType, setAccountType] = useState("bank");
  const [currency, setCurrency] = useState("VND");
  const [balance, setBalance] = useState("");
  const [institution, setInstitution] = useState("");
  const { mutate: createAccount } = useMutation<FinanceAccount, FinanceAccountCreateParams>(
    "finance_account_create",
    "params",
  );

  const handleCreate = async () => {
    const result = await createAccount({
      name,
      accountType,
      currency,
      balance: balance ? Math.round(Number(balance) * 100) : undefined,
      institution: institution || undefined,
    });
    if (!result) return;
    setModalOpen(false);
    setName("");
    setBalance("");
    setInstitution("");
    refetchAll();
  };

  if (loading && accounts.length === 0) {
    return (
      <FinanceLayout
        hidden={hidden}
        onTogglePrivacy={toggle}
        currencyMode={mode}
        currencies={currencies}
        onSelectCurrency={setMode}
      >
        <FinanceSkeleton rows={3} />
      </FinanceLayout>
    );
  }

  if (error && accounts.length === 0) {
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
        {/* ── Header stats ────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-3 gap-4">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Balance
            </p>
            <p className="text-[24px] font-light text-primary">
              {fmtCompact(convertTotal(totalBalance), displayCur, hidden)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Active Accounts
            </p>
            <p className="text-[24px] font-light text-primary">{active.length}</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Currencies
            </p>
            <p className="text-[24px] font-light text-primary">
              {new Set(active.map((a) => a.currency)).size}
            </p>
          </Card>
        </div>

        {/* ── Account grid (left 8col) + Detail (right 4col) ── */}
        <div className="col-span-8">
          <Card className="p-4">
            <CardHeader
              title="Accounts"
              action={
                <button
                  type="button"
                  onClick={() => setModalOpen(true)}
                  className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors"
                >
                  <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Account
                </button>
              }
            />
            <div className="grid grid-cols-2 gap-4">
              {active.map((acct) => {
                const Icon = ACCT_ICONS[acct.accountType] ?? Wallet;
                const isSelected = acct.id === selectedId;
                return (
                  <div
                    key={acct.id}
                    className={cn(
                      "glass-card p-4 cursor-pointer transition-colors",
                      isSelected ? "ring-1 ring-brand bg-white/[0.06]" : "hover:bg-white/[0.06]",
                    )}
                    onClick={() => setSelectedId(isSelected ? null : acct.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") setSelectedId(isSelected ? null : acct.id);
                    }}
                    role="button"
                    tabIndex={0}
                  >
                    <div className="flex items-center gap-3 mb-3">
                      <div className="w-9 h-9 rounded-lg bg-white/[0.08] flex items-center justify-center flex-shrink-0">
                        <Icon className="w-4 h-4 text-muted" strokeWidth={1.5} />
                      </div>
                      <div className="min-w-0 flex-1">
                        <p className="text-[12px] font-medium text-secondary truncate">
                          {acct.name}
                        </p>
                        <p className="text-[9px] text-dim font-light">
                          {acct.institution ?? acct.accountType.replaceAll("_", " ")}
                        </p>
                      </div>
                    </div>
                    <p className="text-[18px] font-light text-primary tabular-nums">
                      {displayAmount({
                        amount: acct.balance,
                        currency: acct.currency,
                        baseAmount: acct.baseBalance,
                        baseCurrency,
                        mode,
                        rates,
                        hidden,
                      })}
                    </p>
                    {(() => {
                      const hint = displayHint({
                        amount: acct.balance,
                        currency: acct.currency,
                        baseAmount: acct.baseBalance,
                        baseCurrency,
                        mode,
                        rates,
                      });
                      return hint ? (
                        <p className="text-[10px] text-dim font-light mt-0.5">≈ {hint}</p>
                      ) : null;
                    })()}
                    {acct.notes && (
                      <p className="text-[9px] text-dim font-light mt-2">{acct.notes}</p>
                    )}
                  </div>
                );
              })}
            </div>
          </Card>

          {archived.length > 0 && (
            <Card className="p-4 mt-4">
              <CardHeader title="Archived" />
              <div className="grid grid-cols-2 gap-4">
                {archived.map((acct) => {
                  const Icon = ACCT_ICONS[acct.accountType] ?? Wallet;
                  return (
                    <div key={acct.id} className="glass-card p-4 opacity-50">
                      <div className="flex items-center gap-3">
                        <Icon className="w-4 h-4 text-dim" strokeWidth={1.5} />
                        <div className="min-w-0 flex-1">
                          <p className="text-[12px] font-light text-muted truncate">{acct.name}</p>
                          <p className="text-[10px] text-dim font-light">
                            {displayAmount({
                              amount: acct.balance,
                              currency: acct.currency,
                              baseAmount: acct.baseBalance,
                              baseCurrency,
                              mode,
                              rates,
                            })}
                          </p>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </Card>
          )}
        </div>

        {/* ── Right sidebar: selected account transactions ── */}
        <div className="col-span-4">
          <Card className="overflow-hidden">
            <div className="px-4 pt-4">
              <CardHeader
                title={selected ? `${selected.name} Transactions` : "Select an account"}
              />
            </div>
            {selected ? (
              acctTxs.length === 0 ? (
                <div className="p-6 text-center text-[11px] text-dim font-light">
                  No transactions
                </div>
              ) : (
                acctTxs.map((tx) => {
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
                      className="flex items-center gap-2 px-3 py-2 hover:bg-white/[0.06] transition-colors border-b border-white/[0.04] last:border-b-0"
                    >
                      <TxI className={cn("w-3 h-3 flex-shrink-0", col)} strokeWidth={1.5} />
                      <div className="min-w-0 flex-1">
                        <p className="text-[11px] font-light text-secondary truncate">
                          {tx.counterparty ?? tx.notes ?? tx.txType}
                        </p>
                        <p className="text-[9px] text-dim font-light tabular-nums">{tx.txDate}</p>
                      </div>
                      <span
                        className={cn("text-[11px] font-light flex-shrink-0 tabular-nums", col)}
                      >
                        {pre}
                        {displayAmount({
                          amount: tx.amount,
                          currency: tx.currency,
                          baseAmount: tx.baseAmount,
                          baseCurrency,
                          mode,
                          rates,
                          hidden,
                        })}
                      </span>
                    </div>
                  );
                })
              )
            ) : (
              <div className="p-8 flex items-center justify-center">
                <p className="text-[11px] text-dim font-light">
                  Click an account to view its transactions
                </p>
              </div>
            )}
          </Card>
        </div>
      </div>

      {/* ── Add Account Modal ──────────────────────────── */}
      <FormModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title="Add Account"
        onSubmit={handleCreate}
        canSubmit={name.trim().length > 0}
      >
        <FormField label="Account Name">
          <input
            className={fieldClass}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Vietcombank Checking"
            autoFocus
          />
        </FormField>
        <FormField label="Account Type">
          <select
            className={fieldClass}
            value={accountType}
            onChange={(e) => setAccountType(e.target.value)}
          >
            <option value="bank">Bank</option>
            <option value="ewallet">E-Wallet</option>
            <option value="crypto_wallet">Crypto Wallet</option>
            <option value="cash">Cash</option>
            <option value="brokerage">Brokerage</option>
          </select>
        </FormField>
        <FormField label="Currency">
          <select
            className={fieldClass}
            value={currency}
            onChange={(e) => setCurrency(e.target.value)}
          >
            <option value="VND">VND</option>
            <option value="USD">USD</option>
            <option value="USDT">USDT</option>
          </select>
        </FormField>
        <FormField label="Initial Balance">
          <input
            className={fieldClass}
            type="number"
            value={balance}
            onChange={(e) => setBalance(e.target.value)}
            placeholder="0"
          />
        </FormField>
        <FormField label="Institution (optional)">
          <input
            className={fieldClass}
            value={institution}
            onChange={(e) => setInstitution(e.target.value)}
            placeholder="e.g. Vietcombank"
          />
        </FormField>
      </FormModal>
    </FinanceLayout>
  );
}
