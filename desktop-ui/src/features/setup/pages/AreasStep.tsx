import { Plus, X } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { useOutletContext } from "react-router";
import { ipc } from "@shared/hooks/useIpc";
import type { SetupContext } from "../hooks/steps";

interface AreaEntry {
  key: string;
  id?: string; // database id — set when loaded from DB
  name: string;
  color: string;
}

const AREA_COLORS = [
  "#f97316",
  "#ef4444",
  "#8b5cf6",
  "#3b82f6",
  "#06b6d4",
  "#10b981",
  "#eab308",
  "#ec4899",
];

function makeDefaultAreas(): AreaEntry[] {
  return [
    { key: crypto.randomUUID(), name: "Work", color: "#3b82f6" },
    { key: crypto.randomUUID(), name: "Personal", color: "#10b981" },
    { key: crypto.randomUUID(), name: "Health", color: "#ef4444" },
    { key: crypto.randomUUID(), name: "Learning", color: "#8b5cf6" },
  ];
}

export function AreasStep() {
  const { forwardRef, setDirty } = useOutletContext<SetupContext>();
  const [areas, setAreas] = useState<AreaEntry[]>(makeDefaultAreas);
  const id = useId();

  // Load existing areas from DB on mount
  useEffect(() => {
    ipc<{ id: string; name: string; color?: string }[]>("area_list")
      .then((saved) => {
        if (saved && saved.length > 0) {
          setAreas(
            saved.map((a) => ({
              key: crypto.randomUUID(),
              id: a.id,
              name: a.name,
              color: a.color ?? "#3b82f6",
            })),
          );
          setDirty(true);
        }
      })
      .catch(() => {});
  }, [setDirty]);

  // Register save handler with layout
  useEffect(() => {
    forwardRef.current = async (isSkip: boolean) => {
      if (isSkip) return true;
      // Only create new areas (ones without a database id)
      const newAreas = areas.filter((a) => !a.id && a.name.trim());
      if (newAreas.length > 0) {
        await Promise.all(
          newAreas.map((area) => ipc("area_create", { name: area.name.trim(), color: area.color })),
        );
      }
      return true;
    };
  }, [forwardRef, areas]);

  const addArea = () => {
    const usedColors = new Set(areas.map((a) => a.color));
    const nextColor = AREA_COLORS.find((c) => !usedColors.has(c)) ?? AREA_COLORS[0];
    setAreas([...areas, { key: crypto.randomUUID(), name: "", color: nextColor }]);
    setDirty(true);
  };

  const removeArea = (key: string) => {
    setAreas(areas.filter((a) => a.key !== key));
    setDirty(true);
  };

  const updateArea = (key: string, update: Partial<AreaEntry>) => {
    setAreas(areas.map((a) => (a.key === key ? { ...a, ...update } : a)));
    setDirty(true);
  };

  return (
    <div>
      <h2 className="text-lg font-medium text-primary mb-1">Areas</h2>
      <p className="text-[13px] text-muted mb-6">
        Areas organize your tasks and projects into life domains. Customize the defaults or add your
        own.
      </p>

      <div className="space-y-2 max-h-[280px] overflow-y-auto pr-1">
        {areas.map((area) => (
          <div key={area.key} className="flex items-center gap-2">
            <div className="relative">
              <input
                type="color"
                value={area.color}
                aria-label={`Color for ${area.name || "area"}`}
                onChange={(e) => updateArea(area.key, { color: e.target.value })}
                className="w-7 h-7 rounded-lg border border-border cursor-pointer bg-transparent [&::-webkit-color-swatch-wrapper]:p-0.5 [&::-webkit-color-swatch]:rounded-md [&::-webkit-color-swatch]:border-none"
              />
            </div>

            <input
              id={`${id}-${area.key}`}
              type="text"
              value={area.name}
              onChange={(e) => updateArea(area.key, { name: e.target.value })}
              placeholder="Area name"
              className="flex-1 px-3 py-1.5 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
            />

            <button
              type="button"
              onClick={() => removeArea(area.key)}
              className="p-1 text-dim hover:text-destructive transition-colors"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        ))}
      </div>

      <button
        type="button"
        onClick={addArea}
        className="flex items-center gap-1.5 mt-3 text-[12px] text-muted hover:text-secondary transition-colors"
      >
        <Plus className="w-3.5 h-3.5" />
        Add area
      </button>
    </div>
  );
}
