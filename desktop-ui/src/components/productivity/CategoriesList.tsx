import { useMemo } from "react";
import { Cell, Pie, PieChart } from "recharts";
import { formatHumanDuration } from "../../lib/dates";
import type { CategoryUsage } from "../../lib/types";
import { getCategoryColor } from "./shared";

interface CategoriesListProps {
  categories: CategoryUsage[];
  totalSecs: number;
}

export function CategoriesList({ categories, totalSecs }: CategoriesListProps) {
  const pieData = useMemo(
    () =>
      categories.map((cat, i) => ({
        name: cat.category,
        value: cat.durationSecs,
        color: getCategoryColor(cat.category, i),
      })),
    [categories],
  );

  if (categories.length === 0) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Categories</h2>
        <p className="text-[12px] font-light text-dim">No category data</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Categories</h2>
        <span className="text-[10px] font-light text-dim tabular-nums">
          {formatHumanDuration(totalSecs)} tracked
        </span>
      </div>

      {/* Mini donut + legend side by side */}
      <div className="flex items-center gap-4">
        {/* Compact donut */}
        <div className="flex-shrink-0 relative">
          <PieChart width={72} height={72}>
            <Pie
              data={pieData}
              cx={35}
              cy={35}
              innerRadius={22}
              outerRadius={32}
              startAngle={90}
              endAngle={-270}
              dataKey="value"
              stroke="none"
              paddingAngle={2}
            >
              {pieData.map((entry) => (
                <Cell key={entry.name} fill={entry.color} />
              ))}
            </Pie>
          </PieChart>
        </div>

        {/* Legend rows — read color from pieData to avoid double lookup */}
        <div className="flex-1 flex flex-col gap-1.5 min-w-0">
          {pieData.map((entry, i) => {
            const cat = categories[i];
            const pct = totalSecs > 0 ? Math.round((cat.durationSecs / totalSecs) * 100) : 0;
            return (
              <div key={entry.name} className="flex items-center gap-2">
                <span
                  className="w-2 h-2 rounded-sm flex-shrink-0"
                  style={{ backgroundColor: entry.color }}
                />
                <span className="text-[11px] font-light text-primary flex-1 truncate">
                  {cat.category}
                </span>
                <span className="text-[10px] font-light text-dim tabular-nums">{pct}%</span>
                <span className="text-[10px] font-light text-dim tabular-nums w-12 text-right">
                  {formatHumanDuration(cat.durationSecs)}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
