import { fmtCompact } from "../lib/finance";
import { Card } from "./Card";

export function NetWorthCard({
  totalNet,
  totalAssets,
  totalInvest,
  totalDebt,
  displayCur,
  convertTotal,
  hidden,
}: {
  totalNet: number;
  totalAssets: number;
  totalInvest: number;
  totalDebt: number;
  displayCur: string;
  convertTotal: (v: number) => number;
  hidden: boolean;
}) {
  const cashAmount = totalAssets - totalInvest;
  const total = totalAssets + totalDebt;

  return (
    <Card className="p-5 flex items-center justify-between">
      <div>
        <p className="text-[10px] text-muted-foreground uppercase tracking-widest mb-1">
          Net Worth
        </p>
        <p className="text-[32px] font-light text-foreground tracking-tight leading-none tabular-nums">
          {fmtCompact(convertTotal(totalNet), displayCur, hidden)}
        </p>
      </div>
      <div className="text-right">
        <div className="flex h-2 rounded-full overflow-hidden gap-0.5 w-48 mb-2 ml-auto">
          {total > 0 && (
            <>
              <div className="bg-success rounded-full" style={{ flex: cashAmount }} />
              <div className="bg-info rounded-full" style={{ flex: totalInvest }} />
              <div className="bg-destructive rounded-full" style={{ flex: totalDebt }} />
            </>
          )}
        </div>
        <div className="flex gap-3 justify-end">
          <span className="text-[10px] text-muted-foreground flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-success" />
            Cash {fmtCompact(convertTotal(cashAmount), displayCur, hidden)}
          </span>
          <span className="text-[10px] text-muted-foreground flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-info" />
            Invest {fmtCompact(convertTotal(totalInvest), displayCur, hidden)}
          </span>
          <span className="text-[10px] text-muted-foreground flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-destructive" />
            Debt {fmtCompact(convertTotal(totalDebt), displayCur, hidden)}
          </span>
        </div>
      </div>
    </Card>
  );
}
