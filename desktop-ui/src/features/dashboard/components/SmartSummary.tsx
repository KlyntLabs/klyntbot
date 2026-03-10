import { CONTEXT_TYPE_COLORS } from "@features/work-contexts";
import { useQuery } from "@shared/hooks/useQuery";
import { TZ_OFFSET_MINS } from "@shared/lib/dates";
import { ArrowDownRight, ArrowUpRight, Brain, Folder, Lightbulb, Minus, Zap } from "lucide-react";

interface WorkContextSummary {
  id: string;
  title: string;
  contextType: string;
  color: string | null;
  durationMins: number;
  confidence: number;
}

interface SessionBlock {
  contextType: string;
  totalDurationMins: number;
  sessionCount: number;
  color: string;
}

interface DashboardNudge {
  message: string;
  nudgeType: string;
  priority: string;
}

interface ResourceCluster {
  resources: string[];
  accessCount: number;
}

interface DashboardIntelligence {
  activeContext: WorkContextSummary | null;
  focusRecommendation: string | null;
  sessionSummary: SessionBlock[];
  contextSwitches: number;
  switchQuality: string;
  productivityScore: number;
  scoreTrend: number;
  patterns: string[];
  nudges: DashboardNudge[];
  resourceClusters: ResourceCluster[];
}

function useDashboardIntelligence(date: string) {
  return useQuery<DashboardIntelligence>(
    "get_dashboard_intelligence",
    { date, tzOffsetMins: TZ_OFFSET_MINS },
    undefined,
    30_000,
  );
}

function TrendArrow({ trend }: { trend: number }) {
  if (trend > 0.05) return <ArrowUpRight className="w-3 h-3 text-green-400" strokeWidth={2} />;
  if (trend < -0.05) return <ArrowDownRight className="w-3 h-3 text-red-400" strokeWidth={2} />;
  return <Minus className="w-3 h-3 text-muted" strokeWidth={2} />;
}

export function SmartSummary({ date }: { date: string }) {
  const { data: intel } = useDashboardIntelligence(date);

  if (!intel) {
    return (
      <div className="w-72 shrink-0 p-4 text-sm text-muted animate-pulse">
        Loading intelligence...
      </div>
    );
  }

  const scorePercent = Math.round(intel.productivityScore * 100);

  return (
    <div className="w-72 shrink-0 overflow-y-auto p-4 flex flex-col gap-5 border-l border-border">
      {/* Today's Focus */}
      <section>
        <h4 className="text-[10px] font-medium text-dim uppercase tracking-wider mb-2">
          Today's Focus
        </h4>
        {intel.activeContext ? (
          <div className="glass-card rounded-xl p-3 flex items-start gap-2.5">
            <div
              className="w-2.5 h-2.5 rounded-full mt-1 shrink-0"
              style={{
                backgroundColor:
                  intel.activeContext.color ??
                  CONTEXT_TYPE_COLORS[intel.activeContext.contextType] ??
                  "#6B7280",
              }}
            />
            <div className="flex-1 min-w-0">
              <p className="text-[13px] font-medium text-primary truncate">
                {intel.activeContext.title}
              </p>
              <p className="text-[11px] text-muted">
                {intel.activeContext.contextType} · {intel.activeContext.durationMins}m
              </p>
            </div>
          </div>
        ) : (
          <p className="text-[12px] text-muted">No active context</p>
        )}
        {intel.focusRecommendation && (
          <p className="mt-2 text-[11px] text-muted italic leading-relaxed">
            {intel.focusRecommendation}
          </p>
        )}
      </section>

      {/* Activity Intelligence */}
      <section>
        <h4 className="text-[10px] font-medium text-dim uppercase tracking-wider mb-2">
          Activity Intelligence
        </h4>

        {/* Session blocks */}
        {intel.sessionSummary.length > 0 && (
          <div className="flex flex-col gap-1 mb-3">
            {intel.sessionSummary.map((s) => (
              <div key={s.contextType} className="flex items-center gap-2 text-[11px]">
                <div
                  className="w-2 h-2 rounded-full shrink-0"
                  style={{ backgroundColor: s.color }}
                />
                <span className="text-secondary flex-1 capitalize">{s.contextType}</span>
                <span className="text-muted tabular-nums">
                  {s.totalDurationMins}m · {s.sessionCount} sessions
                </span>
              </div>
            ))}
          </div>
        )}

        {/* Score + switches */}
        <div className="grid grid-cols-2 gap-2">
          <div className="bg-white/[0.04] rounded-lg p-2.5 text-center">
            <div className="flex items-center justify-center gap-1">
              <span className="text-[16px] font-semibold text-primary tabular-nums">
                {scorePercent}%
              </span>
              <TrendArrow trend={intel.scoreTrend} />
            </div>
            <p className="text-[10px] text-muted mt-0.5">Focus Score</p>
          </div>
          <div className="bg-white/[0.04] rounded-lg p-2.5 text-center">
            <p className="text-[16px] font-semibold text-primary tabular-nums">
              {intel.contextSwitches}
            </p>
            <p className="text-[10px] text-muted mt-0.5">Switches ({intel.switchQuality})</p>
          </div>
        </div>
      </section>

      {/* Insights */}
      {(intel.patterns.length > 0 ||
        intel.nudges.length > 0 ||
        intel.resourceClusters.length > 0) && (
        <section>
          <h4 className="text-[10px] font-medium text-dim uppercase tracking-wider mb-2">
            Insights
          </h4>
          <div className="flex flex-col gap-2">
            {intel.patterns.map((p, i) => (
              <div key={i} className="flex items-start gap-2 text-[11px]">
                <Brain className="w-3 h-3 text-muted mt-0.5 shrink-0" />
                <span className="text-secondary">{p}</span>
              </div>
            ))}
            {intel.nudges.map((n, i) => (
              <div key={i} className="flex items-start gap-2 text-[11px]">
                <Lightbulb className="w-3 h-3 text-amber-400 mt-0.5 shrink-0" />
                <span className="text-secondary">{n.message}</span>
              </div>
            ))}
            {intel.resourceClusters.map((c, i) => (
              <div key={i} className="flex items-start gap-2 text-[11px]">
                <Folder className="w-3 h-3 text-muted mt-0.5 shrink-0" />
                <span className="text-muted">
                  {c.resources.slice(0, 3).join(", ")}
                  {c.resources.length > 3 && ` +${c.resources.length - 3}`}
                </span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
