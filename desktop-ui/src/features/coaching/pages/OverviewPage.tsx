import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries, useQuery } from "@shared/hooks/useQuery";
import { formatTime } from "@shared/lib/dates";
import type { CoachingIntervention } from "@shared/types";
import { Check, RefreshCw, Trash2, X, XCircle } from "lucide-react";
import { useEffect } from "react";
import { useNavigate } from "react-router";
import { FeedbackBadge } from "../components/FeedbackBadge";
import { MorningBriefing } from "../components/MorningBriefing";
import type { DetectedPattern, InterventionLog } from "../types";

// ── Inline types (matching Rust backend) ─────────────────

interface UserSituation {
  energyLevel: number;
  focusState: number;
  deadlinePressure: number;
  distractionRisk: number;
  coachingReceptivity: number;
  taskAvoidanceDetected: boolean;
  hoursActiveToday: number;
  minsSinceBreak: number;
  hourOfDay: number;
  recentContextSwitches: number;
}

interface SignalWindow {
  windowSize: number;
  signals: {
    eventType: string;
    timestamp: string;
    metadata: string;
  }[];
  triggers: {
    name: string;
    cooldownRemainingSecs: number;
    lastFired: string | null;
  }[];
}

interface RouterStatus {
  hourlyCount: number;
  hourlyLimit: number;
  dailyCount: number;
  dailyLimit: number;
}

interface StrategyFeedback {
  strategyType: string;
  domain: string;
  timesUsed: number;
  acceptanceRate: number;
  effectiveness: number;
  behavioralPositive: number;
  behavioralNegative: number;
}

// ── Gauge ────────────────────────────────────────────────

function Gauge({
  label,
  value,
  color = "text-brand",
}: {
  label: string;
  value: number;
  color?: string;
}) {
  const pct = Math.round(value * 100);
  return (
    <div className="flex flex-col items-center gap-1">
      <div className="relative size-16">
        <svg className="size-16 -rotate-90" viewBox="0 0 36 36" aria-hidden="true">
          <circle
            className="text-white/[0.08]"
            strokeWidth="3"
            stroke="currentColor"
            fill="none"
            r="15.5"
            cx="18"
            cy="18"
          />
          <circle
            className={color}
            strokeWidth="3"
            stroke="currentColor"
            fill="none"
            r="15.5"
            cx="18"
            cy="18"
            strokeDasharray={`${pct} 100`}
            strokeLinecap="round"
          />
        </svg>
        <span className="absolute inset-0 flex items-center justify-center text-ui-xs text-fg-secondary font-mono">
          {pct}%
        </span>
      </div>
      <span className="text-ui-xs text-fg-secondary text-center leading-tight">{label}</span>
    </div>
  );
}

// ── Polling interval ─────────────────────────────────────

const POLL_INTERVAL = 5_000;

// ── Default values ───────────────────────────────────────

const DEFAULT_SITUATION: UserSituation = {
  energyLevel: 0,
  focusState: 0,
  deadlinePressure: 0,
  distractionRisk: 0,
  coachingReceptivity: 0,
  taskAvoidanceDetected: false,
  hoursActiveToday: 0,
  minsSinceBreak: 0,
  hourOfDay: 0,
  recentContextSwitches: 0,
};

const DEFAULT_SIGNALS: SignalWindow = {
  windowSize: 0,
  signals: [],
  triggers: [],
};

const DEFAULT_ROUTER: RouterStatus = {
  hourlyCount: 0,
  hourlyLimit: 3,
  dailyCount: 0,
  dailyLimit: 10,
};

// ── Component ────────────────────────────────────────────

