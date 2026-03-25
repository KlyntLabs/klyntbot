import { useMemo } from "react";
import { Area, AreaChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { EvolutionPoint } from "../../hooks/useInsightEvolution";

interface Props {
  versions: EvolutionPoint[];
}

export function InsightEvolutionChart({ versions }: Props) {
  const data = useMemo(
    () =>
      versions.map((v) => ({
        version: `v${v.version}`,
        overall: Math.round(v.overallProgress * 100),
        flashcard: Math.round(v.flashcardSuccess * 100),
        gaps: Math.round(v.gapClosure * 100),
        stability: Math.round((1 - v.semanticDrift) * 100),
        changeNote: v.changeNote,
      })),
    [versions],
  );

  if (data.length < 1) return null;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between px-1">
        <span className="text-[11px] font-medium text-muted-foreground">Learning Progress</span>
        <div className="flex items-center gap-3">
          {LEGEND.map((item) => (
            <span key={item.label} className="flex items-center gap-1 text-[9px] text-dim">
              <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: item.color }} />
              {item.label}
            </span>
          ))}
        </div>
      </div>
      <ResponsiveContainer width="100%" height={140}>
        <AreaChart data={data} margin={{ top: 4, right: 8, bottom: 0, left: -20 }}>
          <XAxis
            dataKey="version"
            tick={{ fill: "var(--text-dim)", fontSize: 10 }}
            axisLine={false}
            tickLine={false}
          />
          <YAxis
            domain={[0, 100]}
            tick={{ fill: "var(--text-dim)", fontSize: 10 }}
            axisLine={false}
            tickLine={false}
            width={28}
            tickFormatter={(v) => `${v}%`}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: "var(--card)",
              border: "1px solid var(--border)",
              borderRadius: 8,
              fontSize: 11,
            }}
            formatter={(value: number, name: string) => [`${value}%`, LABEL_MAP[name] ?? name]}
            labelFormatter={(label) => label}
          />
          <Area
            type="monotone"
            dataKey="overall"
            stroke="var(--brand)"
            fill="var(--brand)"
            fillOpacity={0.15}
            strokeWidth={2}
            dot={{ r: 3, fill: "var(--brand)" }}
          />
          <Area
            type="monotone"
            dataKey="flashcard"
            stroke="var(--success)"
            fill="none"
            strokeWidth={1}
            strokeDasharray="4 2"
            dot={false}
          />
          <Area
            type="monotone"
            dataKey="gaps"
            stroke="var(--chart-2)"
            fill="none"
            strokeWidth={1}
            strokeDasharray="4 2"
            dot={false}
          />
          <Area
            type="monotone"
            dataKey="stability"
            stroke="var(--purple)"
            fill="none"
            strokeWidth={1}
            strokeDasharray="4 2"
            dot={false}
          />
        </AreaChart>
      </ResponsiveContainer>
      {versions.length > 0 && (
        <p className="text-2xs text-dim italic px-1">
          Latest: {versions[versions.length - 1]?.changeNote}
        </p>
      )}
    </div>
  );
}

const LEGEND = [
  { label: "Overall", color: "var(--brand)" },
  { label: "Flashcards", color: "var(--success)" },
  { label: "Gap Closure", color: "var(--chart-2)" },
  { label: "Stability", color: "var(--purple)" },
];

const LABEL_MAP: Record<string, string> = {
  overall: "Overall Progress",
  flashcard: "Flashcard Success",
  gaps: "Gap Closure",
  stability: "Content Stability",
};
