import { Plus, X } from "lucide-react";
import { useId, useState } from "react";
import { useOutletContext } from "react-router";
import { ipc } from "../../../hooks/useIpc";
import type { SetupContext } from "../steps";

interface AreaEntry {
  key: string;
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
  const { next } = useOutletContext<SetupContext>();
  const [areas, setAreas] = useState<AreaEntry[]>(makeDefaultAreas);
  const [saving, setSaving] = useState(false);
  const id = useId();

  const addArea = () => {
    const usedColors = new Set(areas.map((a) => a.color));
    const nextColor = AREA_COLORS.find((c) => !usedColors.has(c)) ?? AREA_COLORS[0];
    setAreas([...areas, { key: crypto.randomUUID(), name: "", color: nextColor }]);
  };

  const removeArea = (key: string) => {
    setAreas(areas.filter((a) => a.key !== key));
  };

  const updateArea = (key: string, update: Partial<AreaEntry>) => {
    setAreas(areas.map((a) => (a.key === key ? { ...a, ...update } : a)));
  };

  const handleContinue = async () => {
    const validAreas = areas.filter((a) => a.name.trim());
    if (validAreas.length > 0) {
      setSaving(true);
      try {
        await Promise.all(
          validAreas.map((area) =>
            ipc("area_create", { name: area.name.trim(), color: area.color }),
          ),
        );
      } catch {
        // Non-blocking
      } finally {
        setSaving(false);
      }
    }
    next();
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

      <div className="mt-6 flex justify-end">
        <button
          type="button"
          onClick={handleContinue}
          disabled={saving}
          className="px-5 py-2 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors disabled:opacity-50"
        >
          {saving ? "Creating..." : "Continue"}
        </button>
      </div>
    </div>
  );
}
