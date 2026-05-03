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
      <div className="dashboard__patterns dashboard__patterns--empty">
        <div className="dashboard__patterns-title">Your Patterns</div>
        <div className="dashboard__patterns-row">
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
    <div className="dashboard__patterns">
      <div className="dashboard__patterns-title">Your Patterns</div>
      <div>
        <div className="dashboard__patterns-row">Peak hours: {peakLabel}</div>
        {data.bestDayOfWeek && (
          <div className="dashboard__patterns-row">Best day: {data.bestDayOfWeek}</div>
        )}
        <div className="dashboard__patterns-row">
          Avg session: {Math.round(data.avgSessionMins)}min
        </div>
        <div className="dashboard__patterns-footer">{data.daysAnalyzed} days analyzed</div>
      </div>
    </div>
  );
}
