import { fmtCompact } from "../lib/finance";
import { Card } from "./Card";

interface StatCardProps {
  label: string;
  value: string;
  accentColor: string;
}

function StatCard({ label, value, accentColor }: StatCardProps) {
  return (
    <Card compact className="relative overflow-hidden pl-4">
      <div
        className="absolute left-0 top-0 bottom-0 w-[3px] rounded-l-xl"
        style={{ backgroundColor: accentColor }}
      />
      <p className="text-[8px] font-medium text-dim uppercase tracking-wider mb-1">{label}</p>
      <p className="text-[22px] font-light text-primary tabular-nums leading-none">{value}</p>
    </Card>
  );
}

export function CashFlowStats({
  income,
  spending,
  displayCur,
  convertTotal,
  hidden,
}: {
  income: number;
  spending: number;
  displayCur: string;
  convertTotal: (v: number) => number;
  hidden: boolean;
}) {
  const net = income - spending;
  const savingsRate = income > 0 ? Math.round((net / income) * 100) : 0;

  return (
    <div className="grid grid-cols-4 gap-3">
      <StatCard
        label="Income"
        value={fmtCompact(convertTotal(income), displayCur, hidden)}
        accentColor="var(--success)"
      />
      <StatCard
        label="Spending"
        value={fmtCompact(convertTotal(spending), displayCur, hidden)}
        accentColor="var(--destructive)"
      />
      <StatCard
        label="Net"
        value={fmtCompact(convertTotal(net), displayCur, hidden)}
        accentColor="var(--info)"
      />
      <StatCard label="Savings Rate" value={`${savingsRate}%`} accentColor="var(--brand)" />
    </div>
  );
}
