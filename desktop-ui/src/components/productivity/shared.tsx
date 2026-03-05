/**
 * Shared constants and components for productivity widgets.
 */

/** Canonical category-to-color map (lowercase keys). */
const CATEGORY_COLORS: Record<string, string> = {
  coding: "var(--success)",
  design: "var(--purple)",
  communication: "var(--info)",
  entertainment: "var(--destructive)",
  project_management: "var(--purple)",
  documentation: "var(--info)",
  email: "var(--text-muted)",
  browsing: "var(--brand)",
  reference: "var(--info)",
};

const FALLBACK_COLORS = [
  "var(--brand)",
  "var(--purple)",
  "var(--info)",
  "var(--success)",
  "var(--text-muted)",
  "var(--destructive)",
];

/**
 * Resolve a category color from either an ID ("coding") or display name ("Coding").
 * Falls back to a rotating palette, then to brand.
 */
export function getCategoryColor(nameOrId: string, index = 0): string {
  const key = nameOrId.toLowerCase().replace(/ /g, "_");
  return CATEGORY_COLORS[key] ?? FALLBACK_COLORS[index % FALLBACK_COLORS.length];
}

/** Score color thresholds — shared between ScoreRing and stats widgets. */
export function scoreColor(score: number): string {
  if (score >= 80) return "var(--success)";
  if (score >= 60) return "var(--brand)";
  if (score >= 40) return "var(--text-muted)";
  return "var(--destructive)";
}

/** Build the standard Focus/Active/Breaks donut segments. */
export function buildBreakdownSegments(
  totalActive: number,
  totalFocus: number,
  totalBreak: number,
): { name: string; value: number; color: string }[] {
  return [
    { name: "Focus", value: totalFocus, color: "var(--brand)" },
    { name: "Active", value: totalActive - totalFocus - totalBreak, color: "var(--purple)" },
    { name: "Breaks", value: totalBreak, color: "var(--info)" },
  ];
}

/** Productivity legend items for bar charts. */
export const PRODUCTIVITY_LEGEND = [
  { label: "Productive", color: "var(--success)" },
  { label: "Neutral", color: "var(--text-muted)" },
  { label: "Distracting", color: "var(--destructive)" },
] as const;

interface TooltipPayloadEntry {
  dataKey: string;
  value: number;
  fill: string;
}

interface ChartTooltipProps {
  active?: boolean;
  payload?: TooltipPayloadEntry[];
  label?: string;
}

/** Shared tooltip for recharts bar charts. */
export function ChartTooltip({ active, payload, label }: ChartTooltipProps) {
  if (!active || !payload?.length) return null;
  const total = payload.reduce((s: number, p: TooltipPayloadEntry) => s + (p.value || 0), 0);
  return (
    <div
      className="rounded-lg px-3 py-2 text-[11px]"
      style={{
        background: "var(--surface-floating)",
        border: "1px solid var(--border)",
        boxShadow: "var(--shadow-tooltip)",
      }}
    >
      <div className="font-medium text-primary mb-1">{label}</div>
      {payload.map((p: TooltipPayloadEntry) => (
        <div key={p.dataKey} className="flex items-center gap-2 text-muted font-light">
          <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: p.fill }} />
          <span className="capitalize">{p.dataKey}</span>
          <span className="ml-auto tabular-nums">{p.value}h</span>
        </div>
      ))}
      <div className="border-t border-white/[0.04] mt-1 pt-1 flex justify-between text-primary font-medium">
        <span>Total</span>
        <span className="tabular-nums">{total.toFixed(1)}h</span>
      </div>
    </div>
  );
}
