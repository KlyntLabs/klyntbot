import { formatHumanDuration } from "../../lib/dates";
import type { CategoryUsage } from "../../lib/types";

interface CategoriesListProps {
  categories: CategoryUsage[];
  totalSecs: number;
}

const CATEGORY_COLORS = [
  "var(--brand)",
  "var(--purple)",
  "var(--info)",
  "var(--success)",
  "var(--text-muted)",
  "var(--destructive)",
  "var(--dim)",
];

export function CategoriesList({ categories, totalSecs }: CategoriesListProps) {
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
        <span className="text-[10px] font-light text-dim">
          Total tracked time: {formatHumanDuration(totalSecs)}
        </span>
      </div>
      <div className="flex flex-col gap-2">
        {categories.map((cat, i) => {
          const pct = totalSecs > 0 ? Math.round((cat.durationSecs / totalSecs) * 100) : 0;
          const color = CATEGORY_COLORS[i % CATEGORY_COLORS.length];
          return (
            <div key={cat.category} className="flex items-center gap-3">
              <span className="text-[11px] font-light text-muted w-8 text-right tabular-nums">
                {pct}%
              </span>
              <span className="text-[11px] font-light text-primary flex-1 truncate">
                {cat.category}
              </span>
              <div className="w-20 h-1.5 rounded-full bg-surface-raised overflow-hidden flex-shrink-0">
                <div
                  className="h-full rounded-full"
                  style={{ width: `${pct}%`, backgroundColor: color }}
                />
              </div>
              <span className="text-[11px] font-light text-muted tabular-nums w-16 text-right">
                {formatHumanDuration(cat.durationSecs)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
