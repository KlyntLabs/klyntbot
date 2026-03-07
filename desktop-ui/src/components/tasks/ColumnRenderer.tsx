import { formatDate } from "../../lib/dates";
import type { ColumnType } from "../../lib/types";

interface ColumnRendererProps {
  columnType: ColumnType;
  value: unknown;
  options?: string[] | null;
}

export function ColumnRenderer({ columnType, value, options: _options }: ColumnRendererProps) {
  if (value === null || value === undefined || value === "") {
    return <span className="text-[11px] text-dim font-light">&mdash;</span>;
  }

  switch (columnType) {
    case "text":
      return <span className="text-[12px] font-light text-secondary">{String(value)}</span>;

    case "number":
      return (
        <span className="text-[12px] font-light text-secondary tabular-nums text-right block">
          {typeof value === "number" ? value.toLocaleString() : String(value)}
        </span>
      );

    case "date":
      return (
        <span className="text-[12px] font-light text-secondary">{formatDate(String(value))}</span>
      );

    case "dropdown":
      return (
        <span className="inline-flex px-2 py-0.5 rounded-full text-[11px] font-light bg-white/[0.06] text-muted border border-white/[0.06]">
          {String(value)}
        </span>
      );

    case "multi_select":
      return <MultiSelectDisplay value={value} />;

    case "checkbox":
      return (
        <span className="text-[12px]">
          {value ? (
            <span className="text-success">&#10003;</span>
          ) : (
            <span className="text-dim">&#10007;</span>
          )}
        </span>
      );

    case "tags":
      return <TagsDisplay value={value} />;

    case "link":
      return <LinkDisplay value={value} />;

    case "rating":
      return <RatingDisplay value={value} />;

    case "progress":
      return <ProgressDisplay value={value} />;

    case "duration":
      return <DurationDisplay value={value} />;

    case "currency":
      return <CurrencyDisplay value={value} />;

    default:
      return <span className="text-[11px] text-dim font-light">{String(value)}</span>;
  }
}

function MultiSelectDisplay({ value }: { value: unknown }) {
  const items = Array.isArray(value) ? value : [];
  if (items.length === 0) {
    return <span className="text-[11px] text-dim font-light">&mdash;</span>;
  }
  return (
    <div className="flex flex-wrap gap-1">
      {items.map((item) => (
        <span
          key={String(item)}
          className="inline-flex px-1.5 py-0.5 rounded-full text-[10px] font-light bg-white/[0.06] text-muted border border-white/[0.06]"
        >
          {String(item)}
        </span>
      ))}
    </div>
  );
}

function TagsDisplay({ value }: { value: unknown }) {
  const items = Array.isArray(value) ? value : [];
  if (items.length === 0) {
    return <span className="text-[11px] text-dim font-light">&mdash;</span>;
  }
  return (
    <div className="flex flex-wrap gap-1">
      {items.map((tag) => (
        <span
          key={String(tag)}
          className="inline-flex px-1.5 py-0.5 rounded-full text-[10px] font-light bg-white/[0.04] text-muted border border-white/[0.06]"
        >
          {String(tag)}
        </span>
      ))}
    </div>
  );
}

function LinkDisplay({ value }: { value: unknown }) {
  if (typeof value === "object" && value !== null && "url" in value) {
    const v = value as { url: string; label?: string };
    return (
      <a
        href={v.url}
        target="_blank"
        rel="noopener noreferrer"
        className="text-[11px] font-light text-brand hover:underline truncate block max-w-[160px]"
        onClick={(e) => e.stopPropagation()}
      >
        {v.label || v.url}
      </a>
    );
  }
  if (typeof value === "string") {
    return (
      <a
        href={value}
        target="_blank"
        rel="noopener noreferrer"
        className="text-[11px] font-light text-brand hover:underline truncate block max-w-[160px]"
        onClick={(e) => e.stopPropagation()}
      >
        {value}
      </a>
    );
  }
  return <span className="text-[11px] text-dim font-light">&mdash;</span>;
}

function RatingDisplay({ value }: { value: unknown }) {
  const rating = typeof value === "number" ? Math.min(5, Math.max(0, Math.round(value))) : 0;
  return (
    <span className="text-[12px] tracking-tight text-amber-400">
      {"★".repeat(rating)}
      <span className="text-white/[0.15]">{"★".repeat(5 - rating)}</span>
    </span>
  );
}

function ProgressDisplay({ value }: { value: unknown }) {
  const pct = typeof value === "number" ? Math.min(100, Math.max(0, value)) : 0;
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-1.5 rounded-full bg-white/[0.06] min-w-[40px] max-w-[80px]">
        <div className="h-full rounded-full bg-brand transition-all" style={{ width: `${pct}%` }} />
      </div>
      <span className="text-[10px] font-light text-dim tabular-nums">{pct}%</span>
    </div>
  );
}

function DurationDisplay({ value }: { value: unknown }) {
  if (typeof value === "number") {
    const totalMinutes = value;
    const h = Math.floor(totalMinutes / 60);
    const m = totalMinutes % 60;
    const parts: string[] = [];
    if (h > 0) parts.push(`${h}h`);
    if (m > 0 || h === 0) parts.push(`${m}m`);
    return (
      <span className="text-[12px] font-light text-secondary tabular-nums">{parts.join(" ")}</span>
    );
  }
  return <span className="text-[12px] font-light text-secondary">{String(value)}</span>;
}

function CurrencyDisplay({ value }: { value: unknown }) {
  if (typeof value === "object" && value !== null && "amount" in value) {
    const v = value as { amount: number; currency?: string };
    const symbol = CURRENCY_SYMBOLS[v.currency ?? "USD"] ?? v.currency ?? "$";
    return (
      <span className="text-[12px] font-light text-secondary tabular-nums">
        {symbol}
        {v.amount.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
      </span>
    );
  }
  if (typeof value === "number") {
    return (
      <span className="text-[12px] font-light text-secondary tabular-nums">
        ${value.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
      </span>
    );
  }
  return <span className="text-[12px] font-light text-secondary">{String(value)}</span>;
}

const CURRENCY_SYMBOLS: Record<string, string> = {
  USD: "$",
  EUR: "\u20AC",
  GBP: "\u00A3",
  JPY: "\u00A5",
  VND: "\u20AB",
  CNY: "\u00A5",
  KRW: "\u20A9",
};
