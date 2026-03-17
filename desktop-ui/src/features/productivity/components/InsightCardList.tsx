import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type { InsightCard, InsightPayload } from "@shared/types";

const SENTIMENT_STYLES: Record<string, { bg: string; text: string; label: string }> = {
  positive: { bg: "bg-success/15", text: "text-success", label: "Positive" },
  warning: {
    bg: "bg-[var(--text-muted-foreground)]/15",
    text: "text-[var(--text-muted-foreground)]",
    label: "Heads up",
  },
  negative: { bg: "bg-destructive/15", text: "text-destructive", label: "Alert" },
  neutral: { bg: "bg-card", text: "text-muted-foreground", label: "Info" },
};

function sentimentStyle(sentiment: string) {
  return SENTIMENT_STYLES[sentiment] ?? SENTIMENT_STYLES.neutral;
}

function MetricComparison({
  metric,
  baseline,
}: {
  metric: number | null;
  baseline: number | null;
}) {
  if (metric == null || baseline == null || baseline === 0) return null;
  const pctChange = ((metric - baseline) / baseline) * 100;
  const isUp = pctChange > 0;
  return (
    <span
      className={`text-[10px] font-medium tabular-nums ${isUp ? "text-success" : "text-destructive"}`}
    >
      {isUp ? "+" : ""}
      {pctChange.toFixed(0)}%
    </span>
  );
}

interface InsightCardListProps {
  date: string;
}

export function InsightCardList({ date }: InsightCardListProps) {
  const { data: cards, refetch } = useQuery<InsightCard[]>("productivity_insights", { date }, []);

  useEvent<InsightPayload>("insight:generated", () => refetch());

  const handleDismiss = async (id: string) => {
    try {
      await ipc("productivity_insight_dismiss", { id });
      refetch();
    } catch {
      // silently ignore — will reconcile on next refetch
    }
  };

  const visible = cards.filter((c) => !c.dismissed);
  if (visible.length === 0) return null;

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-muted-foreground">Insights</h2>
      <div className="flex flex-col gap-2">
        {visible.map((card) => {
          const style = sentimentStyle(card.sentiment);
          return (
            <div key={card.id} className="group flex flex-col gap-1.5 p-2.5 rounded-lg bg-card">
              <div className="flex items-center gap-2">
                <span
                  className={`text-[9px] font-medium px-1.5 py-0.5 rounded-full ${style.bg} ${style.text}`}
                >
                  {style.label}
                </span>
                <span className="text-[11px] font-medium text-foreground flex-1 truncate">
                  {card.title}
                </span>
                <MetricComparison metric={card.metricValue} baseline={card.baselineValue} />
                <button
                  type="button"
                  onClick={() => handleDismiss(card.id)}
                  className="opacity-0 group-hover:opacity-100 text-[10px] text-dim hover:text-muted-foreground transition-opacity"
                  aria-label="Dismiss insight"
                >
                  &times;
                </button>
              </div>
              <p className="text-[11px] font-light text-muted-foreground leading-relaxed">
                {card.body}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
}
