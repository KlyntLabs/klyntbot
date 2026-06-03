import { productivityPatternsQuery } from "@/api/endpoints/dashboard";
import type { ProductivityPatternsResponse } from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";

const FALLBACK: ProductivityPatternsResponse = {
  daysAnalyzed: 0,
  peakFocusHours: [],
  bestDayOfWeek: null,
  avgSessionMins: 0,
  productiveRatio: 0,
  avgContextSwitches: 0,
};

export function PatternsCard() {
  const { data } = useTauriQuery<ProductivityPatternsResponse>({
    queryKey: qk.productivity.patterns(null),
    queryFn: () => productivityPatternsQuery(null),
    fallback: FALLBACK,
    staleTime: 5 * 60 * 1000,
  });

  if (!data || data.daysAnalyzed < 3) {
    const remaining = Math.max(0, 3 - (data?.daysAnalyzed ?? 0));
    return (
      <div className="px-1 py-2 flex flex-col gap-1 opacity-70">
        <div className="text-ui-2xs font-medium text-ds-text-strong">Your Patterns</div>
        <div className="text-ui-2xs text-ds-text-subtle">
          {remaining === 3
            ? "Patterns appear after 3 days of tracking."
            : `${remaining} more day${remaining === 1 ? "" : "s"} of tracking until patterns appear.`}
        </div>
      </div>
    );
  }

  const peakLabel =
    data.peakFocusHours.length > 0 ? data.peakFocusHours.map((h) => `${h}:00`).join(", ") : "—";

  return (
    <div className="px-1 py-2 flex flex-col gap-1">
      <div className="text-ui-2xs font-medium text-ds-text-strong">Your Patterns</div>
      <div>
        <div className="text-ui-2xs text-ds-text-subtle">Peak hours: {peakLabel}</div>
        {data.bestDayOfWeek && (
          <div className="text-ui-2xs text-ds-text-subtle">Best day: {data.bestDayOfWeek}</div>
        )}
        <div className="text-ui-2xs text-ds-text-subtle">
          Avg session: {Math.round(data.avgSessionMins)}min
        </div>
        <div className="text-ui-2xs text-[color-mix(in_srgb,var(--ds-text-subtle)_60%,transparent)]">{data.daysAnalyzed} days analyzed</div>
      </div>
    </div>
  );
}
