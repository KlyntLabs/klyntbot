import { useState } from "react";
import { COLORS, fmtCompact, pct } from "../lib/finance";

const MAX_VISIBLE = 6;

export function CategoryRanking({
  breakdown,
  displayCur,
  convertTotal,
  hidden,
}: {
  breakdown: { category: string; total: number }[];
  displayCur: string;
  convertTotal: (v: number) => number;
  hidden: boolean;
}) {
  const [showAll, setShowAll] = useState(false);

  const totalSpending = breakdown.reduce((sum, row) => sum + row.total, 0);
  const visible =
    showAll || breakdown.length <= MAX_VISIBLE ? breakdown : breakdown.slice(0, MAX_VISIBLE);

  if (breakdown.length === 0) {
    return (
      <p className="text-[12px] text-dim font-light text-center py-4">
        No spending data for this period.
      </p>
    );
  }

  return (
    <div className="space-y-2">
      {visible.map((row, i) => {
        const colorHex = COLORS[i % COLORS.length];
        const percentage = pct(row.total, totalSpending);
        return (
          <div key={row.category} className="flex items-center gap-3">
            {/* Color dot */}
            <div className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: colorHex }} />
            {/* Category name */}
            <span className="flex-1 text-[12px] font-light text-muted-foreground truncate capitalize">
              {row.category}
            </span>
            {/* Amount */}
            <span className="text-[12px] font-light text-foreground tabular-nums">
              {fmtCompact(convertTotal(row.total), displayCur, hidden)}
            </span>
            {/* Percentage */}
            <span className="text-[11px] font-light text-dim tabular-nums w-8 text-right">
              {percentage}%
            </span>
          </div>
        );
      })}

      {breakdown.length > MAX_VISIBLE && (
        <button
          type="button"
          onClick={() => setShowAll((v) => !v)}
          className="text-[11px] font-light text-muted-foreground hover:text-foreground transition-colors mt-1"
        >
          {showAll ? "Show less" : `Show ${breakdown.length - MAX_VISIBLE} more`}
        </button>
      )}
    </div>
  );
}
