import { ipc } from "@shared/hooks/useIpc";
import { Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

const GOAL_TYPES = ["savings", "debt_payoff", "investment", "purchase", "other"] as const;
const GOAL_TYPE_OPTIONS = GOAL_TYPES.map((t) => ({
  value: t,
  label: t.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()),
}));

interface GoalEntry {
  key: string;
  name: string;
  goalType: string;
  targetAmount: string;
  deadline: string;
  monthlyContribution: string;
}

interface GoalsFormProps {
  registerSave: (fn: () => Promise<void>) => void;
  onDirty: () => void;
}

export function GoalsForm({ registerSave, onDirty }: GoalsFormProps) {
  const [goals, setGoals] = useState<GoalEntry[]>([]);

  const addGoal = () => {
    setGoals([
      ...goals,
      {
        key: crypto.randomUUID(),
        name: "",
        goalType: "savings",
        targetAmount: "",
        deadline: "",
        monthlyContribution: "",
      },
    ]);
    onDirty();
  };

  const removeGoal = (key: string) => {
    setGoals(goals.filter((g) => g.key !== key));
    onDirty();
  };

  const updateGoal = (key: string, update: Partial<GoalEntry>) => {
    setGoals(goals.map((g) => (g.key === key ? { ...g, ...update } : g)));
    onDirty();
  };

  const savedKeysRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    registerSave(async () => {
      const valid = goals.filter(
        (g) => g.name.trim() && g.targetAmount && !savedKeysRef.current.has(g.key),
      );
      if (valid.length > 0) {
        const results = await Promise.allSettled(
          valid.map((goal) =>
            ipc("finance_goal_create", {
              params: {
                name: goal.name.trim(),
                goalType: goal.goalType,
                targetAmount: Math.round(Number.parseFloat(goal.targetAmount) * 100),
                currentAmount: null,
                deadline: goal.deadline || null,
                monthlyContribution: goal.monthlyContribution
                  ? Math.round(Number.parseFloat(goal.monthlyContribution) * 100)
                  : null,
                currency: null,
                notes: null,
              },
            }),
          ),
        );
        for (let i = 0; i < results.length; i++) {
          if (results[i].status === "fulfilled") savedKeysRef.current.add(valid[i].key);
          else console.error("Failed to save goal:", (results[i] as PromiseRejectedResult).reason);
        }
        const failCount = results.filter((r) => r.status === "rejected").length;
        if (failCount > 0)
          throw new Error(`Failed to save ${failCount} goal${failCount === 1 ? "" : "s"}`);
      }
    });
  }, [goals, registerSave]);

  return (
    <div>
      <h3 className="text-sm font-medium text-muted-foreground mb-1">Financial Goals</h3>
      <p className="text-[11px] text-dim mb-4">
        Set targets for savings, debt payoff, or investments.
      </p>

      <div className="space-y-2 max-h-[240px] overflow-y-auto pr-1">
        {goals.map((goal) => (
          <div
            key={goal.key}
            className="bg-card rounded-lg border border-border-subtle p-3 space-y-2"
          >
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={goal.name}
                onChange={(e) => updateGoal(goal.key, { name: e.target.value })}
                placeholder="Goal name"
                className="flex-1 px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
              <button
                type="button"
                onClick={() => removeGoal(goal.key)}
                className="p-1 text-dim hover:text-destructive transition-colors"
              >
                <X className="size-3.5" />
              </button>
            </div>
            <div className="flex gap-2">
              <select
                value={goal.goalType}
                onChange={(e) => updateGoal(goal.key, { goalType: e.target.value })}
                className="flex-1 px-2 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              >
                {GOAL_TYPE_OPTIONS.map((t) => (
                  <option key={t.value} value={t.value} className="bg-popover">
                    {t.label}
                  </option>
                ))}
              </select>
              <input
                type="number"
                value={goal.targetAmount}
                onChange={(e) => updateGoal(goal.key, { targetAmount: e.target.value })}
                placeholder="Target amount"
                step="0.01"
                className="w-32 px-2 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </div>
            <div className="flex gap-2">
              <input
                type="date"
                value={goal.deadline}
                onChange={(e) => updateGoal(goal.key, { deadline: e.target.value })}
                className="flex-1 px-2 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              />
              <input
                type="number"
                value={goal.monthlyContribution}
                onChange={(e) => updateGoal(goal.key, { monthlyContribution: e.target.value })}
                placeholder="Monthly contribution"
                step="0.01"
                className="flex-1 px-2 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </div>
          </div>
        ))}
      </div>

      <button
        type="button"
        onClick={addGoal}
        className="flex items-center gap-1.5 mt-3 text-xs text-muted-foreground hover:text-foreground transition-colors"
      >
        <Plus className="size-3.5" />
        Add goal
      </button>
    </div>
  );
}
