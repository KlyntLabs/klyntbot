import { X } from "lucide-react";
import { useState } from "react";

interface AddGoalDialogProps {
  open: boolean;
  onClose: () => void;
  onAdd: (params: { goal_type: string; metric: string; target_value: number }) => void;
}

const METRICS = [
  { value: "productive_hours", label: "Productive hours", unit: "hours", placeholder: "6" },
  { value: "focus_sessions", label: "Focus sessions", unit: "sessions", placeholder: "4" },
  { value: "productivity_score", label: "Productivity score", unit: "/100", placeholder: "70" },
  {
    value: "max_distracting_mins",
    label: "Max distracting minutes",
    unit: "mins",
    placeholder: "30",
  },
] as const;

export function AddGoalDialog({ open, onClose, onAdd }: AddGoalDialogProps) {
  const [goalType, setGoalType] = useState<"daily" | "weekly">("daily");
  const [metric, setMetric] = useState<string>(METRICS[0].value);
  const [targetValue, setTargetValue] = useState("");

  if (!open) return null;

  const selectedMetric = METRICS.find((m) => m.value === metric) ?? METRICS[0];
  const canSubmit = targetValue.trim() !== "" && Number(targetValue) > 0;

  const handleSubmit = () => {
    onAdd({ goal_type: goalType, metric, target_value: Number(targetValue) });
    setTargetValue("");
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="glass-panel w-[400px]">
        <div className="bg-white/[0.04] rounded-[var(--glass-radius-inner)]">
          <div className="flex items-center justify-between px-5 py-4 border-b border-white/[0.08]">
            <h3 className="text-[14px] font-medium text-primary">Add Goal</h3>
            <button
              type="button"
              onClick={onClose}
              className="w-7 h-7 rounded-md flex items-center justify-center text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <div className="px-5 py-4 space-y-4">
            <div>
              <span className="block text-[12px] text-muted mb-1.5">Period</span>
              <div className="flex gap-2">
                {(["daily", "weekly"] as const).map((t) => (
                  <button
                    key={t}
                    type="button"
                    onClick={() => setGoalType(t)}
                    className={`flex-1 py-1.5 text-[12px] rounded-md border transition-colors capitalize ${
                      goalType === t
                        ? "border-brand/50 text-brand bg-brand/5"
                        : "border-white/[0.08] text-muted bg-white/[0.06] hover:bg-white/[0.08]"
                    }`}
                  >
                    {t}
                  </button>
                ))}
              </div>
            </div>

            <div>
              <span className="block text-[12px] text-muted mb-1.5">Metric</span>
              <div className="flex flex-col gap-1.5">
                {METRICS.map((m) => (
                  <button
                    key={m.value}
                    type="button"
                    onClick={() => setMetric(m.value)}
                    className={`px-3 py-2 text-[12px] text-left rounded-md border transition-colors ${
                      metric === m.value
                        ? "border-brand/50 text-brand bg-brand/5"
                        : "border-white/[0.08] text-muted bg-white/[0.06] hover:bg-white/[0.08]"
                    }`}
                  >
                    {m.label}
                  </button>
                ))}
              </div>
            </div>

            <div>
              <label htmlFor="goal-target" className="block text-[12px] text-muted mb-1.5">
                Target <span className="text-dim">({selectedMetric.unit})</span>
              </label>
              <input
                id="goal-target"
                type="number"
                value={targetValue}
                onChange={(e) => setTargetValue(e.target.value)}
                placeholder={selectedMetric.placeholder}
                min={0}
                step={metric === "productive_hours" ? 0.5 : 1}
                className="w-full px-3 py-1.5 text-[13px] bg-white/[0.06] border border-white/[0.08] rounded-md text-primary placeholder:text-dim"
              />
            </div>
          </div>

          <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-white/[0.08]">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 text-[12px] text-muted hover:text-secondary rounded-md hover:bg-white/[0.06] transition-colors"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleSubmit}
              disabled={!canSubmit}
              className="px-4 py-1.5 text-[12px] rounded-md bg-brand text-white hover:bg-brand-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Add goal
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
