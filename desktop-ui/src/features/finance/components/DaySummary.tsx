import { fmtCompact } from "../lib/finance";

function formatDateLabel(date: string): string {
  const [y, m, d] = date.split("-").map(Number);
  const dateObj = new Date(y, m - 1, d);
  return new Intl.DateTimeFormat("en-US", {
    month: "long",
    day: "numeric",
  }).format(dateObj);
}

export function DaySummary({
  date,
  txCount,
  totalSpending,
  categories,
  onClose,
  displayCur,
  convertTotal,
  hidden,
}: {
  date: string;
  txCount: number;
  totalSpending: number;
  categories: string[];
  onClose: () => void;
  displayCur: string;
  convertTotal: (v: number) => number;
  hidden: boolean;
}) {
  const dateLabel = formatDateLabel(date);
  const spendingDisplay = fmtCompact(convertTotal(totalSpending), displayCur, hidden);

  return (
    <div
      role="status"
      aria-live="polite"
      className="glass-card flex items-start justify-between gap-3"
    >
      <div className="flex-1 min-w-0">
        <p className="text-[13px] font-medium text-primary mb-1.5">
          {dateLabel} &mdash;{" "}
          <span className="font-light text-secondary">
            {txCount} {txCount === 1 ? "transaction" : "transactions"}, spent{" "}
            {spendingDisplay}
          </span>
        </p>

        {categories.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {categories.map((cat) => (
              <span
                key={cat}
                className="px-2 py-0.5 rounded-md bg-white/[0.06] border border-white/[0.08] text-[10px] font-light text-dim capitalize"
              >
                {cat}
              </span>
            ))}
          </div>
        )}
      </div>

      <button
        type="button"
        onClick={onClose}
        aria-label="Close day summary"
        className="p-1 rounded-lg text-dim hover:text-secondary hover:bg-white/[0.06] transition-colors shrink-0 text-[14px] leading-none"
      >
        &times;
      </button>
    </div>
  );
}
