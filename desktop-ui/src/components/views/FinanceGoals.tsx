import { Plus, Target } from "lucide-react";
import { useMemo } from "react";
import { useEvent } from "../../hooks/useEvent";
import { useQuery } from "../../hooks/useQuery";
import { COLORS, fmtCompact, fmtMoney, GOAL_ICONS, pct } from "../../lib/finance";
import type { FinanceGoal } from "../../lib/types";
import { Card, SectionLabel } from "../finance/Card";
import { Donut } from "../finance/Donut";
import { FinanceLayout } from "../finance/FinanceLayout";
import { Progress } from "../ui/Progress";
export function FinanceGoals() {
  const { data: goals, refetch } = useQuery<FinanceGoal[]>("finance_goals", undefined, []);
  useEvent<{ entityKind: string }>("entity:updated", refetch);

  const activeGoals = goals.filter((g) => g.status === "active");
  const totalTarget = useMemo(
    () => activeGoals.reduce((s, g) => s + g.targetAmount, 0),
    [activeGoals],
  );
  const totalSaved = useMemo(
    () => activeGoals.reduce((s, g) => s + g.currentAmount, 0),
    [activeGoals],
  );
  const monthlyTotal = useMemo(
    () => activeGoals.reduce((s, g) => s + (g.monthlyContribution ?? 0), 0),
    [activeGoals],
  );

  const goalSegs = useMemo(
    () =>
      activeGoals.map((g, i) => ({
        name: g.name,
        value: g.currentAmount,
        color: COLORS[i % COLORS.length],
      })),
    [activeGoals],
  );

  return (
    <FinanceLayout onRefresh={refetch}>
      <div className="grid grid-cols-12 gap-3 auto-rows-min">
        {/* ── Stats row ─────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-4 gap-3">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Active Goals
            </p>
            <p className="text-[20px] font-light text-primary">{activeGoals.length}</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Total Saved
            </p>
            <p className="text-[20px] font-light text-success">{fmtCompact(totalSaved)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Total Target
            </p>
            <p className="text-[20px] font-light text-primary">{fmtCompact(totalTarget)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Monthly Contributions
            </p>
            <p className="text-[20px] font-light text-brand">{fmtCompact(monthlyTotal)}đ</p>
          </Card>
        </div>

        {/* ── Goal cards (8col) + Chart (4col) ─────────── */}
        <div className="col-span-8">
          <div className="flex items-center justify-between mb-2">
            <SectionLabel>Goals</SectionLabel>
            <button className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors">
              <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Goal
            </button>
          </div>
          <div className="space-y-3">
            {activeGoals.map((g, i) => {
              const p = pct(g.currentAmount, g.targetAmount);
              const Icon = GOAL_ICONS[g.goalType] ?? Target;
              const remaining = g.targetAmount - g.currentAmount;
              const monthsLeft =
                g.monthlyContribution && g.monthlyContribution > 0
                  ? Math.ceil(remaining / g.monthlyContribution)
                  : null;
              return (
                <Card key={g.id} className="p-4">
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center gap-2">
                      <div
                        className="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0"
                        style={{ backgroundColor: `${COLORS[i % COLORS.length]}20` }}
                      >
                        <Icon
                          className="w-4 h-4"
                          style={{ color: COLORS[i % COLORS.length] }}
                          strokeWidth={1.5}
                        />
                      </div>
                      <div>
                        <p className="text-[13px] font-light text-secondary">{g.name}</p>
                        <p className="text-[9px] text-dim font-light">{g.goalType}</p>
                      </div>
                    </div>
                    <div className="text-right">
                      <p className="text-[14px] font-light text-primary">
                        {fmtMoney(g.currentAmount, g.currency)}{" "}
                        <span className="text-dim text-[10px]">
                          / {fmtMoney(g.targetAmount, g.currency)}
                        </span>
                      </p>
                      <p className="text-[11px] font-light text-brand">{p}%</p>
                    </div>
                  </div>
                  <Progress value={p} />
                  <div className="flex items-center gap-4 mt-2">
                    {g.deadline && (
                      <span className="text-[10px] text-dim font-light">
                        Deadline: {g.deadline}
                      </span>
                    )}
                    {g.monthlyContribution && (
                      <span className="text-[10px] text-dim font-light">
                        Contributing: {fmtMoney(g.monthlyContribution, g.currency)}/mo
                      </span>
                    )}
                    {monthsLeft != null && (
                      <span className="text-[10px] text-muted font-light">
                        ~{monthsLeft} months to go
                      </span>
                    )}
                    <span className="text-[10px] text-dim font-light">
                      {fmtMoney(remaining, g.currency)} remaining
                    </span>
                  </div>
                </Card>
              );
            })}
          </div>
        </div>

        <div className="col-span-4 space-y-3">
          <div>
            <SectionLabel>Progress Distribution</SectionLabel>
            <Card className="p-4 flex items-center justify-center">
              <Donut
                segments={goalSegs}
                label="Saved"
                value={`${fmtCompact(totalSaved)}đ`}
                size={150}
              />
            </Card>
          </div>
          <div>
            <SectionLabel>Overall Progress</SectionLabel>
            <Card className="p-4">
              <div className="mb-3">
                <div className="flex justify-between mb-1">
                  <span className="text-[10px] text-muted font-light">Total Progress</span>
                  <span className="text-[10px] text-brand font-light">
                    {pct(totalSaved, totalTarget)}%
                  </span>
                </div>
                <Progress value={pct(totalSaved, totalTarget)} />
              </div>
              <div className="space-y-2 pt-2 border-t border-border-subtle">
                {activeGoals.map((g, i) => {
                  const p = pct(g.currentAmount, g.targetAmount);
                  return (
                    <div key={g.id}>
                      <div className="flex justify-between mb-0.5">
                        <div className="flex items-center gap-1.5">
                          <div
                            className="w-1.5 h-1.5 rounded-full"
                            style={{ backgroundColor: COLORS[i % COLORS.length] }}
                          />
                          <span className="text-[10px] text-muted font-light">{g.name}</span>
                        </div>
                        <span className="text-[10px] text-secondary font-light">{p}%</span>
                      </div>
                      <Progress value={p} />
                    </div>
                  );
                })}
              </div>
            </Card>
          </div>
        </div>
      </div>
    </FinanceLayout>
  );
}