export function CoachingOverviewPage() {
  const navigate = useNavigate();

  // Queries
  const { data: situation, refetch: rSit } = useQuery<UserSituation>(
    "coaching_situation",
    undefined,
    DEFAULT_SITUATION,
  );
  const { data: signals, refetch: rSig } = useQuery<SignalWindow>(
    "coaching_signals",
    undefined,
    DEFAULT_SIGNALS,
  );
  const { data: patterns, refetch: rPat } = useQuery<DetectedPattern[]>(
    "coaching_patterns",
    undefined,
    [],
  );
  const { data: feedback, refetch: rFb } = useQuery<StrategyFeedback[]>(
    "coaching_feedback_stats",
    undefined,
    [],
  );
  const { data: router, refetch: rRtr } = useQuery<RouterStatus>(
    "coaching_router_status",
    undefined,
    DEFAULT_ROUTER,
  );
  const { data: interventions, refetch: rInt } = useQuery<CoachingIntervention[]>(
    "coaching_pending_interventions",
    undefined,
    [],
  );
  const { data: history, refetch: rHist } = useQuery<InterventionLog[]>(
    "coaching_intervention_log",
    { limit: 5 },
    [],
  );

  // 5s polling
  useEffect(() => {
    const id = setInterval(() => {
      rSit();
      rSig();
      rPat();
      rFb();
      rRtr();
      rInt();
      rHist();
    }, POLL_INTERVAL);
    return () => clearInterval(id);
  }, [rSit, rSig, rPat, rFb, rRtr, rInt, rHist]);

  // Mutations
  const { mutate: clearSignals } = useMutation("coaching_clear_signals");
  const { mutate: resetDismissals } = useMutation("coaching_reset_dismissals");
  const { mutate: submitFeedback } = useMutation("coaching_submit_feedback");

  const handleClearSignals = async () => {
    await clearSignals({} as never);
    invalidateQueries("coaching_");
  };

  const handleResetDismissals = async () => {
    await resetDismissals({} as never);
    invalidateQueries("coaching_");
  };

  const handleFeedback = async (
    interventionId: string,
    response: "helpful" | "dismissed" | "stop",
  ) => {
    await submitFeedback({
      intervention_id: interventionId,
      response,
    } as never);
    invalidateQueries("coaching_");
  };

  return (
    <div className="flex flex-col gap-4">
      {/* ── 0. Morning Briefing — Knowledge Health ───── */}
      <MorningBriefing />

      {/* ── 1. User Situation ──────────────────────────── */}
      <div className="island p-5">
        <h2 className="text-ui font-medium text-fg-secondary mb-3">User Situation</h2>
        <div className="flex items-start gap-4">
          <Gauge label="Energy" value={situation.energyLevel} />
          <Gauge label="Focus" value={situation.focusState} />
          <Gauge label="Deadline" value={situation.deadlinePressure} color="text-status-danger" />
          <Gauge label="Distraction" value={situation.distractionRisk} color="text-brand" />
          <Gauge label="Receptivity" value={situation.coachingReceptivity} color="text-status-success" />
          <div className="flex flex-col gap-1 ml-4 text-ui-xs">
            <span className="text-fg-secondary">
              Hours active:{" "}
              <span className="tabular-nums">{situation.hoursActiveToday.toFixed(1)}h</span>
            </span>
            <span className="text-fg-secondary">
              Since break:{" "}
              <span className="tabular-nums">{situation.minsSinceBreak.toFixed(0)}min</span>
            </span>
            <span className="text-fg-secondary">
              Context switches:{" "}
              <span className="tabular-nums">{situation.recentContextSwitches}</span>
            </span>
            {situation.taskAvoidanceDetected && (
              <span className="text-brand font-medium">Task avoidance detected</span>
            )}
          </div>
        </div>
      </div>

      {/* ── 2. Active Interventions ────────────────────── */}
      {interventions.length > 0 && (
        <div className="island p-5">
          <h2 className="text-ui font-medium text-fg-secondary mb-3">
            Active Interventions ({interventions.length})
          </h2>
          <div className="flex flex-col gap-2">
            {interventions.map((iv) => (
              <div key={iv.id} className="p-3 rounded-panel bg-control-hover/30 border border-brand/30">
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-ui-xs px-1.5 py-0.5 bg-brand/20 text-brand rounded font-medium">
                        {iv.interventionType}
                      </span>
                      <span className="text-ui-xs text-fg-secondary">{iv.triggerName}</span>
                    </div>
                    <p className="text-ui-sm text-fg-secondary leading-relaxed">{iv.message}</p>
                  </div>
                  <div className="flex gap-1 shrink-0">
                    <button
                      type="button"
                      onClick={() => handleFeedback(iv.id, "helpful")}
                      className="p-1.5 rounded hover:bg-status-success/20 text-fg-secondary hover:text-status-success transition-colors"
                      title="Helpful"
                    >
                      <Check className="size-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => handleFeedback(iv.id, "dismissed")}
                      className="p-1.5 rounded hover:bg-control-hover text-fg-secondary hover:text-fg transition-colors"
                      title="Dismiss"
                    >
                      <X className="size-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => handleFeedback(iv.id, "stop")}
                      className="p-1.5 rounded hover:bg-status-danger/20 text-fg-secondary hover:text-status-danger transition-colors"
                      title="Stop suggesting"
                    >
                      <XCircle className="size-3.5" />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* ── 3. Two-column grid ─────────────────────────── */}
      <div className="grid grid-cols-2 gap-4">
        {/* Left column */}
        <div className="flex flex-col gap-4">
          {/* Signal Accumulator */}
          <div className="island p-5">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-ui font-medium text-fg-secondary">Signal Accumulator</h2>
              <button
                type="button"
                onClick={handleClearSignals}
                className="text-ui-xs text-fg-secondary hover:text-fg flex items-center gap-1"
              >
                <Trash2 className="size-3" /> Clear
              </button>
            </div>
            <p className="text-ui-sm text-fg-secondary">
              {signals.windowSize} signals in 30min window
            </p>
            {signals.triggers.length > 0 && (
              <div className="mt-2 flex flex-col gap-1">
                {signals.triggers.map((t) => (
                  <div key={t.name} className="flex items-center justify-between text-ui-xs">
                    <span className="text-fg-secondary">{t.name}</span>
                    <span className="text-fg-secondary tabular-nums">
                      {t.cooldownRemainingSecs > 0
                        ? `${t.cooldownRemainingSecs}s cooldown`
                        : "ready"}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Detected Patterns preview */}
          <div className="island p-5">
            <div className="flex items-baseline justify-between mb-3">
              <h2 className="text-ui font-medium text-fg-secondary">Detected Patterns</h2>
              {patterns.length > 0 && (
                <button
                  type="button"
                  onClick={() => navigate("/coaching/patterns")}
                  className="text-ui-xs text-brand hover:underline"
                >
                  View all
                </button>
              )}
            </div>
            {patterns.length > 0 ? (
              <div className="flex flex-col gap-1.5">
                {patterns.slice(0, 4).map((p) => (
                  <div
                    key={p.name}
                    className="flex items-center gap-3 rounded-panel bg-control-hover/30 px-3 py-2"
                  >
                    <span className="text-ui-xs font-medium text-fg w-44 shrink-0 truncate">
                      {p.name}
                    </span>
                    <span className="text-ui-xs text-fg-secondary flex-1 truncate">
                      {p.description}
                    </span>
                    <span className="text-ui-xs text-fg-dim tabular-nums shrink-0">
                      {Math.round(p.confidence * 100)}%
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-ui-xs text-fg-secondary">
                No patterns detected yet. Patterns emerge as the coaching system observes your work
                habits over time.
              </p>
            )}
          </div>
        </div>

        {/* Right column */}
        <div className="flex flex-col gap-4">
          {/* Intervention Router */}
          <div className="island p-5">
            <h2 className="text-ui font-medium text-fg-secondary mb-3">
              Intervention Router
            </h2>
            <div className="flex gap-4 text-ui-sm">
              <span className="text-fg-secondary">
                Hourly:{" "}
                <span className="tabular-nums">
                  {router.hourlyCount}/{router.hourlyLimit}
                </span>
              </span>
              <span className="text-fg-secondary">
                Daily:{" "}
                <span className="tabular-nums">
                  {router.dailyCount}/{router.dailyLimit}
                </span>
              </span>
            </div>
          </div>

          {/* Strategy Feedback */}
          <div className="island p-5">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-ui font-medium text-fg-secondary">Strategy Feedback</h2>
              <button
                type="button"
                onClick={handleResetDismissals}
                className="text-ui-xs text-fg-secondary hover:text-fg flex items-center gap-1"
              >
                <RefreshCw className="size-3" /> Reset All
              </button>
            </div>
            <div className="rounded-panel overflow-hidden">
              <table className="w-full text-ui-sm">
                <thead>
                  <tr className="border-b border-separator">
                    <th className="text-left p-2 text-fg-secondary font-normal">Trigger</th>
                    <th className="text-left p-2 text-fg-secondary font-normal">Type</th>
                    <th className="text-left p-2 text-fg-secondary font-normal">Used</th>
                    <th className="text-left p-2 text-fg-secondary font-normal">Accept</th>
                    <th className="text-left p-2 text-fg-secondary font-normal">Effect</th>
                  </tr>
                </thead>
                <tbody>
                  {feedback.map((s) => (
                    <tr key={s.strategyType} className="border-b border-separator">
                      <td className="p-2 text-fg-secondary">{s.strategyType}</td>
                      <td className="p-2 text-fg-secondary">{s.domain}</td>
                      <td className="p-2 text-fg-secondary tabular-nums">{s.timesUsed}</td>
                      <td className="p-2 text-fg-secondary tabular-nums">
                        {(s.acceptanceRate * 100).toFixed(0)}%
                      </td>
                      <td className="p-2 text-fg-secondary tabular-nums">
                        {(s.effectiveness * 100).toFixed(0)}%
                      </td>
                    </tr>
                  ))}
                  {feedback.length === 0 && (
                    <tr>
                      <td colSpan={5} className="p-4 text-center text-fg-secondary">
                        No feedback data
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>

          {/* Recent Interventions preview */}
          <div className="island p-5">
            <div className="flex items-baseline justify-between mb-3">
              <h2 className="text-ui font-medium text-fg-secondary">
                Recent Interventions
              </h2>
              {history.length > 0 && (
                <button
                  type="button"
                  onClick={() => navigate("/coaching/history")}
                  className="text-ui-xs text-brand hover:underline"
                >
                  View all
                </button>
              )}
            </div>
            {history.length > 0 ? (
              <div className="flex flex-col gap-2">
                {history.map((h) => (
                  <div key={h.id} className="flex items-center gap-3 py-1.5">
                    <span className="text-ui-xs text-fg-dim tabular-nums w-14 shrink-0">
                      {formatTime(h.deliveredAt)}
                    </span>
                    <p className="text-ui-xs text-fg truncate flex-1">{h.message}</p>
                    <FeedbackBadge feedback={h.feedback} />
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-ui-xs text-fg-secondary">
                No coaching interventions yet. The system will start offering suggestions as it
                learns your patterns.
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
