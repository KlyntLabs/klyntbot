import { useMemo } from "react";
import { Cell, Pie, PieChart } from "recharts";
import { formatHumanDuration } from "../../lib/dates";
import type { CategoryUsage } from "../../lib/types";
import { getCategoryColor, getCategoryTypeColor } from "./shared";

interface CategoriesListProps {
  categories: CategoryUsage[];
  totalSecs: number;
}

interface CategoryGroup {
  label: string;
  type: string;
  color: string;
  categories: CategoryUsage[];
  totalSecs: number;
}

const GROUP_CONFIG: { type: string; label: string }[] = [
  { type: "productive", label: "Work" },
  { type: "neutral", label: "Utilities" },
  { type: "distracting", label: "Distraction" },
];

export function CategoriesList({ categories, totalSecs }: CategoriesListProps) {
  const active = useMemo(() => categories.filter((c) => c.durationSecs > 0), [categories]);

  const groups = useMemo<CategoryGroup[]>(() => {
    return GROUP_CONFIG.map((g) => {
      const cats = active.filter((c) => c.categoryType === g.type);
      return {
        label: g.label,
        type: g.type,
        color: getCategoryTypeColor(g.type),
        categories: cats,
        totalSecs: cats.reduce((sum, c) => sum + c.durationSecs, 0),
      };
    }).filter((g) => g.totalSecs > 0);
  }, [active]);

  // Inner ring: individual categories
  const innerData = useMemo(
    () =>
      active.map((cat, i) => ({
        name: cat.category,
        value: cat.durationSecs,
        color: getCategoryColor(cat.categoryId, i),
      })),
    [active],
  );

  // Outer ring: type groups
  const outerData = useMemo(
    () =>
      groups.map((g) => ({
        name: g.label,
        value: g.totalSecs,
        color: g.color,
      })),
    [groups],
  );

  if (active.length === 0) {
    return (
      <div className="glass-card p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Categories</h2>
        <p className="text-[12px] font-light text-dim">No category data</p>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Categories</h2>
        <span className="text-[10px] font-light text-dim tabular-nums">
          {formatHumanDuration(totalSecs)} tracked
        </span>
      </div>

      {/* Nested donut: outer = type groups, inner = categories */}
      <div className="flex justify-center">
        <PieChart width={100} height={100}>
          {/* Inner ring — categories */}
          <Pie
            data={innerData}
            cx={49}
            cy={49}
            innerRadius={20}
            outerRadius={32}
            startAngle={90}
            endAngle={-270}
            dataKey="value"
            stroke="none"
            paddingAngle={1}
          >
            {innerData.map((entry) => (
              <Cell key={entry.name} fill={entry.color} />
            ))}
          </Pie>
          {/* Outer ring — type groups */}
          <Pie
            data={outerData}
            cx={49}
            cy={49}
            innerRadius={35}
            outerRadius={46}
            startAngle={90}
            endAngle={-270}
            dataKey="value"
            stroke="none"
            paddingAngle={2}
          >
            {outerData.map((entry) => (
              <Cell key={entry.name} fill={entry.color} />
            ))}
          </Pie>
        </PieChart>
      </div>

      {/* Grouped legend */}
      <div className="flex flex-col gap-3">
        {groups.map((group) => {
          const groupPct = totalSecs > 0 ? Math.round((group.totalSecs / totalSecs) * 100) : 0;
          return (
            <div key={group.type} className="flex flex-col gap-1">
              {/* Group header */}
              <div className="flex items-center gap-2">
                <span
                  className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                  style={{ backgroundColor: group.color }}
                />
                <span className="text-[11px] font-medium text-secondary flex-1">{group.label}</span>
                <span className="text-[10px] font-medium text-secondary tabular-nums">
                  {groupPct}%
                </span>
                <span className="text-[10px] font-light text-dim tabular-nums w-14 text-right">
                  {formatHumanDuration(group.totalSecs)}
                </span>
              </div>
              {/* Category rows */}
              {group.categories.map((cat, i) => {
                const pct = totalSecs > 0 ? Math.round((cat.durationSecs / totalSecs) * 100) : 0;
                return (
                  <div key={cat.categoryId} className="flex items-center gap-2 pl-3.5">
                    <span
                      className="w-2 h-2 rounded-sm flex-shrink-0"
                      style={{ backgroundColor: getCategoryColor(cat.categoryId, i) }}
                    />
                    <span className="text-[11px] font-light text-primary flex-1 truncate">
                      {cat.category}
                    </span>
                    <span className="text-[10px] font-light text-dim tabular-nums">{pct}%</span>
                    <span className="text-[10px] font-light text-dim tabular-nums w-14 text-right">
                      {formatHumanDuration(cat.durationSecs)}
                    </span>
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
}
