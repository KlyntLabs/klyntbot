import { X } from "lucide-react";
import { useState } from "react";

interface AddGoalDialogProps {
  open: boolean;
  onClose: () => void;
  onAdd: (params: { goalType: string; metric: string; targetValue: number }) => void;
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
    onAdd({ goalType, metric, targetValue: Number(targetValue) });
    setTargetValue("");
    onClose();
  };

  return (
    <div className="dashboard__goal-dialog-backdrop">
      <div className="dashboard__goal-dialog">
        <div className="dashboard__goal-dialog-header">
          <h3>Add Goal</h3>
          <button type="button" onClick={onClose} aria-label="Close dialog">
            <X aria-hidden />
          </button>
        </div>

        <div className="dashboard__goal-dialog-body">
          <div className="dashboard__goal-dialog-section">
            <span>Period</span>
            <div className="dashboard__goal-dialog-period-toggle">
              {(["daily", "weekly"] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => setGoalType(t)}
                  className={
                    goalType === t
                      ? "dashboard__goal-dialog-period-btn dashboard__goal-dialog-period-btn--active"
                      : "dashboard__goal-dialog-period-btn"
                  }
                >
                  {t}
                </button>
              ))}
            </div>
          </div>

          <div className="dashboard__goal-dialog-section">
            <span>Metric</span>
            <div className="dashboard__goal-dialog-metric-list">
              {METRICS.map((m) => (
                <button
                  key={m.value}
                  type="button"
                  onClick={() => setMetric(m.value)}
                  className={
                    metric === m.value
                      ? "dashboard__goal-dialog-metric-btn dashboard__goal-dialog-metric-btn--active"
                      : "dashboard__goal-dialog-metric-btn"
                  }
                >
                  {m.label}
                </button>
              ))}
            </div>
          </div>

          <div className="dashboard__goal-dialog-section">
            <label htmlFor="goal-target">
              Target <span>({selectedMetric.unit})</span>
            </label>
            <input
              id="goal-target"
              type="number"
              value={targetValue}
              onChange={(e) => setTargetValue(e.target.value)}
              placeholder={selectedMetric.placeholder}
              min={0}
              step={metric === "productive_hours" ? 0.5 : 1}
              className="dashboard__goal-dialog-input"
            />
          </div>
        </div>

        <div className="dashboard__goal-dialog-footer">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="button" onClick={handleSubmit} disabled={!canSubmit}>
            Add goal
          </button>
        </div>
      </div>
    </div>
  );
}
