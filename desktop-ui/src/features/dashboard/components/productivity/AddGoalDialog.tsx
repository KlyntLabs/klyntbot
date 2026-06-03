import X from "lucide-react/dist/esm/icons/x";
import { useState } from "react";
import { cn } from "@/utils/cn";

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
  const numericValue = Number(targetValue);
  const isEmpty = targetValue.trim() === "";
  const isInvalid = !isEmpty && (Number.isNaN(numericValue) || numericValue <= 0);
  const canSubmit = !isEmpty && !isInvalid;

  const handleSubmit = () => {
    onAdd({ goalType, metric, targetValue: Number(targetValue) });
    setTargetValue("");
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-[400px] bg-surface-card-strong border border-ds-border-subtle rounded-xl">
        <div className="flex items-center justify-between px-5 py-3 border-b border-ds-border-subtle">
          <h2 className="text-[var(--fs-base)] font-medium m-0">Add Goal</h2>
          <button type="button" onClick={onClose} aria-label="Close dialog" className="w-7 h-7 rounded-md border-none bg-none text-ds-text-subtle cursor-pointer hover:bg-surface-control">
            <X aria-hidden className="w-4 h-4" />
          </button>
        </div>

        <div className="px-5 py-4 flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <span className="text-ui-2xs text-ds-text-subtle">Period</span>
            <div className="flex gap-2">
              {(["daily", "weekly"] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => setGoalType(t)}
                  className={cn(
                    "flex-1 p-1.5 text-ui-2xs capitalize rounded-md border border-ds-border-subtle bg-surface-control text-ds-text-subtle cursor-pointer",
                    goalType === t && "border-[color-mix(in_srgb,var(--brand)_50%,transparent)] bg-[color-mix(in_srgb,var(--brand)_5%,transparent)] text-brand",
                  )}
                >
                  {t}
                </button>
              ))}
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <span className="text-ui-2xs text-ds-text-subtle">Metric</span>
            <div className="flex flex-col gap-1.5">
              {METRICS.map((m) => (
                <button
                  key={m.value}
                  type="button"
                  onClick={() => setMetric(m.value)}
                  className={cn(
                    "px-3 py-2 text-ui-2xs text-left rounded-md border border-ds-border-subtle bg-surface-control text-ds-text-subtle cursor-pointer",
                    metric === m.value && "border-[color-mix(in_srgb,var(--brand)_50%,transparent)] bg-[color-mix(in_srgb,var(--brand)_5%,transparent)] text-brand",
                  )}
                >
                  {m.label}
                </button>
              ))}
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <label htmlFor="goal-target" className="text-ui-2xs text-ds-text-subtle">
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
              className="w-full px-3 py-1.5 text-[var(--fs-base)] bg-surface-control border border-ds-border-subtle rounded-md text-ds-text-strong"
              aria-invalid={isInvalid}
              aria-describedby={isInvalid ? "goal-target-error" : undefined}
            />
            {isInvalid && (
              <span
                id="goal-target-error"
                className="text-ui-2xs text-destructive mt-1"
                role="alert"
              >
                Please enter a positive number
              </span>
            )}
          </div>
        </div>

        <div className="flex justify-end gap-2 px-5 py-3 border-t border-ds-border-subtle">
          <button type="button" onClick={onClose} className="px-4 py-1.5 text-ui-2xs rounded-md border-none bg-none text-ds-text-subtle cursor-pointer">
            Cancel
          </button>
          <button type="button" onClick={handleSubmit} disabled={!canSubmit} className="px-4 py-1.5 text-ui-2xs rounded-md border-none bg-brand text-white cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed">
            Add goal
          </button>
        </div>
      </div>
    </div>
  );
}
