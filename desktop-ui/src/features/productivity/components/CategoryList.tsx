import { Plus } from "lucide-react";
import { useMemo } from "react";
import { useMutation } from "@shared/hooks/useMutation";
import type { ActivityCategory } from "@shared/types";
import {
  CATEGORY_TYPE_GROUPS,
  DEFAULT_CATEGORY_COLOR,
  getCategoryColor,
  getCategoryTypeColor,
} from "../lib/constants";

interface CategoryListProps {
  categories: ActivityCategory[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreated: () => void;
}

export function CategoryList({ categories, selectedId, onSelect, onCreated }: CategoryListProps) {
  const groups = useMemo(
    () =>
      CATEGORY_TYPE_GROUPS.map((g) => ({
        ...g,
        color: getCategoryTypeColor(g.type),
        items: categories.filter((c) => c.categoryType === g.type),
      })).filter((g) => g.items.length > 0),
    [categories],
  );

  const createMut = useMutation("productivity_category_upsert");

  const handleCreate = async () => {
    const id = `custom_${crypto.randomUUID()}`;
    await createMut.mutate({
      id,
      name: "New Category",
      category_type: "neutral",
      color: DEFAULT_CATEGORY_COLOR,
      icon: null,
      rules: { appNames: [], bundleIds: [], urlPatterns: [] },
    });
    onCreated();
    onSelect(id);
  };

  return (
    <div className="glass-card p-3 flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h3 className="text-[12px] font-medium text-secondary">Categories</h3>
        <button
          type="button"
          onClick={handleCreate}
          className="p-1 rounded-md hover:bg-white/[0.06] text-muted hover:text-primary transition-colors"
          title="Add category"
        >
          <Plus size={14} />
        </button>
      </div>

      {groups.map((group) => (
        <div key={group.type} className="flex flex-col gap-0.5">
          <div className="flex items-center gap-1.5 py-1">
            <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: group.color }} />
            <span className="text-[10px] font-medium text-muted uppercase tracking-wider">
              {group.label}
            </span>
          </div>
          {group.items.map((cat) => {
            const isSelected = cat.id === selectedId;
            return (
              <button
                type="button"
                key={cat.id}
                onClick={() => onSelect(cat.id)}
                className={`flex items-center gap-2 px-2 py-1.5 rounded-lg text-left transition-colors ${
                  isSelected
                    ? "bg-white/[0.08] text-primary"
                    : "text-secondary hover:bg-white/[0.04] hover:text-primary"
                }`}
              >
                <span
                  className="w-2.5 h-2.5 rounded-sm flex-shrink-0"
                  style={{ backgroundColor: cat.color ?? getCategoryColor(cat.id) }}
                />
                <span className="text-[11px] font-light truncate">{cat.name}</span>
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}
