import { Check, RefreshCw, Trash2, X, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useEvent } from "../../../hooks/useEvent";
import { useMutation } from "../../../hooks/useMutation";
import { invalidateQueries, useQuery } from "../../../hooks/useQuery";
import type { CoachingIntervention } from "../../../lib/types";

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
  signals: { eventType: string; timestamp: string; metadata: string }[];
  triggers: { name: string; cooldownRemainingSecs: number; lastFired: string | null }[];
}

interface DetectedPattern {
  name: string;
  confidence: number;
  signalCount: number;
  description: string;
  domain: string;
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

interface RouterStatus {
  hourlyCount: number;
  hourlyLimit: number;
  dailyCount: number;
  dailyLimit: number;
}

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
      <div className="relative w-16 h-16">
        <svg className="w-16 h-16 -rotate-90" viewBox="0 0 36 36" aria-hidden="true">
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
        <span className="absolute inset-0 flex items-center justify-center text-[11px] text-secondary font-mono">
          {pct}%
        </span>
      </div>
      <span className="text-[10px] text-muted text-center leading-tight">{label}</span>
    </div>
  );
}

const POLL_INTERVAL = 5_000;

export function CoachingTab() {
  const { data: situation, refetch: rSit } = useQuery<UserSituation>(
    "coaching_situation",
    undefined,
    {
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
    },
  );

  const { data: signals, refetch: rSig } = useQuery<SignalWindow>("coaching_signals", undefined, {
    windowSize: 0,
    signals: [],
    triggers: [],
  });

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
    {
      hourlyCount: 0,
      hourlyLimit: 3,
      dailyCount: 0,
      dailyLimit: 10,
    },
  );

  const { data: interventions, refetch: rInt } = useQuery<CoachingIntervention[]>(
    "coaching_pending_interventions",
    undefined,
    [],
  );

  // Poll all coaching data every 5s so the debug view stays fresh.
  useEffect(() => {
    const id = setInterval(() => {
      rSit();
      rSig();
      rPat();
      rFb();
      rRtr();
      rInt();
    }, POLL_INTERVAL);
    return () => clearInterval(id);
  }, [rSit, rSig, rPat, rFb, rRtr, rInt]);

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
    await submitFeedback({ intervention_id: interventionId, response } as never);
    invalidateQueries("coaching_");
  };

  return (
    <div className="space-y-6">
      {/* Situation Gauges */}
      <div>
        <h2 className="text-[13px] font-medium text-secondary mb-3">User Situation</h2>
        <div className="flex gap-4 items-start p-4 bg-white/[0.04] rounded-lg border border-white/[0.08]">
          <Gauge label="Energy" value={situation.energyLevel} />
          <Gauge label="Focus" value={situation.focusState} />
          <Gauge label="Deadline" value={situation.deadlinePressure} color="text-red-500" />
          <Gauge label="Distraction" value={situation.distractionRisk} color="text-orange-500" />
          <Gauge label="Receptivity" value={situation.coachingReceptivity} color="text-green-500" />
          <div className="flex flex-col gap-1 ml-4 text-[11px]">
            <span className="text-muted">
              Hours active:{" "}
              <span className="text-secondary">{situation.hoursActiveToday.toFixed(1)}h</span>
            </span>
            <span className="text-muted">
              Since break:{" "}
              <span className="text-secondary">{situation.minsSinceBreak.toFixed(0)}min</span>
            </span>
            <span className="text-muted">
              Context switches:{" "}
              <span className="text-secondary">{situation.recentContextSwitches}</span>
            </span>
            {situation.taskAvoidanceDetected && (
              <span className="text-orange-400 font-medium">Task avoidance detected</span>
            )}
          </div>
        </div>
      </div>

      {/* Active Interventions */}
      {interventions.length > 0 && (
        <div>
          <h2 className="text-[13px] font-medium text-secondary mb-3">
            Active Interventions ({interventions.length})
          </h2>
          <div className="space-y-2">
            {interventions.map((iv) => (
              <div key={iv.id} className="p-3 bg-white/[0.04] rounded-lg border border-brand/30">
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-[10px] px-1.5 py-0.5 bg-brand/20 text-brand rounded font-medium">
                        {iv.interventionType}
                      </span>
                      <span className="text-[10px] text-muted">{iv.triggerName}</span>
                    </div>
                    <p className="text-[12px] text-secondary leading-relaxed">{iv.message}</p>
                  </div>
                  <div className="flex gap-1 shrink-0">
                    <button
                      type="button"
                      onClick={() => handleFeedback(iv.id, "helpful")}
                      className="p-1.5 rounded hover:bg-green-500/20 text-muted hover:text-green-400 transition-colors"
                      title="Helpful"
                    >
                      <Check className="w-3.5 h-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => handleFeedback(iv.id, "dismissed")}
                      className="p-1.5 rounded hover:bg-white/10 text-muted hover:text-secondary transition-colors"
                      title="Dismiss"
                    >
                      <X className="w-3.5 h-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => handleFeedback(iv.id, "stop")}
                      className="p-1.5 rounded hover:bg-red-500/20 text-muted hover:text-red-400 transition-colors"
                      title="Stop suggesting"
                    >
                      <XCircle className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="grid grid-cols-2 gap-6">
        {/* Left: Signals & Patterns */}
        <div className="space-y-4">
          <div>
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-[13px] font-medium text-secondary">Signal Accumulator</h3>
              <button
                type="button"
                onClick={handleClearSignals}
                className="text-[10px] text-muted hover:text-secondary flex items-center gap-1"
              >
                <Trash2 className="w-3 h-3" /> Clear
              </button>
            </div>
            <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
              <p className="text-[12px] text-muted">{signals.windowSize} signals in 30min window</p>
              {signals.triggers.length > 0 && (
                <div className="mt-2 space-y-1">
                  {signals.triggers.map((t) => (
                    <div key={t.name} className="flex items-center justify-between text-[11px]">
                      <span className="text-secondary">{t.name}</span>
                      <span className="text-muted">
                        {t.cooldownRemainingSecs > 0
                          ? `${t.cooldownRemainingSecs}s cooldown`
                          : "ready"}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          <div>
            <h3 className="text-[13px] font-medium text-secondary mb-2">Detected Patterns</h3>
            <div className="space-y-2">
              {patterns.map((p) => (
                <div
                  key={p.name}
                  className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]"
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-[12px] text-secondary font-medium">{p.name}</span>
                    <span className="text-[10px] text-muted">{p.signalCount} signals</span>
                  </div>
                  <div className="w-full bg-white/[0.1] rounded-full h-1 mb-1">
                    <div
                      className="bg-brand h-1 rounded-full"
                      style={{ width: `${p.confidence * 100}%` }}
                    />
                  </div>
                  <p className="text-[11px] text-muted">{p.description}</p>
                </div>
              ))}
              {patterns.length === 0 && (
                <p className="text-[12px] text-muted">No patterns detected</p>
              )}
            </div>
          </div>
        </div>

        {/* Right: Router & Feedback */}
        <div className="space-y-4">
          <div>
            <h3 className="text-[13px] font-medium text-secondary mb-2">Intervention Router</h3>
            <div className="p-3 bg-white/[0.04] rounded-lg border border-white/[0.08]">
              <div className="flex gap-4 text-[12px]">
                <span className="text-muted">
                  Hourly:{" "}
                  <span className="text-secondary">
                    {router.hourlyCount}/{router.hourlyLimit}
                  </span>
                </span>
                <span className="text-muted">
                  Daily:{" "}
                  <span className="text-secondary">
                    {router.dailyCount}/{router.dailyLimit}
                  </span>
                </span>
              </div>
            </div>
          </div>

          <div>
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-[13px] font-medium text-secondary">Strategy Feedback</h3>
              <button
                type="button"
                onClick={handleResetDismissals}
                className="text-[10px] text-muted hover:text-secondary flex items-center gap-1"
              >
                <RefreshCw className="w-3 h-3" /> Reset All
              </button>
            </div>
            <div className="bg-white/[0.04] rounded-lg border border-white/[0.08] overflow-hidden">
              <table className="w-full text-[12px]">
                <thead>
                  <tr className="border-b border-white/[0.06]">
                    <th className="text-left p-2 text-muted font-normal">Trigger</th>
                    <th className="text-left p-2 text-muted font-normal">Type</th>
                    <th className="text-left p-2 text-muted font-normal">Used</th>
                    <th className="text-left p-2 text-muted font-normal">Accept</th>
                    <th className="text-left p-2 text-muted font-normal">Effect</th>
                  </tr>
                </thead>
                <tbody>
                  {feedback.map((s) => (
                    <tr key={s.strategyType} className="border-b border-white/[0.04]">
                      <td className="p-2 text-secondary">{s.strategyType}</td>
                      <td className="p-2 text-muted">{s.domain}</td>
                      <td className="p-2 text-muted">{s.timesUsed}</td>
                      <td className="p-2 text-muted">{(s.acceptanceRate * 100).toFixed(0)}%</td>
                      <td className="p-2 text-muted">{(s.effectiveness * 100).toFixed(0)}%</td>
                    </tr>
                  ))}
                  {feedback.length === 0 && (
                    <tr>
                      <td colSpan={5} className="p-4 text-center text-muted">
                        No feedback data
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
