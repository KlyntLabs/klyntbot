import { BarChart3, CheckCircle, Flame, Library } from "lucide-react";

interface StatsBarProps {
  totalDue: number;
}

function StatCard({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string | number;
}) {
  return (
    <div className="glass-card flex items-center gap-2.5 px-3 py-2.5 flex-1 min-w-0">
      <div className="text-muted-foreground shrink-0">{icon}</div>
      <div className="min-w-0">
        <p className="text-[11px] text-muted-foreground leading-none mb-0.5">{label}</p>
        <p className="text-sm font-semibold text-foreground tabular-nums leading-none">{value}</p>
      </div>
    </div>
  );
}

export function StatsBar({ totalDue }: StatsBarProps) {
  return (
    <div className="flex gap-2">
      <StatCard icon={<Flame size={16} strokeWidth={1.5} />} label="Streak" value="--" />
      <StatCard icon={<Library size={16} strokeWidth={1.5} />} label="Due" value={totalDue} />
      <StatCard icon={<CheckCircle size={16} strokeWidth={1.5} />} label="Retention" value="--" />
      <StatCard icon={<BarChart3 size={16} strokeWidth={1.5} />} label="This week" value="--" />
    </div>
  );
}
