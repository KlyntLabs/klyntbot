import { useState } from "react";
import { useQuery } from "@shared/hooks/useQuery";
import type { ActivityCategory, TrackedApp } from "@shared/types";
import { CategoryEditor } from "../components/CategoryEditor";
import { CategoryList } from "../components/CategoryList";
import { TrackedAppsList } from "../components/TrackedAppsList";

export function CategoriesPage() {
  const { data: categories, refetch: refetchCategories } = useQuery<ActivityCategory[]>(
    "productivity_categories",
    undefined,
    [],
  );
  const { data: trackedApps, refetch: refetchApps } = useQuery<TrackedApp[]>(
    "productivity_tracked_apps",
    undefined,
    [],
  );

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = categories.find((c) => c.id === selectedId) ?? null;

  const refresh = () => {
    refetchCategories();
    refetchApps();
  };

  return (
    <div className="flex gap-4 h-full min-h-0 p-4">
      {/* Panel A: Category list */}
      <div className="w-56 flex-shrink-0 overflow-y-auto">
        <CategoryList
          categories={categories}
          selectedId={selectedId}
          onSelect={setSelectedId}
          onCreated={refetchCategories}
        />
      </div>

      {/* Panel B: Category editor */}
      <div className="flex-1 min-w-0 overflow-y-auto">
        <CategoryEditor
          key={selectedId}
          category={selected}
          onSaved={refetchCategories}
          onDeleted={() => {
            setSelectedId(null);
            refresh();
          }}
        />
      </div>

      {/* Panel C: Tracked apps */}
      <div className="w-72 flex-shrink-0 overflow-y-auto">
        <TrackedAppsList apps={trackedApps} categories={categories} onReassigned={refresh} />
      </div>
    </div>
  );
}
